# Releases

## 1.0.0 — 2026-10-24

- Add `--port <port>` to the AW-managed module and management actions, preserving
  profile/testing isolation and the loopback server host.
- Keep passive matching, multiple customer connections, payload privacy defaults,
  bucket/config identities and AW-owned lifecycle behavior unchanged.
- Run locked Clippy alongside local tests in the build script and update synthetic
  module/action checks for the versioned release.
- Retain the existing bodyless 304/restart regressions and versioned Windows x64
  package with SHA256 checksum. No live installation or reboot acceptance is implied.

## 0.2.12 — 2026-10-20

- Enable Delete connection for unnamed discoveries. Remove only the selected cached entry with a backup; preserve customer assignments, other discoveries and ignore settings.

## 0.2.11 — 2026-10-20

- Add Delete connection for an individual saved profile, with confirmation and configuration/discovery backups. Preserve sibling connections, ignore settings and recorded history.
- Remove associated cached discoveries; explain that an active forgotten VPN may be discovered again and Ignore suppresses it instead.

## 0.2.10 — 2026-10-20

- Add independent VPN connection fingerprints to an existing customer by default; preserve old rules, IDs and history. Repeated assignment is idempotent.
- Distinguish Add connection from Rename customer in Settings; rename all connections belonging to a customer together. Remove destructive replacement from the UI and reject ambiguous legacy CLI replacement.
- Manual learning uses connection names instead of transient adapter GUIDs, retaining learned DNS/routes. Shared customer titles remain deduplicated.

## 0.2.9 — 2026-10-12

- Add an explicit desktop-shell AW launcher that refuses tool-owned fallback and avoids duplicate running instances.
- Correct deployment validation: tool-launched AW used a different physical SQLite file behind the same path. Preserve both histories; no automatic migration.
- 40 Rust tests, Clippy and packaging pass. Desktop dispatch and already-running checks passed without restarting the normal installation.

## 0.2.8 — 2026-10-12

- Keep all saved profiles, unnamed discoveries and ignored connections visible in the main Settings window. Show assignment and ignored state; restore ignored entries directly from the selected row.
- Rename saved customer profiles without replacing matching rules, stable profile identity or historical events. Retain the legacy unnamed-only CLI command and add a separate full management inventory.
- 40 Rust tests, Clippy, action/module checks and synthetic UI naming/rename/ignore/direct-restore tests pass. Captured ignored-row UI visually inspected. Separate read-only server checks confirm retained events from before its current startup; the user's empty timeline view still needs identification.

## 0.2.7 — 2026-10-12

- Recognize GlobalProtect's documented PANGP Virtual Ethernet Adapter descriptions, including the Secure variant. Previously the generic driver check looked for GlobalProtect and missed these actual Windows names, so an unlearned active tunnel produced no event.
- Add synthetic coverage for both names plus ignored, disconnected, link-local-only and unrouted adapters. Passive local snapshot replay now recognizes the user's previously missed connection without changing profiles or network settings.
- 40 Rust tests, Clippy and release packaging pass. Live AW receipt after replacement remains to validate; customer assignment is still an explicit management operation.

## 0.2.6 — 2026-10-11

- Fix configuration action failure caused by Windows command-line length limit (error 206). Pass embedded PowerShell UI scripts over UTF-8 stdin, leaving only a short bootstrap in the command line; retain STA and job-owned descendants.
- Apply the same script transport to legacy manual management helpers. Add a >40K script regression and a synthetic actual configuration-window harness.
- Validate the protocol-1 window opens with the producer lock held and server unavailable, visually inspect its capture, close normally and observe exit 0. 39 Rust tests, Clippy and local package/action/module checks pass. Live installation remains untouched.

## 0.2.5 — 2026-10-11

- Implement aw-qt tray actions protocol 1: configuration/privacy UI, redacted diagnostics and About, dispatched before producer locking/collection.
- Share profile/testing and loopback server context with startup. Own UI descendants through a kill-on-close Windows job; retain manual CLI fallback and host-owned lifecycle.
- Package/install the exact executable actions sidecar, remove it on rollback to a source without one, and validate isolated manifest/dispatch/context/ownership behavior locally. 38 Rust tests, Clippy, module/action smoke and packaging pass; actual installed tray/dialog behavior remains unverified.

## 0.2.4 — 2026-10-11

- Add strict title-only and configurable extended payloads with individual field controls in the module management UI. Disabled customer/connection names are removed from titles and event exports; privacy changes discard queued observations.
- Migrate missing settings conservatively, remove identifying diagnostics/notification text, document every data field and custom contract separately from official AW types.
- Add privacy/settings and isolated crash/relaunch/stop regressions. 35 Rust tests, Clippy, packaging and module smoke checks pass; the new dialog is syntax checked but not visually validated. Real-host/reboot recovery remains an acceptance check.

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


