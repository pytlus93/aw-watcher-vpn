# Learning and managing connection profiles

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
case or surrounding spaces. Reusing a customer adds an independent connection fingerprint:

```powershell
.\target\release\aw-watcher-vpn.exe learn local\before-new.json local\after-new.json local\config.json --name "Customer A"
```

Existing profile IDs, rules, destinations and polling settings are retained. Identical
assignments are idempotent. Legacy CLI `--replace` is available only when the customer
has exactly one connection; it preserves that profile ID and destinations, but
replaces its rules. It refuses ambiguous replacement of a multi-connection customer. Updates replace the file atomically on Windows
and save its previous contents beside it as `config.json.<unique-value>.bak`.
If the file changed since it was read, learning refuses to overwrite it.
Zero or multiple changed adapters are rejected without changing the config.
For inspecting an ambiguous comparison, omit `--name` and write a new draft file;
that legacy draft command never overwrites an existing file.

Review `config.json`: remove unrelated Wi-Fi/VM changes and select stable rules.
A learned profile may match a physical interface;
learning is a change comparison, not automatic proof of a VPN. Captures must be
from the same machine with only the intended connection changed. Repeat the
cycle to find stable prefixes/DNS values. New manual learning uses the connection
name instead of a transient adapter GUID, plus the selected DNS/route evidence. Routes containing a temporary
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
Before/after learning selects at most three non-host, non-default route
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
window, use **Ignore selected** for an unnamed VPN or **Ignored VPNs…** to
type any connection name (including a learned VPN) and later restore it.

```json
"ignored_connections": ["Corporate VPN"]
```

Ignored connections produce no AW events, discoveries or naming notifications,
even if they have learned profiles. Other simultaneous customer VPNs remain
visible. If only ignored VPNs are active, no heartbeat is sent. Existing profile
rules and AW history stay intact; restoring resumes future detection.

CLI equivalents (use the actual config path printed by `--config-path`):

```powershell
.\aw-watcher-vpn.exe ignore 'C:\path\to\config.json' --name 'Corporate VPN'
.\aw-watcher-vpn.exe unignore 'C:\path\to\config.json' --name 'Corporate VPN'
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

Existing customer names are offered in the dropdown. Select the new connection,
choose its customer and use **Add connection**. Network and Buildplace connections
keep independent fingerprints with the same canonical customer label. Selecting
an assigned row exposes **Rename customer**, which renames all that customer
connections together while preserving their IDs and rules. Possible
matches based on an adapter ID/name or customer-name resemblance are suggestions,
not automatic reassignment. A changed GUID or overly restrictive old fingerprint
can otherwise make an already learned connection appear unnamed.

The CLI remains available:

```powershell
.\aw-watcher-vpn.exe name local\config.json "<connection-id>" --name "Customer A"
```

Omit `--replace` to add this connection to an existing customer without removing others.
This promotes the stored connection name into the main config, with a backup.
The running watcher reloads customer rules each poll. Review this connection-name rule
for clients that reuse one connection name for different customers; refine it with stable
DNS/routes or use before/after learning for those clients.
Naming upgrades older discoveries that stored only an adapter GUID to an exact
`adapter_names` rule, allowing the same connection to recreate its adapter.
No old customer profile is automatically weakened or overwritten.

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

## Buildplace renewal and customer grouping (0.2.10)

Multiple profiles can carry one customer label in the existing schema; no config
migration or separate customer-alias field is needed. Each profile has its own
stable ID and independent rules. Both connections active produce one customer
in the alphabetical, deduplicated AW title; either connection alone still supplies
that customer. Ignore remains per connection name, not customer-wide.

A regenerated Buildplace adapter can change GUID and interface index without
invalidating a name-based profile. If its name or required DNS/routes also change,
review the newly discovered connection and add it to the same customer. Old
fingerprints remain available. We do not guess an automatic reassignment from
shared Buildplace infrastructure, generic driver descriptions or similar subnets.
No real before/after Buildplace renewal capture has yet established further stable
provider identifiers. Older strict GUID profiles are retained until explicitly
reviewed; adding a renewed connection does not silently weaken them. Historical
AW events are never renamed or rewritten by configuration changes.

## Delete an obsolete connection (0.2.11)

Select the saved connection row and click **Delete connection**, then confirm.
Only that profile and its associated cached discoveries are removed. Other profiles,
ignore settings and historical AW events remain. The configuration and discovery
files receive backups; the running module reloads the change on its next poll.
Deleting the last connection removes that customer's future matching evidence.
An active forgotten VPN can be discovered again as unnamed; use Ignore for lasting
suppression. Delete is available for saved profiles; use Ignore for unnamed discoveries
or Restore selected for ignored-only entries.

```powershell
.\aw-watcher-vpn.exe delete 'C:\path\to\config.json' '<profile-id>'
```

Use the profile ID from `connections` without its `profile:` UI prefix. Unknown IDs
fail without changing configuration. If cached-discovery cleanup fails after the
profile is removed, the command explicitly reports partial completion; its backups
remain available. No AW history endpoint is called by deletion.
