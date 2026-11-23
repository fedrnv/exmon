# Transactional settings and selected-capture preview

Open Settings to edit portable protocols/interfaces or machine-local bindings.
The fixed toolbar keeps Validate, Apply, Cancel, safe defaults and Preview
accessible while the form scrolls. Structured controls expose indexed field
paths, enum choices, booleans and numeric/text values in pages of64 fields.
Add protocol/interface/field/binding uses the displayed zero-based index. Switch
framing or transport kind to install that variant's complete editable defaults.
An empty array/object or nullable value can be edited as JSON. Advanced JSON
supports the complete existing project and binding schemas and structural changes
including deletion; Stage JSON documents is required before Validate or Apply.

All changes are drafts. Validate reports the exact indexed semantic field or JSON
line/column without changing active processing. Invalid fields/types/overlaps,
references, unsupported versions, enabled stored schedules and oversized documents
are rejected. Cancel restores the currently active profile/bindings/limits. Restore
safe defaults stages an empty project and binding set, default limits and no
selected profile; Apply is still required. Existing received-type name edits are
synchronized from the running pipeline without overwriting dirty setting drafts.

Processing interface profile selects the portable interface and its protocol's
stream framing. Its direction, port and CAN-ID filters apply identically to live
processing and preview. The source transport must match the selected profile;
additional source filters remain separately visible. No selected profile leaves
transport-native datagram processing available but supplies no implicit TCP/serial
framing. Machine selectors are stored as local data and never open a device or
replace the explicit synthetic source selector. Baud/USB/CAN configuration is
validated profile data; physical adapter configuration remains later stand work.

Apply compiles the candidate off GUI, rejects superseded requests and constructs
a prepared incoming worker before committing active metadata. A successful apply
stops capture, disconnects outgoing TX permissions/schedules, advances the capture
generation and clears old incoming selection/copy/chart history. Start capture
and TX reconnect/Enable require new explicit actions. Invalid or canceled settings
leave active processing and its running exchange unchanged. Imported templates,
chart records and disabled schedules remain passive project records; this settings
transaction activates processing/filter/safety settings, never creates runtime TX
requests. Existing outgoing drafts retain their historical source definitions.

Select an incoming instance before Preview selected captures. Preview uses up to32
retained raw chunks ending at that source, with at most65536 total payload bytes.
If its history was evicted, the pinned instance's original raw observation remains
available. Earlier missing stream context is not invented. The preview calls the
same application Processor, framer, classifier, checksum and field codecs as live
capture, preserving timestamps, direction, interface/epoch/sequence and source
spans. Raw and normalized rows, HEX, validity and paged decoded fields are visible;
choose Preview row to inspect each result. Preview is a separate draft/request
namespace; it does not recapture hardware or apply a new snapshot length to old
bytes. Editing invalidates prior preview results.

Capture safety limits apply on the next explicit source start after Apply:

| Field | Accepted bound |
| --- | --- |
| queue_items | 1..4096 |
| queue_bytes | 1..16777216 |
| batch_items | 1..32, matching the downstream batch contract |
| batch_bytes | 1..65536 and at least snapshot_bytes |
| snapshot_bytes | 1..64 for the current synthetic source capability |

Defaults preserve the original64 items/4096 bytes queue,32 items/2048 bytes batch
and64-byte snapshot. A stricter snapshot truncates future observations honestly;
backend loss, application drops and display decimation remain separate. Physical
backends must enforce their own qualified snapshot/capability limits later.

Import/export configuration requires an explicit local file path. Portable project
and Machine bindings retain their separate schema-one formats; selectors cannot
enter a portable project export. Runtime safety file has its own small envelope:
`{"schema_version":1,"limits":{"queue_items":64,"queue_bytes":4096,"batch_items":32,"batch_bytes":2048,"snapshot_bytes":64}}`.
Import validates into a draft only. Export validates the chosen document and uses
create-new: existing files are never overwritten. Project exports may contain
stored template bytes, so choose a trusted destination. Reads and writes happen
on the worker, are bounded to1MiB, and report fixed read/size/existing/incomplete
errors without raw paths/driver text in logs. Windows network/device/alternate
stream paths are rejected; selected ordinary local files still obey OS access and
filesystem behavior. A failed write may leave an incomplete new destination.

The settings queue holds eight admitted commands, each document at most1MiB,
field text4096 bytes and pointer512 bytes, file path4096 bytes. Prepared form pages
hold64 fields; rejected-field diagnostics are capped at64 and then require Cancel
or defaults. A lost/rejected input blocks validation/apply/export until explicit
recovery. Checked request revisions fence prepared apply against edits, Cancel and
exhaustion. Independent queued field edits remain ordered. Preview output is at
most96 rows/1MiB charged data, shared immutable snapshots do not retain an
unbounded history, and retired incoming workers are joined off GUI. External
snapshots, native/renderer resources and filesystem latency have separate bounds;
no total RSS or hard real-time guarantee is implied.

Focused application/GUI/file tests cover transactional failure, full queues and
counter exhaustion, exact preview/live vectors and filters, safe defaults,
independent documents/no overwrite, actual scheduled mock revocation and supported
window/DPI/theme interaction. Run the hidden native Windows scenario with
`cargo run -p exmon-gui --example native_settings_smoke --locked`. Physical/macOS
and packaged/install/release qualification remain EXM-035/036/037.
