# Complete connection inventory — 2026-10-12, version 0.2.8

PASS: 40 Rust tests, Clippy, release package, action/module checks. Expanded management regressions check named rows remain listed, ignored rows remain visible and profile rename preserves matching rules/identity. Actual synthetic UI tests exercise naming, rename, visible ignored state and direct restore through the main selected-row button. Screenshot under local/ui-test-*/management.png.ignored.png was visually inspected. Settings shows last cached activity rather than authenticated VPN health; event privacy projection is unchanged. Imported profiles without discovery records remain editable as profile rows. Old unnamed CLI filtering is retained.

Separate persistence diagnosis: the deployed Rust server uses a disk-backed database with WAL. Read-only bucket API counts confirmed historical events preceding server startup, including window/AFK/VPN data. This disproves wholesale deletion in the current database; it does not validate the user's specific empty timeline view. No history contents were exported and no database modified. The view identification question remains pending.

---
# GlobalProtect detection regression — 2026-10-12, version 0.2.7

A user-reported active connection was missed by 0.2.6 because its Windows description is PANGP Virtual Ethernet Adapter Secure, not GlobalProtect. Palo Alto documents old/new names in https://docs.paloaltonetworks.com/globalprotect/release-notes/6-0/changes-to-default-behavior . Detection now accepts the precise PANGP Virtual Ethernet Adapter marker (both variants), still requiring an up interface, usable non-link-local address and a local route. The ignore filter still precedes detection. No vendor client API or active network probes were added.

PASS: 40 Rust tests, Clippy warnings denied, release packaging. The new regression checks both driver names, unknown customer preservation, ignored/down/link-local/unrouted exclusion. Replaying the private passive snapshot from the user's connected machine changes evaluation from empty to nonempty. Private snapshot/profile details remain in ignored local files; no routes/IPs/customer names committed. This is real-host snapshot recognition evidence, not proof of authenticated/reachable access, transitions, or server heartbeat acceptance. Installed replacement and fresh AW receipt remain pending under the existing deployment ownership constraint.

---
# Configure crash regression — 2026-10-11, version 0.2.6

Reproduced actual protocol-1 configuration launch against a synthetic config: the action exited 1 with Windows error 206, "The filename or extension is too long". The full embedded manager script exceeded the CreateProcess command-line limit after UTF-16 base64 encoding. Context probes and direct-script UI tests had bypassed this boundary.

Fixed owned and legacy UI helper launch to use a short encoded bootstrap plus UTF-8 script input over stdin. Owned children still start suspended, enter a kill-on-close Windows job, resume and read their script; STA is preserved. Added a script exceeding 40K characters and configure-ui.ps1, an explicitly interactive synthetic harness that must only run with authorization.

PASS: 39 Rust tests, Clippy, release packaging, module/actions checks. PASS actual UI execution: protocol context, synthetic explicit config, port 1 unavailable, producer lock held, child window shown and captured using PrintWindow, normal window close and action exit 0. Screenshot was visually inspected and remains under ignored local/configure-ui-*/configure.png. The existing synthetic naming/ignore UI harness also passed. No live settings were shown/edited and no production process was terminated or replaced. Actual installed-tray deployment is owned by the coordinating chat; real VPN/server/reboot limitations remain.

---
# Tray action validation — 2026-10-11, version 0.2.5

PASS: 38 Rust tests, Clippy, release package, tests/module-actions.ps1 and tests/module-smoke.ps1. Action context/dispatch, server-unavailable resolution, held producer lock, manual compatibility, sidecar copy/rollback/restoration and native job-owned helper cleanup are isolated evidence. Transport regressions still verify actual loopback heartbeat resumption. See MODULE_ACTIONS.md for full action and lifecycle limitations. No installed tray clicks/dialogs, deployment, reboot or actual server combinations were exercised. No workflows added.

---
# Validation update — 2026-10-11, version 0.2.4

