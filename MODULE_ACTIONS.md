# VPN module tray actions — protocol 1

Version 0.2.5 packages UTF-8 `aw-watcher-vpn.exe.actions.json` alongside the matching executable. Enhanced aw-qt exposes **Modules → aw-watcher-vpn → Running** plus:

- **Connections and privacy settings...** (`configure`): existing naming, ignore and payload UI. Uses local config and remembered discovery evidence; does not contact the server or start collection. Cached connection information is not a new authenticated VPN test.
- **View redacted diagnostics...** (`open-logs`): displays up to 100 recent diagnostic categories from the profile's existing module log. Historical free text, identities and error detail are redacted; missing logs are reported honestly. No external text editor or unsupervised log helper is launched.
- **About...** (`about`): module version, local-context limitations and configured server root. Explicitly says no connectivity test was performed.

No status probe, refresh, pause, deletion or cloud actions are advertised. No IPC is needed by these management actions. Running remains entirely host-owned. The original aw-qt ignores the sidecar and keeps its normal start/stop entry; aw-tauri does not currently implement this protocol.

## Context, command fallback and ownership

Dispatch occurs before config producer initialization, singleton acquisition or capture. Unknown commands, unsupported protocols and mismatched action IDs fail. Protocol environment `AW_MODULE_ACTION_PROTOCOL=1` and `AW_MODULE_ACTION` are validated. `AW_TESTING=0/1`, `AW_PROFILE`, and `AW_SERVER_URL` are consumed; configuration/log resolution is shared with normal startup. Explicit `--testing` and `--config PATH` are supported manually. Without protocol context, direct `configure`, `open-logs`, `about` commands work. `--print-context` is a noninteractive diagnostic for these commands; it resolves context without creating config, locking or contacting the server.

This local-only module accepts HTTP loopback server roots (`127.0.0.1`, `localhost`, `[::1]`) and valid ports, rejecting credentials, nonlocal addresses and API/query paths. Normal producer startup now honors this root as well; the endpoint remains `/api/0` and bucket/heartbeat transport semantics are unchanged. Absent variables retain 5600/5666 and existing profile paths. Explicit AW_TESTING overrides profile inference; a manual --testing flag can enable testing.

Action executables synchronously own their PowerShell UI child. The child starts suspended, is assigned to a Windows job with kill-on-close, then resumes. All descendants remain in that job. Closing/terminating the AW-owned action releases the job, preventing orphan UI children. UI helpers have a separate management-dialog mutex and never acquire the producer lock. Only the action protocol/ID variables are removed for management's internal CLI calls, so naming/payload operations retain profile/testing/server context without being misidentified as new tray actions. Legacy manual `manage` remains available as its pre-existing desktop helper fallback.

## Local build, install and rollback

```powershell
$env:CARGO_HOME = Join-Path $PWD '.toolchain\cargo'
cargo test --locked
cargo clippy --all-targets -- -D warnings
.\scripts\build.ps1 -Test -Package -TargetDirectory target\updated
.\tests\module-actions.ps1
.\tests\module-smoke.ps1
```

No GitHub Actions workflows are used. Build copies the source manifest from scripts into the release executable directory and package. Installer copies `<source-executable>.actions.json` to the matching installed filename. When an explicitly selected older executable has no sidecar, installer removes the installed stale sidecar. Exit AW before authorized deployment/rollback; retain the config/discovery database. Manual replacement must copy the matching sidecar together with the executable, or remove it for a version without action commands. An old manifest must never be left advertising commands against an older binary.

## Evidence and limitations

Local Windows evidence on 2026-10-11: 38 Rust tests, Clippy warnings denied, release packaging, isolated module smoke and action dispatch checks pass. Dispatch checks cover every manifest command, named/testing context, explicit config/flag precedence, unavailable server, producer lock independence, invalid commands/protocol/IDs/roots, and manual fallback. A noninteractive PowerShell child terminates when its owning job is closed; this is native helper ownership evidence, not interactive UI evidence. Installer checks cover matching sidecar copy, removal on rollback and restoration.

No live watcher was stopped or replaced. No interactive action dialog was opened, real VPN data collected or production AW events written. Enhanced installed-tray clicks, duplicate-action host suppression, host exit with an open real dialog, actual window appearance, Python/Rust-server combinations and hard-restart login remain unvalidated. Cached management state is not live server/VPN health. This component does not modify aw-qt or shared root docs.
