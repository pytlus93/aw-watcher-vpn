# Releases

## 0.2.3 — 2026-10-08

- Limit HTTP 304 success to bucket creation; reject unexpected heartbeat and redirect responses instead of dropping undelivered observations.
- Check the existing bucket's `vpn.context` type after 304 before delivering events. Log the concrete transport failure without event contents.
- Use `Activity (context unavailable)` when a nonempty connection set has no readable names.
- Add bounded loopback regressions for producer restart, late AW startup, outage/server restart, incompatible buckets, genuine failures, and original timestamp/gap preservation. All 33 Rust tests and warnings-denied Clippy pass; see `VALIDATION.md` for evidence and remaining real-host checks.

## 0.2.2 — 2026-10-08

- Add AW event `title`: unique customer names sorted alphabetically and joined by `|`.
- Use the observed connection name for an unnamed connection; ignored VPNs remain excluded.
- Keep detailed connection metadata and suppress empty heartbeats.
- Read older local context files without a title.

Validated with 27 Rust tests, including the loopback heartbeat payload.

## 0.2.1 — 2026-10-06

- Ignore selected connection names, even when they have a learned customer profile.
- Exclude ignored VPNs from AW events, discovery and naming notifications.
- Add Ignore selected and Ignored VPNs / Restore controls to the naming window.
- Keep other simultaneous customer connections; ignored-only samples send nothing.
- Keep fingerprints intact so restoring a VPN does not require relearning it.

Validated with 26 Rust tests and synthetic module/Windows naming UI checks.

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
