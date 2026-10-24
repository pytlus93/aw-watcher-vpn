//! Protocol-one dispatch before any producer lock or collection.
use super::*;
#[derive(Debug, serde::Serialize)]
pub struct LaunchContext {
    pub testing: bool,
    pub profile: String,
    pub server_url: String,
    pub config: std::path::PathBuf,
}
pub fn testing(cli: bool) -> Result<bool, String> {
    match std::env::var("AW_TESTING").ok().as_deref() {
        None => Ok(cli || std::env::var("AW_PROFILE").ok().as_deref() == Some("testing")),
        Some("1") => Ok(true),
        Some("0") => Ok(cli),
        _ => Err("AW_TESTING must be 0 or 1".into()),
    }
}
pub fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|p| *p > 0)
        .ok_or_else(|| "Port must be 1–65535".into())
}
pub fn server_url(testing: bool, port: Option<u16>) -> Result<String, String> {
    server_with_port(
        &std::env::var("AW_SERVER_URL")
            .unwrap_or_else(|_| format!("http://127.0.0.1:{}", if testing { 5666 } else { 5600 })),
        port,
    )
}
fn server_with_port(value: &str, port: Option<u16>) -> Result<String, String> {
    let server = validate_server_url(value)?;
    if let Some(port) = port {
        let authority = &server[7..];
        let host = authority
            .rsplit_once(':')
            .map(|(h, _)| h)
            .unwrap_or(authority);
        Ok(format!("http://{host}:{port}"))
    } else {
        Ok(server)
    }
}
fn validate_server_url(value: &str) -> Result<String, String> {
    let authority = value
        .strip_prefix("http://")
        .ok_or("Server URL must use local HTTP")?
        .trim_end_matches('/');
    let (host, port) = authority.rsplit_once(':').unwrap_or((authority, "80"));
    if !["127.0.0.1", "localhost", "[::1]"].contains(&host)
        || port.parse::<u16>().ok().filter(|p| *p > 0).is_none()
    {
        return Err("Server URL must be a loopback HTTP root with valid port".into());
    }
    Ok(format!("http://{authority}"))
}
pub fn dispatch(args: &[String]) -> Option<Result<(), String>> {
    let protocol = std::env::var("AW_MODULE_ACTION_PROTOCOL").ok();
    let action = match args.first().map(String::as_str) {
        Some("configure") => "configure",
        Some("open-logs") => "logs",
        Some("about") => "about",
        _ => return protocol.map(|_| Err("Unknown action command".into())),
    };
    Some((|| {
        if let Some(protocol) = protocol {
            if protocol != "1" {
                return Err("Unsupported action protocol".into());
            }
            if std::env::var("AW_MODULE_ACTION").ok().as_deref() != Some(action) {
                return Err("Action identity mismatch".into());
            }
        }
        let mut cli = false;
        let mut custom = None;
        let mut print = false;
        let mut port = None;
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--testing" => cli = true,
                "--print-context" => print = true,
                "--port" if i + 1 < args.len() => {
                    i += 1;
                    port = Some(parse_port(&args[i])?);
                }
                "--config" if custom.is_none() && i + 1 < args.len() => {
                    i += 1;
                    custom = Some(std::path::PathBuf::from(&args[i]));
                }
                _ => return Err("Unknown action option".into()),
            }
            i += 1;
        }
        let testing = testing(cli)?;
        let default = host::default_config(testing)?;
        let context = LaunchContext {
            testing,
            profile: std::env::var("AW_PROFILE").unwrap_or_default(),
            server_url: server_url(testing, port)?,
            config: custom.unwrap_or(default),
        };
        if print {
            return write_json("-", &context);
        }
        match action {
            "configure"=>{host::init_config(&context.config)?;management::launch_owned(context.config.to_str().ok_or("Invalid config path")?)},
            "logs"=>show_logs(&context),
            "about"=>show_text(&format!("ActivityWatch VPN module {}\nPassive local network context, not authenticated VPN access or work duration.\nTesting: {}\nConfigured server: {}\nNo connectivity test performed.",env!("CARGO_PKG_VERSION"),context.testing,context.server_url)),
            _=>unreachable!(),
        }
    })())
}
fn show_logs(context: &LaunchContext) -> Result<(), String> {
    let path = host::log_path(
        &context.config,
        context.testing,
        context.config != host::default_config(context.testing)?,
    )?;
    let text = match fs::read_to_string(path) {
        Ok(text) => text
            .lines()
            .rev()
            .take(100)
            .map(redact_line)
            .collect::<Vec<_>>()
            .join("\n"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            "No module log available for this profile.".into()
        }
        Err(_) => return Err("Cannot read module log".into()),
    };
    show_text(&format!(
        "Recent diagnostics, newest first. Historical details redacted.\n\n{text}"
    ))
}
fn redact_line(line: &str) -> &'static str {
    if line.contains("Starting AW module") {
        "Module startup"
    } else if line.contains("ActivityWatch delivery failed") {
        "AW delivery failed; observations retained"
    } else if line.contains("Unnamed connection detected") {
        "Unnamed connection detected"
    } else if line.contains("leaving a gap") {
        "Collection failed; timeline gap"
    } else {
        "Diagnostic entry (details redacted)"
    }
}
fn show_text(text: &str) -> Result<(), String> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    owned_helper::run(&format!("Add-Type -AssemblyName System.Windows.Forms; $f=New-Object Windows.Forms.Form; $f.Text='ActivityWatch VPN'; $f.Width=700; $f.Height=450; $t=New-Object Windows.Forms.TextBox; $t.Multiline=$true; $t.ReadOnly=$true; $t.ScrollBars='Vertical'; $t.Dock='Fill'; $t.Text=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}')); $f.Controls.Add($t); [void]$f.ShowDialog(); $f.Dispose()"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_log_details_are_not_exposed() {
        for line in [
            "secret-customer private-id internal.domain",
            "ActivityWatch delivery failed: secret-customer",
            "Unnamed connection detected: internal.domain",
            "Starting AW module secret-key",
            "leaving a gap secret-data",
        ] {
            let redacted = redact_line(line);
            assert!(!redacted.contains("secret"));
            assert!(!redacted.contains("private-id"));
            assert!(!redacted.contains("internal.domain"));
        }
    }
    #[test]
    fn server_roots_are_loopback_only() {
        assert_eq!(
            server_with_port("http://localhost:5600", Some(7123)).unwrap(),
            "http://localhost:7123"
        );
        assert_eq!(
            server_with_port("http://[::1]:5666", Some(7123)).unwrap(),
            "http://[::1]:7123"
        );
        for invalid in ["0", "65536", "-1", "bad", ""] {
            assert!(parse_port(invalid).is_err());
        }
        for value in [
            "http://127.0.0.1:7777/",
            "http://localhost:5600",
            "http://[::1]:5666",
        ] {
            assert!(validate_server_url(value).is_ok())
        }
        for value in [
            "https://localhost:5600",
            "http://evil.example:5600",
            "http://127.0.0.1:0",
            "http://localhost:5600/api/0",
        ] {
            assert!(validate_server_url(value).is_err())
        }
    }
}
