# ActivityWatch VPN context

A Windows module managed by ActivityWatch that observes adapters, addresses, DNS configuration and routes. Learned rules associate connections with customer labels, including simultaneous profiles. These are assignment hints, not proof of authenticated VPN access, reachability or work duration.

## Requirements

- Windows 10/11, Windows PowerShell 5.1 and NetTCPIP.
- ActivityWatch with a local server and `aw-qt` module launcher.

Collection is passive: no drivers, network changes, active probes or cloud service are used. Enterprise policy may prevent collection. Real client/profile compatibility requires transition testing; see [validation](VALIDATION.md).

## Install or upgrade

Extract `aw-watcher-vpn-<version>-windows-x64.zip`, close ActivityWatch normally, and run from its `aw-watcher-vpn` folder:

```powershell
.\install.ps1 -ActivityWatchDirectory 'C:\path\to\activitywatch'
```

The directory must contain `aw-qt.exe`. The installer copies the module and matching actions sidecar, preserves module settings, and backs up a changed tray configuration. `-NoAutostart` leaves the startup list unchanged. To import an old profile file on first installation, add `-ProfileConfig 'C:\private\config.json'`; import refuses to overwrite an existing module configuration.

Start ActivityWatch normally. **Modules → aw-watcher-vpn** controls the worker. Protocol 1 hosts also expose module actions; see [tray integration and rollback](MODULE_ACTIONS.md). Start the host from your normal desktop session rather than a tool-owned background process.

## Configuration and everyday use

Default config: `%LOCALAPPDATA%\activitywatch\activitywatch\aw-watcher-vpn\config.json`. `AW_PROFILE` selects an isolated root; testing selects port `5666` and `config-testing.json`. Normal delivery uses loopback port `5600`. Settings reload each poll.

Polling defaults to 5 seconds (`poll_seconds`, range 2–60). Discovery and notifications are on by default (`autolearn` and `notify`). Open **Settings…** on a supporting tray, click a discovery notification, or run `name-connections.cmd` to name, rename, ignore or restore connections. Learning is optional; unnamed tunnel candidates can be detected without a profile. A reused connection name or overlapping network fingerprint may need more specific rules.

Use the management UI's payload controls for strict/extended mode and individual metadata choices. Strict is the default and sends a readable `title` for this custom `vpn.context` bucket. Customer/name consent also controls titles. No active connections means no heartbeat. Local management/discovery files contain network evidence independently of selected AW fields; keep them private.

For advanced learning, exclusions and route rules, see [configuration](docs/CONFIGURATION.md). The [field catalog](DATA_FIELDS.md) explains defaults, migration and privacy.

## Troubleshooting

- **No context:** check whether collection is permitted and the interface has a usable address/route. Physical or unrecognized virtual adapters may require learned rules.
- **Wrong customer:** refine the saved rules; passive fingerprints cannot distinguish identical networks.
- **No AW events:** check the module state, selected profile/port, ignored connections and enabled payload choices. Review logs under AW's `Logs\aw-watcher-vpn\` directory; explicit `--config` keeps logs beside that config.
- **Missing time:** collection failure and long gaps remain gaps. Retry memory is bounded and lost on exit; context is not authenticated connection uptime.

## Local development

Requires Rust and the MSVC linker. Use a separate build directory when another development binary is running:

```powershell
./scripts/build.ps1 -Test -Package -TargetDirectory target/updated
cargo clippy --locked --all-targets --target-dir target/updated -- -D warnings
./tests/module-smoke.ps1
./tests/module-actions.ps1
```

The version from `Cargo.toml` determines the ZIP name in `dist/`; a `.sha256` file accompanies it. [Validation](VALIDATION.md) separates synthetic protocol, installed receipt, UI and lifecycle evidence. [Configuration](docs/CONFIGURATION.md) documents synthetic capture/learning commands; private snapshots belong under ignored `local/`.

## License

[MIT](LICENSE).

Customers can have multiple independent VPN connections (for example Network and
Buildplace). Select an unnamed connection, choose an existing customer and click
**Add connection**; existing fingerprints and history remain. Renaming an assigned
customer renames all its connection profiles together. New manual fingerprints use
connection names rather than transient adapter GUIDs; changed names or required
network evidence still need review. See [configuration](docs/CONFIGURATION.md).

Use **Delete connection** on a saved profile or unnamed discovery to forget that connection fingerprint.
Deletion preserves other connections for the customer and all recorded AW events.
An active forgotten VPN can be discovered again; use Ignore instead to suppress it.
