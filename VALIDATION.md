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
