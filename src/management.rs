//! Local management commands. A desktop process can use these while the watcher
//! runs as an AW module or in a different Windows session.
use super::*;

#[derive(serde::Serialize)]
pub struct UnnamedConnection {
    id: String,
    name: String,
    suggested_name: Option<String>,
    existing_customers: Vec<String>,
    active: bool,
}
#[derive(serde::Serialize)]
pub struct Unnamed {
    connections: Vec<UnnamedConnection>,
    customers: Vec<String>,
    notify: bool,
    ignored_connections: Vec<String>,
}
fn naming_rules(d: &Discovered) -> aw_watcher_vpn::Rules {
    let mut rules = d.rules.clone();
    // Upgrade the first discovery format, which stored only a transient GUID.
    if rules.adapter_names.is_empty()
        && rules.adapter_ids == vec![d.id.clone()]
        && rules.description_contains.is_empty()
        && rules.dns_suffixes.is_empty()
        && rules.dns_servers.is_empty()
        && rules.route_prefixes.is_empty()
    {
        rules.adapter_ids.clear();
        rules.adapter_names = vec![d.name.clone()];
    }
    rules
}

pub fn unnamed(path: &str) -> Result<Unnamed, String> {
    let config = read_config(path)?;
    let discovered = read_discovered(&format!("{path}.discovered.json"))?;
    let context: Option<Context> = match fs::read_to_string(format!("{path}.context.json")) {
        Ok(text) => parse_json(&text)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    let mut connections = vec![];
    for d in discovered {
        if aw_watcher_vpn::connection_ignored(&config, &d.name) {
            continue;
        }
        let active = context
            .as_ref()
            .and_then(|c| c.connections.iter().find(|c| c.id == d.id));
        // A named active connection, or a discovery explicitly promoted by the
        // naming command, is resolved. Strict older rules remain reviewable.
        if active.is_some_and(|c| !c.customers.is_empty())
            || config.profiles.iter().any(|p| p.rules == naming_rules(&d))
        {
            continue;
        }
        let mut existing_customers: Vec<_> = config
            .profiles
            .iter()
            .filter(|p| {
                p.rules
                    .adapter_ids
                    .iter()
                    .any(|id| id.eq_ignore_ascii_case(&d.id))
                    || p.rules
                        .adapter_names
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(&d.name))
                    || (d.name.len() >= 3
                        && p.customer_hint
                            .to_lowercase()
                            .contains(&d.name.to_lowercase()))
            })
            .map(|p| p.customer_hint.clone())
            .collect();
        existing_customers.sort();
        existing_customers.dedup();
        connections.push(UnnamedConnection {
            id: d.id,
            name: d.name,
            suggested_name: d.suggested_name,
            existing_customers,
            active: active.is_some(),
        });
    }
    connections.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut customers: Vec<_> = config
        .profiles
        .iter()
        .map(|p| p.customer_hint.clone())
        .collect();
    customers.sort();
    customers.dedup();
    Ok(Unnamed {
        connections,
        customers,
        notify: config.notify,
        ignored_connections: config.ignored_connections,
    })
}

pub fn name_connection(path: &str, id: &str, name: &str, replace: bool) -> Result<(), String> {
    let discovered = read_discovered(&format!("{path}.discovered.json"))?;
    let d = discovered
        .iter()
        .find(|d| d.id == id)
        .ok_or("unknown discovery ID; run with auto-learning enabled first")?;
    let existing = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let config = parse_json(&existing)?;
    let draft = Config {
        schema_version: 1,
        poll_seconds: 5,
        autolearn: true,
        notify: true,
        ignored_connections: vec![],
        profiles: vec![aw_watcher_vpn::Profile {
            id: "draft".into(),
            customer_hint: String::new(),
            rules: naming_rules(d),
            destinations: vec![],
        }],
    };
    let merged = merge_profile(config, draft, name, replace)?;
    save_config(path, Some(&existing), &merged)
}

pub fn save_context(path: &str, context: &Context) -> Result<(), String> {
    save_context_value(path, Some(context))
}
pub fn set_ignored(path: &str, name: &str, ignored: bool) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("connection name cannot be empty".into());
    }
    let existing = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut config: Config = parse_json(&existing)?;
    config.validate()?;
    let before = config.ignored_connections.clone();
    if ignored {
        if !aw_watcher_vpn::connection_ignored(&config, name) {
            config.ignored_connections.push(name.into());
        }
    } else {
        config
            .ignored_connections
            .retain(|n| !n.trim().eq_ignore_ascii_case(name));
    }
    if before == config.ignored_connections {
        return Ok(());
    }
    config.validate()?;
    save_config(path, Some(&existing), &config)
}
pub fn clear_context(path: &str) -> Result<(), String> {
    save_context_value(path, None)
}
fn save_context_value(path: &str, context: Option<&Context>) -> Result<(), String> {
    let path = format!("{path}.context.json");
    let existing = match fs::read_to_string(&path) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    save_json(&path, existing.as_deref(), &context, false)
}

