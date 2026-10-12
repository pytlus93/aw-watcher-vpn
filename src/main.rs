use aw_watcher_vpn::{active_connections, learn, Config, Connection, Snapshot};
use base64::Engine;
use chrono::{DateTime, Utc};
use serde_json::json;
use std::{
    collections::{HashSet, VecDeque},
    fs,
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
mod actions;
mod host;
mod management;
mod owned_helper;
mod payload;
macro_rules! diag { ($($arg:tt)*) => { host::record(&format!($($arg)*)) }; }
fn parse_json<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())
}

fn read_config(path: &str) -> Result<Config, String> {
    let c: Config = parse_json(&fs::read_to_string(path).map_err(|e| e.to_string())?)?;
    c.validate()?;
    Ok(c)
}
fn read_snapshot(path: &str) -> Result<Snapshot, String> {
    let s: Snapshot = parse_json(&fs::read_to_string(path).map_err(|e| e.to_string())?)?;
    if s.schema_version != 1 {
        return Err("unsupported snapshot schema".into());
    }
    Ok(s)
}

#[cfg(windows)]
fn capture() -> Result<Snapshot, String> {
    use std::os::windows::process::CommandExt;
    let script = include_str!("../scripts/snapshot.ps1");
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    // Pin the OS executable instead of searching PATH. No policy bypass or admin.
    let exe =
        std::path::PathBuf::from(std::env::var_os("SystemRoot").ok_or("SystemRoot unavailable")?)
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut child = Command::new(exe)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &base64::engine::general_purpose::STANDARD.encode(bytes),
        ])
        .creation_flags(0x08000000)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().ok_or("stdout unavailable")?;
    let mut stderr = child.stderr.take().ok_or("stderr unavailable")?;
    let out = thread::spawn(move || {
        let mut b = Vec::new();
        stdout.read_to_end(&mut b).map(|_| b)
    });
    let err = thread::spawn(move || {
        let mut b = Vec::new();
        stderr.read_to_end(&mut b).map(|_| b)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if start.elapsed() >= Duration::from_secs(15) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out.join();
                let _ = err.join();
                return Err("network collector timed out".into());
            }
            None => thread::sleep(Duration::from_millis(50)),
        }
    };
    let bytes = out
        .join()
        .map_err(|_| "collector reader failed")?
        .map_err(|e| e.to_string())?;
    let _ = err.join(); // Do not print raw network details in error diagnostics.
    if !status.success() {
        return Err("Windows collector failed; check NetTCPIP permissions/policy".into());
    }
    let s: Snapshot =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid collector JSON: {e}"))?;
    if s.schema_version != 1 {
        return Err("unsupported collector schema".into());
    }
    Ok(s)
}
#[cfg(not(windows))]
fn capture() -> Result<Snapshot, String> {
    Err("live capture requires Windows; evaluate/learn work offline".into())
}

