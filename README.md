# aw-watcher-vpn

Windows network-context watcher for ActivityWatch. One shared collector observes
adapters, assigned addresses, DNS configuration and routes across VPN vendors.
Customer rules produce assignment hints; they do not assign cases or bill time.

This first release uses a Rust core and a built-in Windows PowerShell/.NET
collector. It requires Windows 10/11 with Windows PowerShell 5.1 and the NetTCPIP
module. It requests no elevation, installs no driver, modifies no networking,
and sends no probes or DNS queries. Enterprise policy can restrict collection;
in that case it reports unknown. Runtime HTTP is exclusively 127.0.0.1:5600
(5666 with `--testing`), with no redirects. No cloud services are used.

## Build

With Rust and a compatible Windows linker installed:

```powershell
cargo test
cargo build --release
```

`scripts/build.ps1 -Test -Package` also creates a ZIP in `dist`. No automatic
startup or VPN-client changes are made. Start it manually or configure AW's
launcher after validating your profiles.

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
| `description_contains` | Any case-insensitive substring |
| `dns_suffixes` | Any exact suffix; ignores case and trailing dot |
| `dns_servers` | All specified numeric IPs must be present |
| `route_prefixes` | All exact prefixes must exist on this interface |

All nonempty categories must match **the same operational adapter**, which must
have a usable unicast address. Loopback, unspecified, multicast and link-local
addresses alone do not count. At least one rule is required. Route prefixes are
exact matches, not subnet-containment rules. Config/schema typos are rejected.

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
The notification does not open a naming dialog; name the candidate using its ID
from the watcher output or discovery file:

```powershell
.\aw-watcher-vpn.exe name local\config.json "<connection-id>" --name "Customer A"
```

This promotes the stored adapter fingerprint into the main config, with a backup.
The running watcher reloads customer rules each poll. Review this adapter-ID rule
for clients that reuse one adapter for different customers; refine it with stable
DNS/routes or use before/after learning for those clients.

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
  "connections": [
    {"id": "synthetic-adapter-a", "name": "VPN A", "customers": ["Customer A"]},
    {"id": "synthetic-adapter-b", "name": "VPN B", "customers": [], "suggested_name": "customer-b.example"}
  ]
}
```

No active tunnels produces `connections: []`. Collection failure emits no
heartbeat, leaving a gap rather than reporting a disconnection. A changed set
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

Validated on 2026-10-05 using Rust 1.99.0 and the installed MSVC toolchain:
19 automated tests pass, including mixed known/unknown connections, exclusions,
named merging, replacement and Windows atomic
backup handling. Clippy passes with warnings denied, and the release
binary's embedded Windows PowerShell collector and synthetic-profile dry run
pass. The ZIP is `dist/aw-watcher-vpn-0.1.0-windows-x64.zip`. Automated AW delivery
tests use a real loopback mock HTTP server. No real VPN connect/disconnect cycle
or actual ActivityWatch server session has been validated yet.

An installed/up tunnel with stale routes is not proof of authentication or
reachability. Profiles with identical adapter/routes/DNS cannot be distinguished
generically. Rules may also match a customer's office LAN. DNS suffixes are
adapter-specific; NRPT, WFP rules, proxies and alternate routing compartments
are not collected. Collection is not atomic while a VPN changes configuration.
Reconnects may produce short empty-context intervals. No vendor-wide compatibility
claim is made before real connect/disconnect captures validate each profile.

Validate standard-user capture, connect/disconnect, reconnect, sleep/resume,
simultaneous tunnels, overlapping customer subnets, an AW outage/restart, and
DNS/address changes. Generic tests use synthetic addresses only.

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
