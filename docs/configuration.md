# Configuration schema version 1

The domain types live in `exmon_domain::config`. Use
`exmon_storage::config::{load_project, save_project, load_bindings, save_bindings}`
at the JSON boundary. These functions validate on both load and save, perform no
device or disk I/O and never enable transmission. Direct Serde deserialization
is not validation; in-memory edits require `validate()` before use.

Every document requires integer `schema_version: 1`. A missing version is a
schema error; any other version requires an explicit future migration tool.
There is no implicit conversion or downgrade. Unknown fields, duplicate struct
keys, invalid enum values and trailing data fail. Defaults apply only to absent
optional collections/values, never to an explicitly invalid value. The original
input remains available to the caller after an error.

## Portable project

```json
{
  "schema_version": 1,
  "protocols": [{
    "id": "demo",
    "name": "Synthetic two-byte messages",
    "framing": {"kind": "fixed", "length": 2},
    "fields": [{
      "id": "value", "offset": 0, "length": 2,
      "representation": "unsigned", "endian": "big"
    }]
  }],
  "interfaces": [{
    "id": "stream", "protocol_id": "demo",
    "transport": {"kind": "tcp"}
  }],
  "charts": [{
    "id": "values", "protocol_id": "demo", "field_id": "value",
    "samples": 100, "moving_average": 2
  }]
}
```

The smallest valid project is `{"schema_version":1}`. Collections default empty.
Ids use 1..128 ASCII alphanumeric, underscore or hyphen bytes and are unique
within each collection (fields within a protocol). Display names are independent
labels, 1..128 bytes without control characters. References must resolve in the
same project; no filesystem or external reference resolution occurs.

| Section | Contract |
| --- | --- |
| Protocol | Reusable id/name, framing, optional `escape` byte/xor, classification and field list. No machine selectors. Missing escape means None and is omitted on save. |
| Framing | `datagram`; `fixed` with positive length; `delimiter` with 1..16 bytes and max_length; `length_prefix` with offset, width 1/2/4, endian and max_length; `markers` with start/end byte arrays and max_length; `idle_gap` with positive gap_ns and max_length. Positive decoded-frame limits at most 65536 include framing bytes. TCP and serial require explicit stream framing. See [runtime semantics](framing.md). |
| Classification | Optional positive exact length, prefix, offset, equal-length mask, direction, typed transport predicate and signed priority (higher wins). Absent new fields preserve old defaults; equal top matches stay ambiguous. Prefix/header range must fit framing and optional exact length. See [classification](classification.md). |
| Grouping | Optional `grouping` settings: header_bytes 0..16 (default 4), up to 256 explicitly named canonical automatic-group IDs. Default settings are omitted on save. Names and configured Protocol.name are persisted only by explicit edits; classification adds no learned history. |
| Field | Byte offset/length within framing limit, representation unsigned/signed/float/bytes/ascii/utf8/derived, explicit little/big endian. Integers allow widths 1/2/4/8; float allows 4/8. Optional bits, typed constant, enumeration, constraints and bounded derived expression. Finite scale/bias default 1/0, unit defaults empty and is at most 32 bytes without controls. Physical bit overlap is rejected; virtual derived fields use offset/length zero. See [binary field semantics](binary-fields.md). |
| Interface | Logical id, protocol reference, tagged transport, optional filter, TX permission default false. Permission alone never sends data. |
| Transport | ethernet/tcp/udp; serial with baud, data_bits, stop_bits, parity none/odd/even; usb with non-control endpoint and transfer hid/bulk/interrupt; can with bitrate and fd flag. Serial covers USB CDC and RS-232/422/485 byte streams; platform electrical mode/capabilities belong to adapter configuration. |
| Filter | Optional Rx/Tx/Unknown direction, port list for TCP/UDP, CAN id list for CAN. Ports are u16; CAN ids are at most 29 bits. Empty lists impose no constraint. |
| Chart | Protocol/numeric-field references, samples 1..65536, optional positive moving_average no larger than samples. |
| Template | Id, reference to a TX-permitted logical interface, 1..65536 payload bytes. Backend-specific TX/frame validation is required again before actual submission. |
| Schedule | Id, template reference, interval_ms 1..86400000, enabled default false. Persisted true is rejected; explicit runtime enable remains a separate action. |

Numeric ranges in schema validation are portable safety bounds, not guarantees
that an OS driver or device accepts that setting. Serial baud is 1..4000000,
data bits 5..8, stop bits 1/2; CAN bitrate is 1..8000000. USB endpoint addresses
allow direction bit and endpoint 1..15 only. Adapters must check actual supported
capabilities and transmission validity before use. Configuration loading cannot
open a physical interface, execute a selector or restore an enabled schedule.

## Machine-local bindings

Bindings use a separate versioned document and validate against the project:

```json
{
  "schema_version": 1,
  "bindings": [{"interface_id": "stream", "selector": "local-loopback-alias"}]
}
```

Selectors are nonempty opaque local aliases at most 128 bytes without controls.
Only an adapter resolves them; no shell commands are supported. Bindings are
unique per interface and may cover a subset of the project's interfaces. An
unbound logical interface is valid portable configuration, but cannot be opened
until explicitly resolved. Exporting a project never includes bindings. Local
files may contain sensitive device identity; do not commit real binding files.

## Bounds and deterministic output

JSON input is limited to 1 MiB before parsing. Serde's recursion guard remains
enabled; collections are validated to at most 256 entries each, including fields
and filter lists. Byte payload/frame limits are 65536. Checked range addition
rejects arithmetic overflow. Parse errors expose line/column and fixed guidance,
never the parser's input-containing text; semantic errors use fixed section and
reason strings. Validation stops at the first error, keeping diagnostics bounded.

Save validates before serialization and refuses output above 1 MiB. JSON uses
the declared struct order, preserves user list order, materializes defaults and
emits no timestamps or random data. Save/load/save of the same valid object
therefore produces identical bytes. List permutation is intentionally a change
to the document, not canonicalized away. Allocation remains bounded by the input
cap on load and by collection/string/byte limits on save. Parsing does not claim
to be zero-copy. Streaming file reads must enforce the same cap upstream.

Run `cargo test -p exmon-storage --locked` for synthetic schema integration tests,
including transport variants, deterministic roundtrips, separate bindings,
version failures, unknown/duplicate fields, unsafe schedules and malformed ranges.