- PASS: 35 Rust tests, warnings-denied Clippy, release build. Existing loopback tests still verify first/existing creation (304 only on creation), producer restart, late server startup, outage/server restart and actual heartbeat delivery resumption. Zero-duration original observation timestamps and pulsetime gaps remain preserved.
- PASS: privacy projection for strict and each extended choice, omitted disabled fields, title redaction/neutral fallback, serializer title-before-extensions order, invalid settings rejection, atomic persistence, and backlog removal on privacy changes. Missing payload settings migrate to conservative defaults. Field inventory and migration are in DATA_FIELDS.md.
- PASS: isolated module smoke checks validate installer idempotence/settings preservation, duplicate rejection, no-argument/testing lifetime, host-style termination/relaunch with lock release, explicit stop without self-respawn, missing executable launch failure, and CLI payload settings persistence. These are module/launcher checks, not proof of the installed host's restart scheduler.
- UI evidence: new Payload dialog parsed successfully under Windows PowerShell 5.1. No interactive dialog was opened or visually validated during this task; previous naming UI evidence does not validate these new controls.
- Source-only architecture: sibling aw-qt manager retains desired started state on failed launch; recovery.py retries with bounded 5-to-300-second backoff, retains per-module retry state, and forgets intentional stops. This component neither modifies nor replaces that host. Installed aw-qt recovery behavior, hard-restart login startup, failed-relaunch recovery and crash recovery still require real-host acceptance. No reboot was attempted.
- Read-only deployed evidence: server v0.13.2 and published heartbeat schema confirmed again. No production event queries, VPN captures, synthetic production writes or replacement installation were performed. Actual AW storage/API key order, GUI tooltip presentation, real VPN transitions and foreground/idle joins remain unverified. VPN presence deliberately remains independent of foreground/idle because it is a context hint, never a work-duration metric; consumers must intersect it with foreground/AFK activity for attribution.

The first sandbox run failed due to denied socket/atomic-file operations; the same regressions passed outside those sandbox restrictions. A synthetic launcher test initially used shell execution with an intentionally invalid SystemRoot; using redirected CreateProcess corrected that harness issue. These were harness/environment failures rather than claimed successful runtime tests.

---
# VPN module validation — 2026-10-08

Version: 0.2.3. Rust 1.99.0/MSVC, Windows PowerShell 5.1. This component remains an ActivityWatch-managed module in its independent Git repository. The installed 0.2.2 module was not changed during this validation.

## Protocol review

Reviewed ActivityWatch's [watchers](https://docs.activitywatch.net/en/latest/watchers.html), [writing a watcher](https://docs.activitywatch.net/en/latest/examples/writing-watchers.html), [data model](https://docs.activitywatch.net/en/latest/buckets-and-events.html), and [REST API](https://docs.activitywatch.net/en/latest/api/rest.html) documentation, plus maintained [Python client](https://github.com/ActivityWatch/aw-client/blob/master/aw_client/client.py) and [server REST implementation](https://github.com/ActivityWatch/aw-server/blob/master/aw_server/rest.py).

The maintained server returns 200 on first creation and 304 when a bucket already exists. The client tolerates that response. ureq already tolerated 304 in 0.2.2, so this module did not have the PL/SQL existing-bucket rejection defect. However, it also counted unexpected heartbeat 304 and some redirect statuses as delivered. 0.2.3 accepts only 2xx for heartbeats, and 2xx or 304 for bucket creation. On 304 it reads bucket metadata and requires `type: vpn.context`. It does not overwrite incompatible buckets or historical events. HTTP 404 on a heartbeat invalidates the creation cache and retries initialization on the next flush.

Read-only local `/api/0/info` reports server v0.13.2. Its `/api/swagger.json` exposes the expected bucket POST and heartbeat POST with an Event payload and pulsetime query parameter. Its generated response list only documents 200, so the maintained server implementation supplies the 304 regression contract. No bucket/event data or live network snapshots were collected for this review, and no synthetic events were sent to the production server.

