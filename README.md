# aw-watcher-vpn

Windows network-context module managed by ActivityWatch. One shared collector observes
adapters, assigned addresses, DNS configuration and routes across VPN vendors.
Customer rules produce assignment hints; they do not assign cases or bill time.

Release 0.2.3 uses a Rust core and a built-in Windows PowerShell/.NET
collector. It requires Windows 10/11 with Windows PowerShell 5.1 and the NetTCPIP
module. It requests no elevation, installs no driver, modifies no networking,
and sends no probes or DNS queries. Enterprise policy can restrict collection;
in that case collection leaves a gap. Runtime HTTP is exclusively 127.0.0.1:5600
(5666 with `--testing`), with no redirects. No cloud services are used.

## Build

With Rust and a compatible Windows linker installed:

```powershell
cargo test
cargo build --release
```

`scripts/build.ps1 -Test -Package` creates
`dist/aw-watcher-vpn-0.2.3-windows-x64.zip` and its `.sha256` checksum.
The ZIP contains an `aw-watcher-vpn` folder with the executable, installer,
naming shortcut, synthetic config example, README and license.

## Install as an ActivityWatch module

Exit ActivityWatch, extract the ZIP, and open its `aw-watcher-vpn` folder:

```powershell
.\install.ps1 -ActivityWatchDirectory 'C:\path\to\activitywatch'
```

