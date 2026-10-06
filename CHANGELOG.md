# Releases

## 0.2.0 — 2026-10-06

- Run through ActivityWatch's Modules menu: no-argument startup and `--testing`.
- Keep configuration, discovered connections and logs under AW's profile folders.
- Install with preserved AW autostart modules and optional import of learned profiles.
- Preserve existing configuration on reinstall; back up changed tray settings.
- Reload auto-learning/notification settings and prevent duplicate watcher instances.
- Close automatically launched naming helpers when their watcher exits.
- Keep active-only heartbeats, simultaneous known/unknown connections and the naming UI.

Validated with 25 Rust tests, Clippy, synthetic Windows installer/launcher tests,
and the Windows Forms naming test. Real installed AW tray start/stop validation
remains pending. This is a local Windows x64 release; no service or scheduler
is installed.

## 0.1.0 — 2026-10-05

Development harness: passive Windows network snapshots, customer learning,
one AW bucket per computer, active connection events and an unnamed-connection
management window. Empty connection sets produce no AW heartbeat.
