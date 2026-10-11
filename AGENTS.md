# VPN component guidance

Apply the root repository guidelines. This is an AW-managed context module; never add a self-respawning service/scheduled task. AW owns recovery and intentional stop. Keep matching evidence separate from event payload privacy projection in src/payload.rs. Document every new event field in DATA_FIELDS.md and expose it in Payload settings; new private fields default off. Keep strict title-only for the custom vpn.context contract. Do not substitute official currentwindow/afk/editor bucket types.

Run cargo test --locked, cargo clippy --all-targets -- -D warnings, scripts/build.ps1 -Test -Package (target/updated avoids a locked development binary), and tests/module-smoke.ps1. Smoke tests deliberately invalidate SystemRoot and isolate LOCALAPPDATA/AW_PROFILE; use redirected CreateProcess launches and terminate only harness-owned processes. Do not launch management-ui.ps1 or deploy/stop live modules without authorization. Separate serializer, loopback, source-only host recovery, UI and actual AW/reboot evidence in VALIDATION.md.

Tray action changes must follow MODULE_ACTIONS.md and the aw-qt protocol specification. Run tests/module-actions.ps1. Keep sidecar identity aligned with the executable; action UI children must be job-owned and never spawn producers. No GitHub workflows. Do not claim installed-tray or interactive evidence from context probes.
