//! ActivityWatch launcher/configuration adapter. Detection remains in the core.
use super::*;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::OnceLock,
};
static LOG: OnceLock<PathBuf> = OnceLock::new();

fn config_at(base: &Path, profile: &str, testing: bool) -> Result<PathBuf, String> {
    if !profile
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("invalid AW_PROFILE".into());
    }
    let app = if profile.is_empty() || profile == "default" {
        "activitywatch".into()
    } else {
        format!("activitywatch-{profile}")
    };
    #[cfg(windows)]
    let root = base.join(&app).join(&app);
    #[cfg(not(windows))]
    let root = base.join(&app);
    Ok(root.join("aw-watcher-vpn").join(if testing {
        "config-testing.json"
    } else {
        "config.json"
    }))
}
pub fn default_config(testing: bool) -> Result<PathBuf, String> {
    #[cfg(windows)]
    let base = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA missing")?);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    config_at(
        &base,
        &std::env::var("AW_PROFILE").unwrap_or_default(),
        testing,
    )
}
pub fn init_config(path: &Path) -> Result<(), String> {
    let config = Config {
        schema_version: 1,
        poll_seconds: 5,
        autolearn: true,
        notify: true,
        ignored_connections: vec![],
        payload: Default::default(),
        profiles: vec![],
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => f
            .write_all(
                serde_json::to_string_pretty(&config)
                    .map_err(|e| e.to_string())?
                    .as_bytes(),
            )
            .map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
pub fn instance_lock(path: &str) -> Result<fs::File, String> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    options.open(format!("{path}.lock")).map_err(|e| {
        if e.raw_os_error() == Some(32) {
            "watcher already running for this config".into()
        } else {
            format!("cannot lock module config: {e}")
        }
    })
}
pub fn record(message: &str) {
    eprintln!("{message}");
    if let Some(path) = LOG.get() {
        if fs::metadata(path).is_ok_and(|m| m.len() >= 1_048_576) {
            let _ = fs::rename(path, path.with_extension("log.1"));
        }
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{} {message}", Utc::now().to_rfc3339());
        }
    }
}

#[derive(Default)]
struct Options {
    config: Option<PathBuf>,
    testing: bool,
    once: bool,
    dry_run: bool,
    init: bool,
    path_only: bool,
    validate: bool,
    no_autolearn: bool,
    no_notify: bool,
}
impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut o = Self::default();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--config" if o.config.is_none() && i + 1 < args.len() => {
                    i += 1;
                    o.config = Some(PathBuf::from(&args[i]));
                }
                "--testing" => o.testing = true,
                "--once" => o.once = true,
                "--dry-run" => o.dry_run = true,
                "--init-config" => o.init = true,
                "--config-path" => o.path_only = true,
                "--validate-config" => o.validate = true,
                "--no-autolearn" => o.no_autolearn = true,
                "--no-notify" => o.no_notify = true,
                _ => return Err(format!("unknown or incomplete module option: {}", args[i])),
            }
            i += 1;
        }
        Ok(o)
    }
}
pub fn log_path(path: &Path, testing: bool, custom: bool) -> Result<PathBuf, String> {
    Ok(if custom {
        path.with_extension("log")
    } else {
        path.parent()
            .and_then(Path::parent)
            .ok_or("config parent missing")?
            .join("Logs")
            .join("aw-watcher-vpn")
            .join(if testing {
                "aw-watcher-vpn-testing.log"
            } else {
                "aw-watcher-vpn.log"
            })
    })
}
pub fn launch(args: &[String]) -> Result<(), String> {
    if args == ["--help"] {
        println!("aw-watcher-vpn [--config PATH] [--testing] [--once] [--dry-run] [--no-autolearn] [--no-notify]\nNo arguments: run as an ActivityWatch module.\n--init-config: initialize config without overwriting and print its path.\n--config-path: print config path without writing.\n--validate-config: validate settings without collection or AW requests.\nconfigure / open-logs / about [--config PATH] [--testing]: tray actions with manual fallback.\nmanage [CONFIG]: open unnamed connections.\nLegacy snapshot/evaluate/learn/name/run commands remain available.");
        return Ok(());
    }
    if args == ["--version"] {
        println!("aw-watcher-vpn {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let mut o = Options::parse(args)?;
    o.testing = actions::testing(o.testing)?;
    let custom = o.config.is_some();
    let path = match o.config {
        Some(path) => path,
        None => default_config(o.testing)?,
    };
    let path_str = path.to_str().ok_or("config path is not valid UTF-8")?;
    if o.path_only {
        println!("{}", path.display());
        return Ok(());
    }
    if o.validate {
        read_config(path_str)?;
        println!("Config valid");
        return Ok(());
    }
    init_config(&path)?;
    if o.init {
        println!("{}", path.display());
        return Ok(());
    }
    let log = log_path(&path, o.testing, custom)?;
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let _ = LOG.set(log);
    record(&format!(
        "Starting AW module v{}",
        env!("CARGO_PKG_VERSION")
    ));
    loop {
        match run(
            path_str,
            o.once,
            o.testing,
            o.dry_run,
            !o.no_autolearn,
            !o.no_notify,
        ) {
            Ok(()) => return Ok(()),
            Err(e) if o.once || o.dry_run || e == "watcher already running for this config" => {
                return Err(e)
            }
            Err(e) => {
                record(&format!(
                    "Module startup failed: {e}; retrying in 10 seconds"
                ));
                thread::sleep(Duration::from_secs(10));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn module_options_and_profile_paths_are_isolated() {
        let args = vec![
            "--testing".into(),
            "--no-notify".into(),
            "--config".into(),
            "synthetic.json".into(),
        ];
        let o = Options::parse(&args).unwrap();
        assert!(o.testing && o.no_notify);
        assert_eq!(o.config.unwrap(), PathBuf::from("synthetic.json"));
        assert!(Options::parse(&["--config".into()]).is_err());
        let normal = config_at(Path::new("synthetic"), "default", false).unwrap();
        let testing = config_at(Path::new("synthetic"), "work", true).unwrap();
        assert_ne!(normal, testing);
        assert!(testing.ends_with("aw-watcher-vpn/config-testing.json"));
        assert!(config_at(Path::new("synthetic"), "../private", false).is_err());
    }
    #[test]
    fn module_init_preserves_existing_settings() {
        let dir = std::env::temp_dir().join(format!(
            "aw-vpn-host-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let path = dir.join("config.json");
        init_config(&path).unwrap();
        let mut config = read_config(path.to_str().unwrap()).unwrap();
        assert!(config.profiles.is_empty());
        assert!(config.autolearn && config.notify);
        config.notify = false;
        fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();
        init_config(&path).unwrap();
        assert!(!read_config(path.to_str().unwrap()).unwrap().notify);
        #[cfg(windows)]
        {
            let first = instance_lock(path.to_str().unwrap()).unwrap();
            assert!(instance_lock(path.to_str().unwrap()).is_err());
            drop(first);
        }
        fs::remove_file(&path).unwrap();
        let _ = fs::remove_file(format!("{}.lock", path.display()));
        fs::remove_dir(dir).unwrap();
    }
}