#[cfg(windows)]
fn spawn_script(script: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let exe =
        std::path::PathBuf::from(std::env::var_os("SystemRoot").ok_or("SystemRoot unavailable")?)
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    Command::new(exe)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-STA",
            "-EncodedCommand",
            &base64::engine::general_purpose::STANDARD.encode(bytes),
        ])
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
#[cfg(windows)]
pub fn launch(path: &str, notification: bool) -> Result<(), String> {
    let config_path = fs::canonicalize(path).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let decode = |s: &str| {
        format!(
            "[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{}'))",
            base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
        )
    };
    let bootstrap = format!(
        "$configPath = {}; $watcherExe = {}; $notificationMessage = {}; $watcherPid = {};\n",
        decode(&config_path.to_string_lossy()),
        decode(&exe.to_string_lossy()),
        if notification { "$true" } else { "$false" },
        if notification { std::process::id() } else { 0 }
    );
    spawn_script(&(bootstrap + include_str!("../scripts/manage.ps1")))
}
#[cfg(not(windows))]
pub fn launch(_path: &str, _notification: bool) -> Result<(), String> {
    Err("management window requires Windows".into())
}

#[cfg(windows)]
pub fn notify_only(name: &str) -> Result<(), String> {
    let message = base64::engine::general_purpose::STANDARD.encode(format!("Unnamed VPN connection: {name}. Auto-learning is disabled; use before/after learning to name it.").as_bytes());
    spawn_script(&format!("Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $n = New-Object System.Windows.Forms.NotifyIcon; $n.Icon = [System.Drawing.SystemIcons]::Information; $n.Visible = $true; $m = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{message}')); $n.ShowBalloonTip(5000, 'ActivityWatch VPN', $m, [System.Windows.Forms.ToolTipIcon]::Info); Start-Sleep -Seconds 6; $n.Dispose()"))
}
#[cfg(not(windows))]
pub fn notify_only(_name: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn management_lists_all_pending_and_reuses_existing_customer() {
        let dir = std::env::temp_dir().join(format!(
            "aw-vpn-manage-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("config.json");
        let path = path.to_str().unwrap();
        let mut config = super::super::tests::draft();
        config.profiles[0].customer_hint = "Existing Customer".into();
        config.profiles[0].rules.route_prefixes = vec!["192.0.2.1/32".into()];
        config.profiles[0].destinations = vec!["192.0.2.1".into()];
        save_config(path, None, &config).unwrap();
        let discoveries = vec![
            Discovered {
                id: "synthetic".into(),
                name: "VPN A".into(),
                rules: aw_watcher_vpn::Rules {
                    adapter_ids: vec!["synthetic".into()],
                    ..Default::default()
                },
                suggested_name: None,
            },
            Discovered {
                id: "other".into(),
                name: "VPN B".into(),
                rules: aw_watcher_vpn::Rules {
                    adapter_ids: vec!["other".into()],
                    ..Default::default()
                },
                suggested_name: None,
            },
        ];
        save_config(&format!("{path}.discovered.json"), None, &discoveries).unwrap();
        let pending = unnamed(path).unwrap();
        assert_eq!(pending.connections.len(), 2);
        assert_eq!(
            pending.connections[0].existing_customers,
            vec!["Existing Customer"]
        );
        assert!(name_connection(path, "synthetic", "Existing Customer", false).is_err());
        name_connection(path, "synthetic", "Existing Customer", true).unwrap();
        let updated = read_config(path).unwrap();
        assert_eq!(updated.profiles.len(), 1);
        assert_eq!(updated.profiles[0].id, config.profiles[0].id);
        assert_eq!(
            updated.profiles[0].destinations,
            config.profiles[0].destinations
        );
        assert_eq!(unnamed(path).unwrap().connections.len(), 1);
        save_context(
            path,
            &Context::new(vec![Connection {
                id: "other".into(),
                name: "VPN B".into(),
                customers: vec!["Known now".into()],
                suggested_name: None,
            }]),
        )
        .unwrap();
        assert!(unnamed(path).unwrap().connections.is_empty());
        clear_context(path).unwrap();
        assert_eq!(unnamed(path).unwrap().connections.len(), 1);
        for file in fs::read_dir(&dir).unwrap() {
            fs::remove_file(file.unwrap().path()).unwrap();
        }
        fs::remove_dir(dir).unwrap();
    }
}
