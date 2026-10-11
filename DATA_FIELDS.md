# Event data contract and privacy

`vpn.context` is this module's custom bucket type, not an official ActivityWatch event-type standard. The official AW contract requires a UTC/RFC3339 `timestamp`, numeric `duration`, and object `data`; it does not prescribe VPN data fields. See the [AW data model](https://docs.activitywatch.net/en/latest/buckets-and-events.html) and [REST API](https://docs.activitywatch.net/en/latest/api/rest.html). Presence is network context, not foreground work duration or authenticated/reachable VPN access.

Strict mode sends **only `title`**, required by this workspace in addition to the generic AW contract. There are no official VPN properties to put ahead of title. Extended mode sends title first, then selected custom extensions. Declaration order is tested in the serializer used for HTTP. JSON objects have no semantic order: actual AW storage/API reserialization and GUI tooltips have not been verified, and GUI ordering must be implemented in AW's presentation layer if necessary.

## Configuration and migration

The `payload` object is optional in schema-1 configuration. Missing objects/properties receive the conservative defaults below, without rewriting existing config automatically. Old configs therefore switch from unconditional metadata delivery to strict title-only delivery on upgrade. Existing customer titles remain; unknown connection names are hidden unless explicitly enabled. Existing AW history/config backups are not erased or redacted. Newly disabling a choice affects future payloads and CLI event exports, and clears in-memory delivery backlog on the next config reload (normally within one poll). Already transmitted events cannot be recalled.

```json
"payload": {
  "mode": "strict",
  "schema_version": false,
  "connection_ids": false,
  "connection_names": false,
  "customers": true,
  "suggested_names": false
}
```

Use the module's management UI **Payload...** button to select strict/extended and each optional field. Customer/name choices control title content in both modes; the other choices apply only in extended mode. Strict retains these choices when switching modes but never sends structured extensions. Invalid modes/unknown properties are rejected. Settings are saved atomically with the same stale-write protection as profile management. CLI: `payload <config>` reads settings; `payload <config> --base64 <UTF8-JSON-base64>` saves them. No new public endpoint or independently deployed application is introduced.

## Every available event data field

| Field | Type / example | Meaning and applicability | Privacy / default | Unavailable or disabled |
|---|---|---|---|---|
| `title` | string, `Customer A\|Customer B` | Workspace summary for every emitted event. Trimmed, alphabetical case-insensitive deduplication, joined by `\|`. Customer labels take precedence; unknown connection names may contribute only when enabled. No added app/module prefix. | Customer consent on by default; connection names off. No DNS suggestions/IDs used in title. | `Activity (context unavailable)` for active connections without enabled readable context. Empty connection sets emit no event. Cannot disable title. |
| `schema_version` | integer, `3` | Custom structured payload version, extended only. Not the config schema version. | Nonprivate; default off. | Omitted unless extended and selected. |
| `connections` | array of objects, `[{"customers":["Customer A"]}]` | Active nonignored simultaneous tunnels; extended only when at least one nested choice is enabled. Array order follows internal connection identity. | Container may expose count; default omitted in strict. | Omitted if strict or every nested choice is off. No events for an empty active set. |
| `connections[].id` | string, `synthetic-adapter-a` | Local adapter identity; connection_ids choice. | Persistent machine/interface identifier; default off. | Omitted when disabled. Collected connections have an ID. |
| `connections[].name` | string, `Customer tunnel` | Observed adapter/connection label; connection_names choice. | May identify company/customer; default off. Also governs connection-name fallback in title. | Omitted when disabled; empty string if source label unavailable. |
| `connections[].customers` | string array, `["Customer A"]` | Learned matching customer labels; customers choice. Multiple simultaneous assignments supported. | May identify customers; choice defaults on for title, structured array is only sent in extended. | Omitted when disabled, and labels removed from title. Empty array for an unknown connection. |
| `connections[].suggested_name` | string, `customer.example` | Optional DNS-suffix inference on an unnamed connection; suggested_names choice. It is not an assigned customer. | Potential private internal domain; default off. Never contributes to title. | Omitted if disabled, absent, or connection is named. |

Addresses, routes, DNS server IPs, descriptions, SQL/message contents and credentials are never event fields. `evaluate` and run/dry-run event output use the same privacy projection as HTTP; diagnostic/notification messages do not print disabled connection names, IDs or suggestions. Customer/title text is only printed when the customer choice is enabled. The raw `snapshot` command is an explicit developer capture, not an event export: it contains private matching evidence and must remain under ignored `local/`.

Local learning/profile files and `.discovered.json`/`.context.json` support detection and deliberate management review, not AW event history. They retain private evidence so disabled delivery fields do not break recognition/naming. Payload choices do not erase or redact this management database or its UI, nor the explicitly supplied profile config. Disable `autolearn`/`notify` separately to prevent new discovery persistence/notifications; these switches do not prevent network collection needed for detection. This distinction is shown here rather than treating payload switches as a machine-wide evidence deletion policy.