## Automated evidence

33 Rust tests pass, including 16 adapter/host/management tests and 17 core detection tests. Loopback HTTP tests exercise actual adapter requests and responses:

- First creation, FIFO retry after HTTP 503, and heartbeat payload/title.
- Existing bucket returning 304, metadata compatibility, and two fresh producer adapters delivering heartbeats to that existing bucket.
- Server absent at startup, then listening later: queued heartbeat actually arrives with its original observation timestamp.
- Successful delivery, HTTP 503 outage, listener shutdown, and server restart with a missing bucket: heartbeat 404 triggers recreation and delivery resumes.
- HTTP 500 creation failure and incompatible existing type prevent heartbeat delivery; delivery resumes after compatible metadata becomes available.
- Redirect creation responses and heartbeat 304 retain the queued observation until a real 2xx delivery response.
- Retry payloads retain timestamp and zero duration. Observations separated by 30 seconds retain that spacing and a six-second pulsetime rather than extending stale context across the gap. This validates wire semantics; it is not a real-server merge test.
- Empty connection sets produce no requests; ignored company tunnels do not hide simultaneous customer connections. Titles are sorted, trimmed, deduplicated customer/observed connection names joined by `|`, without added module/app prefixes. Nameless context uses `Activity (context unavailable)`.

Collection remains local and passive; titles derive only from enabled customer/connection context. Ignored names are filtered before title construction. No SQL, message contents, credentials, or raw route/DNS lists are included in events. DNS-derived suggestions remain optional structured inference, not authenticated VPN access or tracked work duration. The autolearn/notify controls govern discovery and notification, not an additional title-redaction setting.

## Remaining runtime acceptance

- With a real nonignored VPN connected, verify receipt of `data.title` in the deployed server and inspect the AW GUI event view. A title alone does not provide custom `vpn.context` dashboard visualization support.
- Exercise actual connect/disconnect, simultaneous known/unknown tunnels, sleep/resume, producer restart against existing history, AW starting late, and AW outage/restart. Confirm time gaps and event merging in the real server.
- Verify tray start/stop, notification clicks, and management actions in the installed host. Prior synthetic module smoke checks cover packaging/config preservation and launcher lifetime; they are separate from real tray evidence.

These real-host acceptance actions were not run during this validation. No live installation was updated and no intrusive interactive GUI harness was launched.

## Installed 0.2.8 follow-up evidence (2026-10-12)

Installed the tested executable and matching sidecar with existing settings preserved. The management inventory reports six assigned profile rows, including one visibly ignored row. Before/after counts for events preceding the earlier server startup were identical across this actual AW restart for every existing bucket. This validates retention for that restart, not reboot recovery or the user's specific timeline rendering. No database changed and no history contents exported. The timeline-view clarification remains pending.

## Installed Activity/Timeline inspection (2026-10-12)

After the user identified the Activity/Timeline page, inspected the installed v0.14.0 Rust server's web UI in a fresh in-app browser session. Timeline loaded 240 events over its selected 12-hour range and visibly rendered AFK, editor, call, VPN and window rows; the VPN row included both neutral and assigned customer title states. Activity showed today's nonzero activity and earlier days. Console inspection showed no errors. This adds actual installed GUI evidence for timeline rendering of vpn.context with title, separate from default dashboard customization or duration attribution. No live activity contents/screenshots were committed. No browser preferences, buckets or database were modified.

The user's reported empty state after restart was not reproduced in this fresh session. Stale existing-browser state or a transient startup query failure remains a hypothesis, not a diagnosed source defect. Database/API retention and current page rendering passed; intermittent reload behavior still requires reproduction in the affected browser.

## Correction: real missing interval (2026-10-12)

