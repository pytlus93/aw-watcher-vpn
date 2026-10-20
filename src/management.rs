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
    assigned: bool,
    ignored: bool,
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
            assigned: false,
            ignored: false,
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

pub fn connections(path: &str) -> Result<Unnamed, String> {
    let mut result = unnamed(path)?;
    let config = read_config(path)?;
    let context: Option<Context> = fs::read_to_string(format!("{path}.context.json"))
        .ok()
        .map(|s| parse_json(&s))
        .transpose()?
        .flatten();
    for p in &config.profiles {
        let name = p
            .rules
            .adapter_names
            .first()
            .cloned()
            .unwrap_or_else(|| p.customer_hint.clone());
        let ignored = aw_watcher_vpn::connection_ignored(&config, &name);
        let active = context.as_ref().is_some_and(|c| {
            c.connections.iter().any(|a| {
                a.customers.contains(&p.customer_hint)
                    && (p
                        .rules
                        .adapter_names
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&a.name))
                        || p.rules
                            .adapter_ids
                            .iter()
                            .any(|id| id.eq_ignore_ascii_case(&a.id)))
            })
        });
        result.connections.push(UnnamedConnection {
            id: format!("profile:{}", p.id),
            name,
            suggested_name: None,
            existing_customers: vec![p.customer_hint.clone()],
            active,
            assigned: true,
            ignored,
        });
    }
    for name in &config.ignored_connections {
        if result
            .connections
            .iter()
            .any(|c| c.name.trim().eq_ignore_ascii_case(name.trim()))
        {
            continue;
        }
        result.connections.push(UnnamedConnection {
            id: format!("ignored:{name}"),
            name: name.clone(),
            suggested_name: None,
            existing_customers: vec![],
            active: false,
            assigned: false,
            ignored: true,
        });
    }
    result.connections.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(result)
}
/// Forget one cached discovery without changing customer assignments or ignores.
pub fn delete_discovery(path: &str, id: &str) -> Result<(), String> {
    read_config(path)?.validate()?;
    let discovery_path = format!("{path}.discovered.json");
    let original = fs::read_to_string(&discovery_path).map_err(|e| e.to_string())?;
    let mut discoveries: Vec<Discovered> = parse_json(&original)?;
    let index = discoveries
        .iter()
        .position(|d| d.id == id)
        .ok_or("Unknown discovery ID; nothing deleted")?;
    discoveries.remove(index);
    save_config(&discovery_path, Some(&original), &discoveries)
}

/// Forget exactly one saved fingerprint; never delete customer siblings or AW events.
pub fn delete_profile(path: &str, id: &str) -> Result<(), String> {
    let original = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut config: Config = parse_json(&original)?;
    config.validate()?;
    let index = config
        .profiles
        .iter()
        .position(|p| p.id == id)
        .ok_or("Unknown profile ID; nothing deleted")?;
    let removed = config.profiles.remove(index);
    let discovery_path = format!("{path}.discovered.json");
    let discovery_original = match fs::read_to_string(&discovery_path) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    let mut discoveries = read_discovered(&discovery_path)?;
    discoveries.retain(|d| {
        let rules = naming_rules(d);
        let belongs = rules == removed.rules
            || removed
                .rules
                .adapter_ids
                .iter()
                .any(|id| id.eq_ignore_ascii_case(&d.id))
            || removed
                .rules
                .adapter_names
                .iter()
                .any(|n| n.eq_ignore_ascii_case(&d.name));
        !belongs || config.profiles.iter().any(|p| p.rules == rules)
    });
    config.validate()?;
    save_config(path, Some(&original), &config)?;
    if let Some(previous) = discovery_original {
        save_config(&discovery_path, Some(&previous), &discoveries)
            .map_err(|e| format!("Profile deleted, but discovery cleanup failed: {e}"))?;
    }
    Ok(())
}

pub fn rename_profile(path: &str, id: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Customer name cannot be empty".into());
    }
    let original = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut config: Config = parse_json(&original)?;
    let previous = config
        .profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or("Unknown profile ID")?
        .customer_hint
        .clone();
    for profile in &mut config.profiles {
        if profile
            .customer_hint
            .trim()
            .eq_ignore_ascii_case(previous.trim())
        {
            profile.customer_hint = name.into();
        }
    }
    config.validate()?;
    save_config(path, Some(&original), &config)
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
        payload: Default::default(),
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
pub fn set_payload(path: &str, settings: aw_watcher_vpn::PayloadSettings) -> Result<(), String> {
    let existing = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut config: Config = parse_json(&existing)?;
    config.payload = settings;
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
    owned_helper::detached(script)
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
#[cfg(windows)]
pub fn launch_owned(path: &str) -> Result<(), String> {
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
        "$false",
        0
    );
    owned_helper::run(&(bootstrap + "Remove-Item Env:AW_MODULE_ACTION_PROTOCOL,Env:AW_MODULE_ACTION -ErrorAction SilentlyContinue;\n" + include_str!("../scripts/manage.ps1")))
}

#[cfg(not(windows))]
pub fn launch_owned(_path: &str) -> Result<(), String> {
    Err("Action UI requires Windows".into())
}
#[cfg(not(windows))]
pub fn launch(_path: &str, _notification: bool) -> Result<(), String> {
    Err("management window requires Windows".into())
}