#[derive(Clone)]
struct Pending {
    bucket: String,
    timestamp: DateTime<Utc>,
    data: Context,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Context {
    schema_version: u32,
    #[serde(default)]
    title: String,
    connections: Vec<Connection>,
}
impl Context {
    fn new(connections: Vec<Connection>) -> Self {
        let mut names: Vec<String> = connections
            .iter()
            .flat_map(|c| {
                if c.customers.is_empty() {
                    vec![c.name.trim().to_string()]
                } else {
                    c.customers
                        .iter()
                        .map(|name| name.trim().to_string())
                        .collect()
                }
            })
            .filter(|name| !name.is_empty())
            .collect();
        names.sort_by(|a, b| {
            a.to_lowercase()
                .cmp(&b.to_lowercase())
                .then_with(|| a.cmp(b))
        });
        names.dedup_by(|a, b| a.to_lowercase() == b.to_lowercase());
        Self {
            schema_version: 3,
            title: if names.is_empty() && !connections.is_empty() {
                "Activity (context unavailable)".into()
            } else {
                names.join("|")
            },
            connections,
        }
    }
}
struct Adapter {
    agent: ureq::Agent,
    base: String,
    host: String,
    pulse: u64,
    created: HashSet<String>,
    queue: VecDeque<Pending>,
    payload: aw_watcher_vpn::PayloadSettings,
}
impl Adapter {
    fn new(port: u16, host: String, interval: u64) -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .try_proxy_from_env(false)
                .timeout(Duration::from_secs(2))
                .redirects(0)
                .build(),
            base: format!("http://127.0.0.1:{port}/api/0"),
            host,
            pulse: interval + 1,
            created: HashSet::new(),
            queue: VecDeque::new(),
            payload: Default::default(),
        }
    }
    fn set_payload(&mut self, settings: &aw_watcher_vpn::PayloadSettings) {
        if &self.payload != settings {
            self.queue.clear(); // Never retry previously enabled private fields after a policy change.
            self.payload = settings.clone();
        }
    }
    fn enqueue(&mut self, timestamp: DateTime<Utc>, data: Context) {
        if data.connections.is_empty() {
            return;
        }
        if self.queue.len() >= 512 {
            self.queue.pop_front();
            diag!("AW queue full: oldest sample dropped");
        }
        let bucket = format!("aw-watcher-vpn_{}", self.host);
        self.queue.push_back(Pending {
            bucket,
            timestamp,
            data,
        });
    }
    fn send(&mut self, p: &Pending) -> Result<(), String> {
        if !self.created.contains(&p.bucket) {
            let response = self
                .agent
                .post(&format!("{}/buckets/{}", self.base, p.bucket))
                .send_json(
                    json!({"client":"aw-watcher-vpn", "type":"vpn.context", "hostname":self.host}),
                )
                .map_err(|e| e.to_string())?;
            if response.status() == 304 {
                let metadata = self
                    .agent
                    .get(&format!("{}/buckets/{}", self.base, p.bucket))
                    .call()
                    .map_err(|e| e.to_string())?;
                if !(200..300).contains(&metadata.status()) {
                    return Err(format!(
                        "Unexpected bucket metadata status {}",
                        metadata.status()
                    ));
                }
                let metadata: serde_json::Value =
                    metadata.into_json().map_err(|e| e.to_string())?;
                if metadata["type"] != "vpn.context" {
                    return Err("Existing bucket has incompatible type; no heartbeat sent".into());
                }
            } else if !(200..300).contains(&response.status()) {
                return Err(format!(
                    "Unexpected bucket creation status {}",
                    response.status()
                ));
            }
            self.created.insert(p.bucket.clone());
        }
        let result = self
            .agent
            .post(&format!(
                "{}/buckets/{}/heartbeat?pulsetime={}",
                self.base, p.bucket, self.pulse
            ))
            .send_json(payload::Event {
                timestamp: p.timestamp.to_rfc3339(),
                duration: 0,
                data: &payload::EventData::new(&p.data, &self.payload),
            });
        if matches!(result, Err(ureq::Error::Status(404, _))) {
            self.created.remove(&p.bucket);
        }
        let response = result.map_err(|e| e.to_string())?;
        if !(200..300).contains(&response.status()) {
            return Err(format!("Unexpected heartbeat status {}", response.status()));
        }
        Ok(())
    }
    fn flush(&mut self) {
        // Bounded work and age. FIFO prevents old states overwriting newer states.
        for _ in 0..10 {
            let Some(p) = self.queue.front().cloned() else {
                return;
            };
            if Utc::now().signed_duration_since(p.timestamp).num_seconds() > 300 {
                self.queue.pop_front();
                continue;
            }
            if let Err(error) = self.send(&p) {
                diag!("ActivityWatch delivery failed: {error}; samples retained locally in memory");
                return;
            }
            self.queue.pop_front();
        }
    }
}
fn write_json(path: &str, value: &impl serde::Serialize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    if path == "-" {
        println!("{text}");
        return Ok(());
    }
    // Refuse accidental overwrite of private captures/configs.
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(text.as_bytes()).map_err(|e| e.to_string())
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Discovered {
    id: String,
    name: String,
    rules: aw_watcher_vpn::Rules,
    #[serde(default)]
    suggested_name: Option<String>,
}
fn read_discovered(path: &str) -> Result<Vec<Discovered>, String> {
    match fs::read_to_string(path) {
        Ok(text) => parse_json(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(windows)]
fn notify_unknown(name: &str) -> Result<(), String> {
    management::launch(name, true)
}
#[cfg(not(windows))]
fn notify_unknown(_name: &str) -> Result<(), String> {
    Ok(())
}

fn run(
    config_path: &str,
    once: bool,
    testing: bool,
    dry_run: bool,
    autolearn: bool,
    notify: bool,
) -> Result<(), String> {
    let mut config = read_config(config_path)?;
    let _instance = host::instance_lock(config_path)?;
    let discovery_path = format!("{config_path}.discovered.json");
    let mut notified = HashSet::new();
    let host: String = std::env::var("COMPUTERNAME")
        .unwrap_or_else(|_| "local".into())
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut aw = Adapter::new(if testing { 5666 } else { 5600 }, host, config.poll_seconds);
    aw.base = format!("{}/api/0", actions::server_url(actions::testing(testing)?)?);
    let mut previous: Option<Context> = None;
    loop {
        let start = Instant::now();
        match read_config(config_path) {
            Ok(updated) => config = updated,
            Err(e) => diag!("Config reload failed: {e}; retaining previous rules"),
        }
        aw.pulse = config.poll_seconds + 1;
        aw.set_payload(&config.payload);
        let autolearn = autolearn && config.autolearn;
        let notify = notify && config.notify;
        let snapshot = match capture() {
            Ok(s) => Some(s),
            Err(e) => {
                diag!("{e}; leaving a gap in the context timeline");
                None
            }
        };
        let capture_failed = snapshot.is_none();
        let now = snapshot
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(&s.observed_at).ok())
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        if let Some(snapshot) = snapshot.as_ref() {
            let context = Context::new(active_connections(&config, snapshot));
            for connection in context
                .connections
                .iter()
                .filter(|c| c.customers.is_empty())
            {
                if !notified.contains(&connection.id) {
                    // Retry failed writes rather than silently losing a discovery.
                    if autolearn && !dry_run {
                        let result = (|| {
                            let existing = match fs::read_to_string(&discovery_path) {
                                Ok(s) => Some(s),
                                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                                Err(e) => return Err(e.to_string()),
                            };
                            let mut discovered = read_discovered(&discovery_path)?;
                            if !discovered.iter().any(|d| d.id == connection.id) {
                                let adapter = snapshot
                                    .adapters
                                    .iter()
                                    .find(|a| a.id == connection.id)
                                    .ok_or("adapter disappeared")?;
                                discovered.push(Discovered {
                                    id: connection.id.clone(),
                                    name: connection.name.clone(),
                                    suggested_name: connection.suggested_name.clone(),
                                    rules: aw_watcher_vpn::Rules {
                                        adapter_names: vec![adapter.name.clone()],
                                        ..Default::default()
                                    },
                                });
                                save_config(&discovery_path, existing.as_deref(), &discovered)?;
                            }
                            Ok::<(), String>(())
                        })();
                        if let Err(e) = result {
                            diag!("Discovery save failed: {e}");
                            continue;
                        }
                    }
                    diag!("Unnamed connection detected. Open management to review it.");
                    if notify && !dry_run {
                        if autolearn {
                            if let Err(e) = notify_unknown(config_path) {
                                diag!("Notification failed: {e}");
                            }
                        } else {
                            if let Err(e) = management::notify_only(&connection.name) {
                                diag!("Notification failed: {e}");
                            }
                        }
                    }
                    notified.insert(connection.id.clone());
                }
            }
            if previous.as_ref() != Some(&context) {
                if autolearn && !dry_run {
                    if let Err(e) = management::save_context(config_path, &context) {
                        diag!("Management context save failed: {e}");
                    }
                }
                println!(
                    "{}",
                    serde_json::to_string(&payload::Event {
                        timestamp: now.to_rfc3339(),
                        duration: 0,
                        data: &payload::EventData::new(&context, &config.payload)
                    })
                    .map_err(|e| e.to_string())?
                );
                previous = Some(context.clone());
            }
            if !dry_run {
                aw.enqueue(now, context);
            }
        } else {
            previous = None;
            if autolearn && !dry_run {
                let _ = management::clear_context(config_path);
            }
        }
        if !dry_run {
            aw.flush();
        }
        if once {
            if !aw.queue.is_empty() {
                return Err("sample collected but AW delivery failed".into());
            }
            if capture_failed {
                return Err("network collection failed".into());
            }
            return Ok(());
        }
        thread::sleep(Duration::from_secs(config.poll_seconds).saturating_sub(start.elapsed()));
    }
}
fn main_result() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(result) = actions::dispatch(&args) {
        return result;
    }
    if args.is_empty() || args[0].starts_with("--") {
        return host::launch(&args);
    }
    match args.first().map(String::as_str) {
        Some("snapshot") if args.len() == 2 => write_json(&args[1], &capture()?),
        Some("evaluate") if args.len() == 3 => { let config = read_config(&args[1])?; write_json("-", &payload::EventData::new(&Context::new(active_connections(&config, &read_snapshot(&args[2])?)), &config.payload)) },
        Some("learn") if args.len() >= 4 => {
            let mut name = None;
            let mut replace = false;
            let mut i = 4;
            while i < args.len() {
                match args[i].as_str() {
                    "--name" if name.is_none() && i + 1 < args.len() => { i += 1; name = Some(args[i].as_str()); }
                    "--replace" if !replace => replace = true,
                    _ => return Err(format!("invalid learn option: {}", args[i])),
                }
                i += 1;
            }
            let draft = learn(&read_snapshot(&args[1])?, &read_snapshot(&args[2])?);
            if let Some(name) = name {
                if args[3] == "-" { return Err("named learning requires a config file".into()); }
                let existing = match fs::read_to_string(&args[3]) {
                    Ok(text) => Some(text),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => return Err(e.to_string()),
                };
                let config = match &existing {
                    Some(text) => parse_json(text)?,
                    None => Config { schema_version: 1, poll_seconds: 5, autolearn: true, notify: true, ignored_connections: vec![], payload: Default::default(), profiles: vec![] },
                };
                let merged = merge_profile(config, draft, name, replace)?;
                save_config(&args[3], existing.as_deref(), &merged)?;
                println!("Saved profile {name:?}; review learned rules before tracking.");
                Ok(())
            } else {
                if replace { return Err("--replace requires --name".into()); }
                write_json(&args[3], &draft)
            }
        }
        Some("name") if (args.len() == 5 || (args.len() == 6 && args[5] == "--replace")) && args[3] == "--name" => management::name_connection(&args[1], &args[2], &args[4], args.len() == 6),
        Some("payload") if args.len() == 2 => write_json("-", &read_config(&args[1])?.payload),
        Some("payload") if args.len() == 4 && args[2] == "--base64" => {
            let bytes = base64::engine::general_purpose::STANDARD.decode(&args[3]).map_err(|e| e.to_string())?;
            let settings = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            management::set_payload(&args[1], settings)
        },
        Some("connections") if args.len()==2 => write_json("-",&management::connections(&args[1])?),
        Some("rename") if args.len()==5 && args[3]=="--name" => management::rename_profile(&args[1],&args[2],&args[4]),
        Some("unnamed") if args.len() == 2 => write_json("-", &management::unnamed(&args[1])?),
        Some("ignore") | Some("unignore") if args.len() == 4 && args[2] == "--name" => management::set_ignored(&args[1], &args[3], args[0] == "ignore"),
        Some("manage") if args.len() == 2 => { read_config(&args[1])?; management::launch(&args[1], false) },
        Some("manage") if args.len() == 1 => { let path = host::default_config(false)?; host::init_config(&path)?; management::launch(path.to_str().ok_or("invalid config path")?, false) },
        Some("run") if args.len() >= 2 => {
            for a in &args[2..] { if !["--once", "--testing", "--dry-run", "--no-autolearn", "--no-notify"].contains(&a.as_str()) { return Err(format!("unknown option: {a}")); } }
            run(&args[1], args.iter().any(|s| s == "--once"), args.iter().any(|s| s == "--testing"), args.iter().any(|s| s == "--dry-run"), !args.iter().any(|s| s == "--no-autolearn"), !args.iter().any(|s| s == "--no-notify"))
        }
        _ => Err("Usage: aw-watcher-vpn snapshot <new-file|-> | evaluate <config> <snapshot> | learn <before> <after> <config> --name <customer> [--replace] | name <config> <connection-id> --name <customer> | run <config> [--dry-run] [--testing] [--once] [--no-autolearn] [--no-notify]".into()),
    }
}

fn merge_profile(
    mut config: Config,
    mut draft: Config,
    name: &str,
    replace: bool,
) -> Result<Config, String> {
    config.validate()?;
    let name = name.trim();
    if name.is_empty() {
        return Err("customer name cannot be empty".into());
    }
    if draft.profiles.len() != 1 {
        return Err(format!("Learning found {} changed adapters; capture one VPN change at a time (no config written)", draft.profiles.len()));
    }
    let mut profile = draft.profiles.remove(0);
    profile.customer_hint = name.into();
    let existing: Vec<usize> = config
        .profiles
        .iter()
        .enumerate()
        .filter(|(_, p)| p.customer_hint.trim().to_lowercase() == name.to_lowercase())
        .map(|(i, _)| i)
        .collect();
    if existing.len() > 1 {
        return Err("multiple existing profiles share this name; resolve them manually".into());
    }
    if let Some(&i) = existing.first() {
        if !replace {
            return Err(format!(
                "Customer {name:?} already exists; use --replace to relearn it"
            ));
        }
        profile.id = config.profiles[i].id.clone();
        profile.destinations = config.profiles[i].destinations.clone();
        config.profiles[i] = profile;
    } else {
        let slug: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let slug = slug
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base: String = if slug.is_empty() {
            "customer".into()
        } else {
            slug.chars().take(50).collect()
        };
        let mut id = base.clone();
        let mut n = 2;
        while config.profiles.iter().any(|p| p.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        profile.id = id;
        config.profiles.push(profile);
    }
    config.validate()?;
    Ok(config)
}

fn save_config(
    path: &str,
    expected: Option<&str>,
    config: &impl serde::Serialize,
) -> Result<(), String> {
    save_json(path, expected, config, true)
}
fn save_json(
    path: &str,
    expected: Option<&str>,
    config: &impl serde::Serialize,
    keep_backup: bool,
) -> Result<(), String> {
    use std::io::Write;
    use std::path::Path;
    let target = Path::new(path);
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let filename = target
        .file_name()
        .ok_or("invalid config path")?
        .to_string_lossy();
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let tmp = parent.join(format!(".{filename}.{nonce}.tmp"));
    let backup = parent.join(format!("{filename}.{nonce}.bak"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(
            serde_json::to_string_pretty(config)
                .map_err(|e| e.to_string())?
                .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        let current = match fs::read_to_string(target) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if current.as_deref() != expected {
            return Err("config changed during learning; retry (no overwrite)".into());
        }
        atomic_config_swap(
            target,
            &tmp,
            if keep_backup { Some(&backup) } else { None },
            expected.is_some(),
        )?;
        if expected.is_some() && keep_backup {
            println!("Previous config backed up to {}", backup.display());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(windows)]
fn atomic_config_swap(
    target: &std::path::Path,
    tmp: &std::path::Path,
    backup: Option<&std::path::Path>,
    exists: bool,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *mut std::ffi::c_void,
            reserved: *mut std::ffi::c_void,
        ) -> i32;
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let wide = |p: &std::path::Path| {
        p.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let t = wide(target);
    let s = wide(tmp);
    let b = backup.map(wide);
    let success = unsafe {
        if exists {
            ReplaceFileW(
                t.as_ptr(),
                s.as_ptr(),
                b.as_ref().map_or(std::ptr::null(), |b| b.as_ptr()),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        } else {
            MoveFileExW(s.as_ptr(), t.as_ptr(), 0)
        }
    };
    if success == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn atomic_config_swap(
    target: &std::path::Path,
    tmp: &std::path::Path,
    backup: Option<&std::path::Path>,
    exists: bool,
) -> Result<(), String> {
    if exists {
        if let Some(backup) = backup {
            fs::copy(target, backup).map_err(|e| e.to_string())?;
        }
        fs::rename(tmp, target).map_err(|e| e.to_string())
    } else {
        fs::hard_link(tmp, target).map_err(|e| e.to_string())?;
        fs::remove_file(tmp).map_err(|e| e.to_string())
    }
}
fn main() {
    if let Err(e) = main_result() {
        diag!("{e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn draft() -> Config {
        Config {
            schema_version: 1,
            poll_seconds: 5,
            autolearn: true,
            notify: true,
            ignored_connections: vec![],
            payload: Default::default(),
            profiles: vec![aw_watcher_vpn::Profile {
                id: "draft-1".into(),
                customer_hint: "draft".into(),
                rules: aw_watcher_vpn::Rules {
                    adapter_ids: vec!["synthetic".into()],
                    ..Default::default()
                },
                destinations: vec![],
            }],
        }
    }
    #[test]
    fn named_learning_merges_and_replaces_without_losing_settings() {
        let mut config = draft();
        config.profiles.clear();
        config.poll_seconds = 9;
        let mut config = merge_profile(config, draft(), "Customer A", false).unwrap();
        config.profiles[0].destinations.push("192.0.2.1".into());
        let config = merge_profile(config, draft(), "Customer B", false).unwrap();
        assert_eq!(config.profiles.len(), 2);
        assert_eq!(config.poll_seconds, 9);
        assert!(merge_profile(config.clone(), draft(), " customer a ", false).is_err());
        let config = merge_profile(config, draft(), " CUSTOMER A ", true).unwrap();
        assert_eq!(config.profiles[0].id, "customer-a");
        assert_eq!(config.profiles[0].destinations, vec!["192.0.2.1"]);
        assert_eq!(config.profiles[1].customer_hint, "Customer B");
    }
    #[test]
    fn learning_rejects_ambiguity_and_generates_unique_ids() {
        let mut empty = draft();
        empty.profiles.clear();
        assert!(merge_profile(empty.clone(), draft(), " ", false).is_err());
        assert!(merge_profile(empty.clone(), empty.clone(), "A", false).is_err());
        let mut multiple = draft();
        multiple.profiles.push(multiple.profiles[0].clone());
        assert!(merge_profile(empty.clone(), multiple, "A", false).is_err());
        let config = merge_profile(empty, draft(), "A B", false).unwrap();
        let config = merge_profile(config, draft(), "A-B", false).unwrap();
        assert_eq!(config.profiles[1].id, "a-b-2");
    }
    #[test]
    fn atomic_save_keeps_backup_and_rejects_stale_contents() {
        let dir = std::env::temp_dir().join(format!(
            "aw-vpn-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("config.json");
        let path_str = path.to_str().unwrap();
        save_config(path_str, None, &draft()).unwrap();
        let original = fs::read_to_string(&path).unwrap();
        let mut updated = draft();
        updated.poll_seconds = 8;
        save_config(path_str, Some(&original), &updated).unwrap();
        assert!(save_config(path_str, Some(&original), &draft()).is_err());
        assert_eq!(read_config(path_str).unwrap().poll_seconds, 8);
        let files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(files.len(), 2);
        let backup = files
            .iter()
            .find(|p| p.extension().is_some_and(|e| e == "bak"))
            .unwrap();
        assert_eq!(fs::read_to_string(backup).unwrap(), original);
        for file in files {
            fs::remove_file(file).unwrap();
        }
        fs::remove_dir(dir).unwrap();
    }

    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
    };
    fn connection() -> Connection {
        Connection {
            id: "sample".into(),
            name: "Synthetic VPN".into(),
            customers: vec!["Synthetic A".into()],
            suggested_name: None,
        }
    }
    #[test]
    fn loopback_aw_protocol_and_fifo_retry() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            for i in 0..4 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if line.to_lowercase().starts_with("content-length:") {
                        len = line.split(':').nth(1).unwrap().trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                requests.push((
                    first,
                    serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                ));
                let code = if i == 1 {
                    "503 Service Unavailable"
                } else {
                    "200 OK"
                };
                write!(
                    socket,
                    "HTTP/1.1 {code}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
                )
                .unwrap();
            }
            requests
        });
        let mut aw = Adapter::new(port, "synthetic".into(), 5);
        aw.payload = aw_watcher_vpn::PayloadSettings {
            mode: aw_watcher_vpn::PayloadMode::Extended,
            schema_version: true,
            connection_names: true,
            customers: true,
            ..Default::default()
        };
        let mut second = connection();
        second.id = "second".into();
        second.customers.clear();
        aw.enqueue(Utc::now(), Context::new(vec![connection(), second.clone()]));
        aw.enqueue(Utc::now(), Context::new(vec![second]));
        aw.flush();
        assert_eq!(aw.queue.len(), 2);
        aw.flush();
        assert!(aw.queue.is_empty());
        let requests = server.join().unwrap();
        assert!(requests[0]
            .0
            .contains("/api/0/buckets/aw-watcher-vpn_synthetic "));
        assert_eq!(requests[0].1["type"], "vpn.context");
        assert!(requests[2].0.contains("heartbeat?pulsetime=6"));
        assert_eq!(requests[2].1["data"]["schema_version"], 3);
        assert_eq!(requests[2].1["data"]["title"], "Synthetic A|Synthetic VPN");
        assert_eq!(
            requests[2].1["data"]["connections"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            requests[3].1["data"]["connections"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            requests[3].1["data"]["connections"][0]["customers"],
            json!([])
        );
        assert!(requests[3]
            .0
            .contains("/buckets/aw-watcher-vpn_synthetic/heartbeat"));
    }
    #[test]
    fn queue_is_bounded_and_expires() {
        let mut aw = Adapter::new(1, "synthetic".into(), 5);
        for _ in 0..513 {
            aw.enqueue(
                Utc::now() - chrono::Duration::seconds(301),
                Context::new(vec![connection()]),
            );
        }
        assert_eq!(aw.queue.len(), 512);
        aw.flush();
        assert_eq!(aw.queue.len(), 502);
    }
    #[test]
    fn no_connections_produces_no_aw_requests() {
        let mut aw = Adapter::new(1, "synthetic".into(), 5);
        aw.enqueue(Utc::now(), Context::new(vec![]));
        aw.flush();
        assert!(aw.queue.is_empty());
        assert!(aw.created.is_empty());
    }
    #[test]
    fn title_is_alphabetical_unique_and_preserves_unnamed_connections() {
        let mut a = connection();
        a.customers = vec!["zeta".into(), " Alpha ".into(), "Beta".into()];
        let mut b = connection();
        b.customers = vec!["alpha".into()];
        let mut unknown = connection();
        unknown.customers.clear();
        unknown.name = "Unknown VPN".into();
        let context = Context::new(vec![a.clone(), b.clone(), unknown.clone()]);
        assert_eq!(context.title, "Alpha|Beta|Unknown VPN|zeta");
        assert_eq!(Context::new(vec![unknown, b, a]).title, context.title);
        assert!(Context::new(vec![]).title.is_empty());
        let old: Context = parse_json(r#"{"schema_version":3,"connections":[]}"#).unwrap();
        assert!(old.title.is_empty());
    }
    // Each scripted server has a bounded accept deadline so regressions fail instead of hanging.
    fn protocol_server(
        listener: TcpListener,
        replies: Vec<(u16, serde_json::Value)>,
    ) -> thread::JoinHandle<Vec<(String, serde_json::Value)>> {
        thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            let mut requests = vec![];
            for (status, body) in replies {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "Expected HTTP request did not arrive"
                            );
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if line.to_lowercase().starts_with("content-length:") {
                        len = line.split(':').nth(1).unwrap().trim().parse().unwrap();
                    }
                }
                let mut bytes = vec![0; len];
                reader.read_exact(&mut bytes).unwrap();
                requests.push((
                    first,
                    if bytes.is_empty() {
                        serde_json::Value::Null
                    } else {
                        serde_json::from_slice(&bytes).unwrap()
                    },
                ));
                let body = if status == 304 {
                    String::new()
                } else {
                    body.to_string()
                };
                write!(socket, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            requests
        })
    }
    fn metadata() -> serde_json::Value {
        json!({"type":"vpn.context"})
    }
    #[test]
    fn existing_bucket_304_and_producer_restart_deliver_heartbeats() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = protocol_server(
            listener,
            vec![
                (304, json!({})),
                (200, metadata()),
                (200, json!({})),
                (304, json!({})),
                (200, metadata()),
                (200, json!({})),
            ],
        );
        for _ in 0..2 {
            let mut aw = Adapter::new(port, "synthetic".into(), 5);
            aw.enqueue(Utc::now(), Context::new(vec![connection()]));
            aw.flush();
            assert!(aw.queue.is_empty());
        }
        let requests = server.join().unwrap();
        assert!(requests[1].0.starts_with("GET "));
        for i in [2, 5] {
            assert!(requests[i].0.contains("/heartbeat?pulsetime=6"));
            assert_eq!(requests[i].1["data"]["title"], "Synthetic A");
        }
    }
    #[test]
    fn server_starting_late_retains_observation_timestamp() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let mut aw = Adapter::new(addr.port(), "synthetic".into(), 5);
        let timestamp = Utc::now() - chrono::Duration::seconds(20);
        aw.enqueue(timestamp, Context::new(vec![connection()]));
        aw.flush();
        assert_eq!(aw.queue.len(), 1);
        let server = protocol_server(
            TcpListener::bind(addr).unwrap(),
            vec![(200, json!({})), (200, json!({}))],
        );
        aw.flush();
        assert!(aw.queue.is_empty());
        let requests = server.join().unwrap();
        assert_eq!(requests[1].1["timestamp"], timestamp.to_rfc3339());
        assert_eq!(requests[1].1["duration"], 0);
    }
    #[test]
    fn outage_and_server_restart_recreate_missing_bucket_and_resume() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = protocol_server(
            listener,
            vec![(200, json!({})), (200, json!({})), (503, json!({}))],
        );
        let mut aw = Adapter::new(addr.port(), "synthetic".into(), 5);
        let before = Utc::now() - chrono::Duration::seconds(60);
        aw.enqueue(before, Context::new(vec![connection()]));
        aw.flush();
        let after = before + chrono::Duration::seconds(30);
        aw.enqueue(after, Context::new(vec![connection()]));
        aw.flush();
        assert_eq!(aw.queue.len(), 1);
        let initial = server.join().unwrap();
        aw.flush(); // Server process has stopped, keep the same observation.
        assert_eq!(aw.queue.len(), 1);
        let server = protocol_server(
            TcpListener::bind(addr).unwrap(),
            vec![(404, json!({})), (200, json!({})), (200, json!({}))],
        );
        aw.flush();
        assert!(aw.created.is_empty());
        assert_eq!(aw.queue.len(), 1);
        aw.flush();
        assert!(aw.queue.is_empty());
        let resumed = server.join().unwrap();
        assert_eq!(resumed[2].1, initial[2].1);
        assert_eq!(resumed[2].1["timestamp"], after.to_rfc3339());
        assert_eq!(resumed[2].1["duration"], 0);
        // No stale interval is extended over the 30-second missing-observation gap.
        assert_eq!(initial[1].1["timestamp"], before.to_rfc3339());
        assert!(resumed[2].0.contains("pulsetime=6"));
    }
    #[test]
    fn creation_failure_and_incompatible_existing_bucket_block_delivery() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = protocol_server(
            listener,
            vec![
                (500, json!({})),
                (304, json!({})),
                (200, json!({"type":"wrong.type"})),
                (304, json!({})),
                (200, metadata()),
                (200, json!({})),
            ],
        );
        let mut aw = Adapter::new(port, "synthetic".into(), 5);
        aw.enqueue(Utc::now(), Context::new(vec![connection()]));
        for _ in 0..2 {
            aw.flush();
            assert_eq!(aw.queue.len(), 1);
            assert!(aw.created.is_empty());
        }
        aw.flush();
        assert!(aw.queue.is_empty());
        let requests = server.join().unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.0.contains("/heartbeat"))
                .count(),
            1
        );
    }
    #[test]
    fn redirects_and_heartbeat_304_are_not_delivery_success() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = protocol_server(
            listener,
            vec![
                (301, json!({})),
                (200, json!({})),
                (304, json!({})),
                (200, json!({})),
            ],
        );
        let mut aw = Adapter::new(port, "synthetic".into(), 5);
        aw.enqueue(Utc::now(), Context::new(vec![connection()]));
        aw.flush();
        assert!(aw.created.is_empty());
        assert_eq!(aw.queue.len(), 1);
        aw.flush();
        assert_eq!(aw.queue.len(), 1);
        aw.flush();
        assert!(aw.queue.is_empty());
        let requests = server.join().unwrap();
        assert_eq!(requests[2].1, requests[3].1);
    }
    #[test]
    fn unavailable_context_has_neutral_title() {
        let mut unknown = connection();
        unknown.customers.clear();
        unknown.name = " ".into();
        assert_eq!(
            Context::new(vec![unknown]).title,
            "Activity (context unavailable)"
        );
    }
    #[test]
    fn privacy_reload_discards_pending_samples_and_settings_roundtrip() {
        let mut aw = Adapter::new(1, "synthetic".into(), 5);
        aw.enqueue(Utc::now(), Context::new(vec![connection()]));
        aw.set_payload(&aw_watcher_vpn::PayloadSettings {
            customers: false,
            ..Default::default()
        });
        assert!(aw.queue.is_empty());
        let path = std::env::temp_dir().join(format!(
            "vpn-payload-{}.json",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let path = path.to_str().unwrap();
        write_json(path, &draft()).unwrap();
        let settings = aw.payload.clone();
        management::set_payload(path, settings.clone()).unwrap();
        let config = read_config(path).unwrap();
        assert_eq!(config.payload, settings);
        assert_eq!(
            config.profiles[0].customer_hint,
            draft().profiles[0].customer_hint
        );
        assert!(
            serde_json::from_str::<aw_watcher_vpn::PayloadSettings>(r#"{"mode":"unknown"}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<aw_watcher_vpn::PayloadSettings>(r#"{"secret":true}"#).is_err()
        );
        fs::remove_file(path).unwrap();
        let prefix = std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for entry in fs::read_dir(std::env::temp_dir()).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{prefix}.")) && name.ends_with(".bak") {
                fs::remove_file(entry.path()).unwrap();
            }
        }
    }
}