The user's screenshot clarified a time gap, not an entirely empty page. Direct read-only SQLite interval checks confirm AFK coverage stops at 07:12:21 UTC (09:12:21 Europe/Prague) and resumes at 10:12:43 UTC (12:12:43 Europe/Prague); window events also have no intervals overlapping that gap. The legacy Python database ends the prior day, so it does not contain recoverable records for these hours. The present database retaining older rows and passing a later restart check does not establish that this earlier interval was ever persisted. Earlier stale-view speculation does not explain this actual gap.

Watcher logs show a 10:24 local launch and initial server connection, but available logs do not establish why that session's activity is absent. No source defect or exact failure cause is claimed. No matching accessible clock-change/crash evidence was found. No missing activity was reconstructed or fabricated; current collection and retention evidence remain separate from this unresolved historical gap.

## Corrected diagnosis: isolated launch/storage context (2026-10-12)

The user's complementary screenshots were confirmed against the normal desktop server. The SQLite path string is identical in both contexts, but Win32 volume/file identities differ. A tool-created Shell.Application dispatch still resolved the tool copy. Dispatching through the already running desktop shell's FindWindowSW(..., SWC_DESKTOP, ...) Document.Application resolved the same physical file as the normal server's open handle. This supersedes the earlier speculation about absent recording: the histories are split between physical stores. A direct tool-started process can also inherit tool job lifetime; no production AW should be launched that way.

Added scripts/start-activitywatch.ps1 with no fallback, validated dispatch availability and duplicate prevention while leaving the existing normal instance untouched. A read-only comparison found 356 source records absent by bucket/start/context identity from the normal instance, plus no same-identity longer source intervals. These are recovery candidates, not proof that every interval is conflict-free. No private event contents were logged or committed, no DB overwritten and no history merged. Recovery must back up both stores, review overlap and use idempotent replay before any authorized merge.

Normal-desktop Teams diagnostics show joined evidence becoming unknown during the reported media transitions, accounting for visible call gaps; toggling media alone is not a declared end. Historical logs lack the reason, so exact UI/collector cause remains unresolved. Teams diagnostics now distinguish collector_failure/controls_unavailable in a separately validated component change; no gap was silently filled.


## Fresh validation — 2026-10-16

Fresh scripts/build.ps1 -Test -Package -TargetDirectory target/updated PASS, followed by offline locked all-target Clippy with warnings denied. Forty Rust tests passed. tests/module-smoke.ps1 and tests/module-actions.ps1 passed using isolated synthetic installation/settings and owned child processes. Production AW contains positive-duration vpn.context events dated today with title-only data. Installed executable SHA256 differs from the fresh target/updated build; this alone does not identify a source-version difference. No VPN transitions, production restart, settings UI or reboot was exercised.


## 2026-10-20 � multiple connections per customer (0.2.10)

42 Rust tests pass (23 adapter/management/transport, 19 detection), with Rust
1.99.0; Clippy passes with warnings denied. Added regressions preserve the original
network fingerprint when adding Buildplace, canonicalize the existing customer
label, make repeated assignment idempotent, reject ambiguous legacy replacement,
rename a customer group without changing rules/IDs, deduplicate the AW title,
and verify either connection remains present independently. Manual learning
survives a changed GUID using the same connection name and required network rules.

Synthetic module-smoke and module-actions checks pass. PowerShell settings source
parses without errors. No interactive UI validation, installed-host deployment,
production configuration edits, real Buildplace renewal capture or reboot test
was performed. Existing strict GUID profiles are not migrated automatically.

The synthetic HTTP fixture now serves bodyless 304 responses with Content-Length:
67 over open persistent connections. It handles both same-connection reuse and
parallel requests, requiring subsequent metadata/heartbeat requests before it closes
the connection. Existing-bucket producer restart and actual heartbeat delivery,
metadata compatibility, heartbeat-304 rejection and outage/recovery tests pass.
This is synthetic protocol evidence; the deployed server was not retested.
Initial sandbox runs were blocked by socket/file-replacement permissions; validation
passed outside tool isolation without touching live AW. Release ZIP built locally.