#[cfg(windows)]
pub fn notify_only(_name: &str) -> Result<(), String> {
    let message = base64::engine::general_purpose::STANDARD.encode(b"Unnamed connection detected. Auto-learning is disabled; use before/after learning to name it.");
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
    fn deleting_unnamed_discovery_keeps_config_and_other_discoveries() {
        let dir = std::env::temp_dir().join(format!(
            "aw-vpn-forget-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("config.json");
        let path = path.to_str().unwrap();
        save_config(path, None, &super::super::tests::draft()).unwrap();
        let config_before = fs::read(path).unwrap();
        let make = |id: &str| Discovered {
            id: id.into(),
            name: id.into(),
            rules: Default::default(),
            suggested_name: None,
        };
        let discovery_path = format!("{path}.discovered.json");
        save_config(&discovery_path, None, &vec![make("old"), make("other")]).unwrap();
        delete_discovery(path, "old").unwrap();
        assert_eq!(fs::read(path).unwrap(), config_before);
        let remaining = read_discovered(&discovery_path).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "other");
        let before = fs::read(&discovery_path).unwrap();
        assert!(delete_discovery(path, "missing").is_err());
        assert_eq!(fs::read(&discovery_path).unwrap(), before);
        assert!(fs::read_dir(&dir).unwrap().any(|f| f
            .unwrap()
            .path()
            .extension()
            .is_some_and(|x| x == "bak")));
        for file in fs::read_dir(&dir).unwrap() {
            fs::remove_file(file.unwrap().path()).unwrap();
        }
        fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn delete_one_fingerprint_preserves_customer_siblings_and_history() {
        let dir = std::env::temp_dir().join(format!(
            "aw-vpn-delete-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("config.json");
        let path = path.to_str().unwrap();
        let mut config = super::super::tests::draft();
        let mut sibling = config.profiles[0].clone();
        sibling.id = "bp".into();
        sibling.rules = aw_watcher_vpn::Rules {
            adapter_names: vec!["BP".into()],
            ..Default::default()
        };
        config.profiles.push(sibling);
        config.ignored_connections = vec!["Company".into()];
        save_config(path, None, &config).unwrap();
        let discovery = Discovered {
            id: "old-guid".into(),
            name: "BP".into(),
            rules: config.profiles[1].rules.clone(),
            suggested_name: None,
        };
        save_config(&format!("{path}.discovered.json"), None, &vec![discovery]).unwrap();
        let history = dir.join("history-sentinel.json");
        fs::write(&history, "historical events").unwrap();
        delete_profile(path, "bp").unwrap();
        let saved = read_config(path).unwrap();
        assert_eq!(saved.profiles.len(), 1);
        assert_eq!(saved.profiles[0].id, config.profiles[0].id);
        assert_eq!(saved.profiles[0].rules, config.profiles[0].rules);
        assert_eq!(saved.ignored_connections, config.ignored_connections);
        assert!(read_discovered(&format!("{path}.discovered.json"))
            .unwrap()
            .is_empty());
        assert_eq!(fs::read_to_string(&history).unwrap(), "historical events");
        assert!(delete_profile(path, "missing").is_err());
        assert_eq!(read_config(path).unwrap().profiles.len(), 1);
        delete_profile(path, &config.profiles[0].id).unwrap();
        assert!(read_config(path).unwrap().profiles.is_empty());
        assert!(fs::read_dir(&dir).unwrap().any(|f| f
            .unwrap()
            .path()
            .extension()
            .is_some_and(|x| x == "bak")));
        for file in fs::read_dir(&dir).unwrap() {
            fs::remove_file(file.unwrap().path()).unwrap();
        }
        fs::remove_dir(dir).unwrap();
    }
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
        set_ignored(path, "VPN B", true).unwrap();
        set_ignored(path, " vpn b ", true).unwrap();
        assert_eq!(
            read_config(path).unwrap().ignored_connections,
            vec!["VPN B"]
        );
        assert_eq!(unnamed(path).unwrap().connections.len(), 1);
        set_ignored(path, "VPN B", false).unwrap();
        assert_eq!(unnamed(path).unwrap().connections.len(), 2);
        assert_eq!(
            pending.connections[0].existing_customers,
            vec!["Existing Customer"]
        );
        name_connection(path, "synthetic", "Existing Customer", false).unwrap();
        assert_eq!(read_config(path).unwrap().profiles.len(), 2);
        assert_eq!(
            read_config(path).unwrap().profiles[0].rules,
            config.profiles[0].rules
        );
        // Explicit legacy replace refuses ambiguous customer-wide replacement.
        assert!(name_connection(path, "synthetic", "Existing Customer", true).is_err());
        rename_profile(path, &config.profiles[0].id, "Renamed group").unwrap();
        assert!(read_config(path)
            .unwrap()
            .profiles
            .iter()
            .all(|p| p.customer_hint == "Renamed group"));
        // Keep the remainder of the legacy management scenario single-profile.
        save_config(path, Some(&fs::read_to_string(path).unwrap()), &config).unwrap();
        name_connection(path, "synthetic", "Existing Customer", true).unwrap();
        let inventory = connections(path).unwrap();
        assert!(inventory
            .connections
            .iter()
            .any(|c| c.assigned && c.existing_customers == vec!["Existing Customer"]));
        set_ignored(path, "VPN A", true).unwrap();
        assert!(connections(path)
            .unwrap()
            .connections
            .iter()
            .any(|c| c.ignored && c.name == "VPN A"));
        set_ignored(path, "VPN A", false).unwrap();
        let saved_rules = read_config(path).unwrap().profiles[0].rules.clone();
        rename_profile(path, &config.profiles[0].id, "Renamed Customer").unwrap();
        assert_eq!(read_config(path).unwrap().profiles[0].rules, saved_rules);
        rename_profile(path, &config.profiles[0].id, "Existing Customer").unwrap();
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