The directory must contain `aw-qt.exe`. Installation places the executable in
`activitywatch\aw-watcher-vpn\`, which AW discovers through its existing
module launcher. It adds `aw-watcher-vpn` to `[aw-qt].autostart_modules`, retaining
other modules and backing up a changed tray config. Restart ActivityWatch;
**Modules -> aw-watcher-vpn** controls start/stop. No separate Windows service
or scheduled task is installed. Use `-NoAutostart` to leave startup settings alone.

To keep your previously learned profiles on first installation:

```powershell
.\install.ps1 -ActivityWatchDirectory 'C:\path\to\activitywatch' -ProfileConfig 'C:\path\to\old\config.json'
```

Import keeps profile IDs, labels, rules and polling settings, and copies the
discovery sidecar if present. It refuses to overwrite an existing AW module
configuration. Reinstallation without `-ProfileConfig` preserves your settings.
Stop any old development-harness instance before starting the AW-managed module.

The default configuration is under AW's Windows config root:

```text
%LOCALAPPDATA%\activitywatch\activitywatch\aw-watcher-vpn\config.json
```

`AW_PROFILE` selects a separate `activitywatch-<profile>` root. AW's `--testing`
launch uses `config-testing.json` and port 5666. With no arguments, the executable
initializes an empty profile list if needed, then stays running. Unlearned tunnel
discovery works immediately. Invalid initial settings are retried every ten
seconds. A config-specific lock prevents two watcher instances using one config.

Set `autolearn` or `notify` to `false` in this JSON to disable either feature;
defaults are `true`, including for imported old configs. The module reloads
settings each poll. Logs are in AW's `Logs\aw-watcher-vpn\` directory, rotated
after 1 MiB. An explicit `--config` puts logs beside that config.

Click a discovery notification to name connections, or open the installed
`name-connections.cmd`. Automatically launched naming helpers close when their
watcher exits. Manually opened management windows remain available independently.
The helper is part of the module; it is not a new module in AW's Modules menu.

Useful maintenance commands:

```powershell
.\aw-watcher-vpn.exe --config-path
.\aw-watcher-vpn.exe --init-config
.\aw-watcher-vpn.exe --config 'C:\private\config.json' --validate-config
.\aw-watcher-vpn.exe --dry-run --once
.\aw-watcher-vpn.exe manage
```

The explicit `run`, capture and learning commands below remain development tools.

## Capture and learn

```powershell
New-Item -ItemType Directory local -Force
.\target\release\aw-watcher-vpn.exe snapshot local\before.json
# Connect the desired VPN normally, then wait for networking to settle.
.\target\release\aw-watcher-vpn.exe snapshot local\after.json
.\target\release\aw-watcher-vpn.exe learn local\before.json local\after.json local\config.json --name "Customer A"
```

Capture files contain private network details. Keep them local; `local/` is
ignored by Git. The watcher refuses to overwrite captures. Use new capture names.
Captures and learned fingerprints stay local; normal operation keeps AW retries
in memory. Auto-learning also writes a local discovery file, as described below.

Repeat with another before/after pair and `--name "Customer B"`, using the same
`local\config.json`. Learning automatically appends the customer and generates
an internal profile ID; no separate alias is needed. Names are compared without
case or surrounding spaces. Existing customers require `--replace` to relearn:

```powershell
.\target\release\aw-watcher-vpn.exe learn local\before-new.json local\after-new.json local\config.json --name "Customer A" --replace
```

Replacement preserves the profile ID and configured destinations. Other profiles
and polling settings are retained. Updates replace the file atomically on Windows
and save its previous contents beside it as `config.json.<unique-value>.bak`.
If the file changed since it was read, learning refuses to overwrite it.
Zero or multiple changed adapters are rejected without changing the config.
For inspecting an ambiguous comparison, omit `--name` and write a new draft file;
that legacy draft command never overwrites an existing file.

Review `config.json`: remove unrelated Wi-Fi/VM changes and select stable rules.
A learned profile may match a physical interface;
learning is a change comparison, not automatic proof of a VPN. Captures must be
from the same machine with only the intended connection changed. Repeat the
cycle to find stable prefixes/DNS values. Dynamically recreated adapters may
need a description rule instead of an adapter ID. Routes containing a temporary
assigned IP should be removed/replaced with a stable customer subnet.

Rules within each category behave as follows:

| Category | Meaning |
|---|---|
| `adapter_ids` | Any exact ID, case-insensitive |
| `adapter_names` | Any exact connection/adapter name, case-insensitive |
| `description_contains` | Any case-insensitive substring |
| `dns_suffixes` | Any exact suffix; ignores case and trailing dot |
| `dns_servers` | All specified numeric IPs must be present |
| `route_prefixes` | All exact prefixes must exist on this interface |

All nonempty categories must match **the same operational adapter**, which must
have a usable unicast address. Loopback, unspecified, multicast and link-local
addresses alone do not count. At least one rule is required. Route prefixes are
exact matches, not subnet-containment rules. Config/schema typos are rejected.
Before/after learning now selects at most three non-host, non-default route
prefixes instead of requiring every changed route. Older profiles retain their
original rules until explicitly replaced.

`destinations` optionally contains numeric internal IP addresses. The core
estimates the best route by longest prefix, then route + interface metric.
Equal-cost ties across interfaces count as ambiguous. This is a local route
estimate, not a server reachability test or Windows policy-routing guarantee.
It does not send packets. Destination routing is reported separately from
profile presence, so a second connected profile remains visible even if another
tunnel wins the route. IPv4 and IPv6 are supported.

```powershell
.\target\release\aw-watcher-vpn.exe evaluate local\config.json local\after.json
.\target\release\aw-watcher-vpn.exe run local\config.json --dry-run --once
.\target\release\aw-watcher-vpn.exe run local\config.json --testing --once
.\target\release\aw-watcher-vpn.exe run local\config.json
```

`--dry-run` prints changes without contacting AW. `--once` samples once; AW
delivery failure returns a nonzero exit code. Stop continuous operation with
Ctrl+C. Collection takes some time because each poll launches PowerShell.
The collector has a 15-second timeout; AW requests have a 2-second timeout.

## Discovery and naming

Company VPNs that should never be customer context can be ignored by exact
connection name. Matching ignores case and surrounding spaces. In the naming
window, use **Ignore selected** for an unnamed VPN or **Ignored VPNs...** to
type any connection name (including a learned VPN) and later restore it.

```json
"ignored_connections": ["Polcom"]
```

Ignored connections produce no AW events, discoveries or naming notifications,
even if they have learned profiles. Other simultaneous customer VPNs remain
visible. If only ignored VPNs are active, no heartbeat is sent. Existing profile
rules and AW history stay intact; restoring resumes future detection.

CLI equivalents (use the actual config path printed by `--config-path`):

```powershell
.\aw-watcher-vpn.exe ignore 'C:\path\to\config.json' --name 'Polcom'
.\aw-watcher-vpn.exe unignore 'C:\path\to\config.json' --name 'Polcom'
```

The setting reloads on the next poll. Older binaries need updating to v0.2.1
before they can read this new setting.

Learning is optional. An empty `profiles` array is valid: active tunnel candidates
are still detected using PPP/tunnel interface types and recognizable VPN driver
descriptions. Ordinary Ethernet/Wi-Fi/VM adapters are excluded unless learned.
Candidates must be up, have a usable address and have a route. Unrecognized
virtual Ethernet drivers can be missed; this is passive inference, not proof of
VPN authentication. Unknown connections retain their observed adapter name. A
DNS suffix supplies an optional `suggested_name`, never an automatic customer
assignment. A generic/shared DNS suffix can be a poor suggestion.

By default, the watcher remembers new unnamed candidates in
`<config>.discovered.json` and shows a Windows balloon notification once per
candidate per run. Windows notification settings can suppress the balloon.
Click the balloon or double-click the naming helper's tray icon to open the
window listing **all remembered unnamed connections**, including previously
seen disconnected ones. It refreshes every five seconds; status refers to the
last collected context. Select a connection, enter a customer name, and save.
You can also open it while the watcher runs:

```powershell
.\aw-watcher-vpn.exe manage local\config.json
```

Existing customer names are offered in the dropdown. Reusing an existing name
requires selecting "Update an existing customer fingerprint" and confirming
replacement. This preserves the profile ID, history and destinations. Possible
matches based on an adapter ID/name or customer-name resemblance are suggestions,
not automatic reassignment. A changed GUID or overly restrictive old fingerprint
can otherwise make an already learned connection appear unnamed.

The CLI remains available:

```powershell
.\aw-watcher-vpn.exe name local\config.json "<connection-id>" --name "Customer A"
```

Add `--replace` to update an existing customer using the naming command.
This promotes the stored connection name into the main config, with a backup.
The running watcher reloads customer rules each poll. Review this connection-name rule
for clients that reuse one connection name for different customers; refine it with stable
DNS/routes or use before/after learning for those clients.
Naming upgrades older discoveries that stored only an adapter GUID to an exact
`adapter_names` rule, allowing the same connection to recreate its adapter.
No old customer profile is automatically weakened or overwritten.

For a future Windows service, run `manage` in your desktop session using the
service's absolute config path and file permissions that allow the UI to update
it. Notification UI belongs to an interactive desktop session; launching a
notification process from service session 0 does not display it on your desktop.
Service installation is not included; AW tray module installation is supplied
by `install.ps1` as described above.
The local `<config>.context.json` file supplies the helper's last observed state;
it is replaced on context changes and cleared on collection failure.

Disable either feature independently:

```powershell
.\aw-watcher-vpn.exe run local\config.json --no-autolearn
.\aw-watcher-vpn.exe run local\config.json --no-notify
.\aw-watcher-vpn.exe run local\config.json --no-autolearn --no-notify
```

Detection and AW recording continue with either option. With auto-learning off,
no new discoveries are saved; before/after learning is still available. Dry runs
never save discoveries or display desktop notifications.

## Event schema v3

One stable AW bucket per computer: `aw-watcher-vpn_<host>`.
Bucket type: `vpn.context`. Each heartbeat contains only active connections,
including learned and unlearned simultaneous tunnels. Old per-profile
buckets remain as history; the updated watcher stops writing to them. Stop the
old watcher and replace its executable to switch; the config needs no changes.

```json
{
  "schema_version": 3,
  "title": "Customer A|VPN B",
  "connections": [
    {"id": "synthetic-adapter-a", "name": "VPN A", "customers": ["Customer A"]},
    {"id": "synthetic-adapter-b", "name": "VPN B", "customers": [], "suggested_name": "customer-b.example"}
  ]
}
```

`title` is the alphabetical, case-insensitively deduplicated list of customer
names joined with `|` (no surrounding spaces), for example `Customer A|Customer B`.
Unnamed connections contribute their observed connection name until named.
Ignored connections never contribute to the title. Details remain in `connections`.

No active tunnels produces no AW heartbeat. Collection failure also emits no
heartbeat, leaving a gap. An empty set is retained only in local management
state and diagnostic output. A changed nonempty set
starts a new event; unchanged samples extend its duration through AW heartbeats.
Raw addresses, routes, DNS server IPs, descriptions and credentials are not sent
to AW. Adapter IDs/names, customer labels and optional DNS suffix suggestions
are sent to the local AW server. Stdout prints only derived changes.

AW heartbeats preserve sample timestamps. Pulse time is polling interval + one
second; long gaps stay gaps. There is no shutdown-disconnect claim. Retry FIFO
holds at most 512 samples for at most five minutes; it is lost on process exit.
Startup cannot recover the actual authenticated connection start. Times describe
observations, usually accurate to one polling interval plus collection latency.
Default AW visualizations may not display this custom bucket type; consume its
events in the planned review UI or inspect them through AW's bucket API.

## Limits and validation

Validated on 2026-10-08 using Rust 1.99.0 and the installed MSVC toolchain:
33 automated tests pass, including module config initialization/profile isolation,
duplicate-instance locking, suppression of empty heartbeats,
mixed known/unknown connections, exclusions,
named merging, replacement and Windows atomic
backup handling. Clippy passes with warnings denied, and the release
binary's embedded Windows PowerShell collector and synthetic-profile dry run
pass. The ZIP is `dist/aw-watcher-vpn-0.2.3-windows-x64.zip`. Automated AW delivery
tests use a real loopback mock HTTP server. No real VPN connect/disconnect cycle
or actual ActivityWatch server session has been validated yet.
The synthetic Windows Forms test `tests/management-ui.ps1` validates listing two
unnamed connections, saving a label, and removing the resolved row. Its rendered
form was visually inspected. Desktop notification clicks and a real Windows
service session have not been validated.
`tests/module-smoke.ps1` checks installation against a synthetic AW directory,
preserving other startup modules and existing settings, repeat installation,
profile import protection, default/testing/profile paths, duplicate instances,
and no-argument/`--testing` launcher lifetime. It forces collection failure so
these host tests never write to the user's real AW server. A complete installed
AW tray start/stop session has not yet been exercised.

An installed/up tunnel with stale routes is not proof of authentication or
reachability. Profiles with identical adapter/routes/DNS cannot be distinguished
generically. Rules may also match a customer's office LAN. DNS suffixes are
adapter-specific; NRPT, WFP rules, proxies and alternate routing compartments
are not collected. Collection is not atomic while a VPN changes configuration.
Reconnects may produce short gaps. No vendor-wide compatibility
claim is made before real connect/disconnect captures validate each profile.

Validate standard-user capture, connect/disconnect, reconnect, sleep/resume,
simultaneous tunnels, overlapping customer subnets, an AW outage/restart, and
DNS/address changes. Generic tests use synthetic addresses only.

See [VALIDATION.md](VALIDATION.md) for the 0.2.3 protocol review, deployed server schema evidence, restart/recovery regressions, and real-host acceptance checks.

## Architecture and references

`src/lib.rs` is pure matching/learning logic, reusable by another host adapter.
`scripts/snapshot.ps1` supplies Windows snapshots and is embedded in the binary.
`src/main.rs` handles capture, CLI and loopback AW delivery. Notifications and
native IP Helper calls can replace the polling collector later without changing
the event schema or profile matcher.

Primary references:
- https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getadaptersaddresses
- https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getipforwardtable2
- https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getbestroute2
- https://docs.activitywatch.net/en/latest/api.html

Inspected prior art: NetWatch (adapter/route heuristics), NetCatcher (interface
polling), netdev (Rust IP Helper wrapper), and aw-watcher-netstatus (TCP probe).
This implementation does not copy their source or perform their active probes.
