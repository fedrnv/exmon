# Product domain model

EXM-001 defines the shared contract for subsequent implementation. This is a
design contract, not evidence of implemented adapters or hardware support.
It follows ADR-0001, ADR-0002 and ADR-0003. Backend handles and privileges belong
to capture adapters; domain values contain no GUI or operating-system objects.

## Identities and ownership

All identifiers are opaque and typed: InterfaceId, SessionId, ObservationId,
MessageId, MessageTypeId, FieldId, ChartId, TemplateId and ScheduleId. They are
unique within a project; imported identities are namespaced/remapped atomically
with their references. Display names are editable labels, never identity keys.
An observation's identity is never recycled after retention eviction. Sequence
counters use checked arithmetic; exhaustion ends the stream with a diagnostic
instead of wrapping. Schema/version serialization is a subsequent task.

| Entity | Required values and lifecycle |
| --- | --- |
| Interface | Id, display name, transport kind, capabilities, bounded adapter metadata; discovered -> opening -> active -> stopping -> closed, or failed with a reason. A reconnect creates a new capture epoch, preserving the logical interface identity only when association is known. |
| Session | Id, configuration revision, source interfaces/epochs, clock origins, retention limits, loss totals; idle -> recording -> stopping -> complete, or failed. Replay creates a distinct epoch and preserves source provenance. |
| Observation | Id, session/interface/epoch, per-interface sequence, direction, timestamps, immutable captured bytes, original length when known, typed metadata and validity flags. Accepted observations are immutable. |
| Message | Id, ordered source spans, type/revision when classified, framing status, decoded fields and diagnostics. A message is a derived view; re-decoding creates a new revision without modifying source bytes. |
| Message type | Id, name, revision, matching/framing rules, field definitions. Edits create a revision; historical messages retain their original revision. Unknown and ambiguous classifications are explicit results. |
| Field | Stable id within its type, name, checked byte/bit range, scalar/byte representation, byte order, optional scale/unit and validation. A decode result is value, unavailable or error with reason; invalid is never numeric zero. |
| Chart | Id, named channels referencing field/type revisions, time basis, bounded sample window and optional moving average window. Invalid/missing samples create gaps; a positive bounded window is required. Raw values remain distinguishable from aggregates. |
| Transmit template | Id, name, revision, interface capability requirements, payload definition and editable fields. Draft -> validated -> eligible for explicit send. Any edit invalidates validation; RX bytes are never automatically a transmit instruction. |
| Schedule | Id, template revision, interface epoch, validated interval and enabled state. Disabled initially; explicit enable -> running; stop, interface loss or invalid config -> disabled with reason. Reconnect/reload never silently re-enables it. |

Derived objects retain bounded provenance spans, each containing observation id,
offset and length, with checked `offset + length <= captured length`. Spans may
cover several stream chunks or a subset of a datagram. An evicted observation
produces unavailable provenance, never a reference to newer bytes. A live derived
object either owns its charged source bytes or reports missing source explicitly.

## Shared observation envelope

Direction is Rx, Tx or Unknown relative to the selected interface. Unknown capture
direction is not guessed. Tx observations distinguish requested, submitted,
confirmed and failed; adapter submission is not proof of delivery. A received
loopback copy remains a distinct Rx observation with optional correlation id.

Each observation carries an ingest monotonic timestamp and optional source/wall
timestamp with declared units, origin and precision. Monotonic times are comparable
only within a clock epoch. Wall-clock adjustments never reorder per-interface
sequences or drive schedules. Cross-interface ordering uses a documented display
tie-break (ingest time, interface id, sequence), without claiming wire ordering.
Replay source time and current ingest time remain separate.

Captured bytes are immutable; original/wire length may exceed captured length
when truncated, but must not be smaller. Unknown original length remains unknown.
Truncation is visible and missing bytes are never padded. HEX/ASCII inspection
uses these bytes; decoded edits belong to a draft, not the captured observation.

Transport metadata is a tagged, bounded value carried alongside the envelope:

| Kind | Metadata and boundary semantics |
| --- | --- |
| Ethernet | Link type, captured/original length and optional VLAN/address information; one link frame per observation. Unsupported link parsing preserves raw bytes and reports unsupported. |
| TCP | Local/remote endpoints, connection epoch, optional sequence/flags and capture origin; arbitrary ordered stream chunks, not application packets. Reassembly reports gaps, retransmissions and overlaps; it never invents missing bytes. |
| UDP | Local/remote endpoints and truncation; one datagram, including an empty datagram, per observation. |
| Serial | Logical port, active configuration revision and optional line/error status; arbitrary byte chunks requiring stream framing. RS-232/422/485 and USB CDC share this stream contract. |
| USB | Active transfer versus passive capture origin, device's session-local alias, endpoint, transfer kind, direction and completion status; one bounded transfer/chunk with reported boundaries. HID report id is optional metadata, never silently stripped. |
| CAN | Channel, standard/extended arbitration id, classic/FD kind, remote/error flags, DLC and actual data length; one frame. Validate identifier width and permitted DLC/data combinations, including remote frames with no data. |

Transport tags support backend diagnostics and protocol selectors. GUI tables,
HEX views, fields, charts and templates consume the same envelope and capability
descriptors; they do not depend on concrete backend classes. A capability can be
unsupported, unavailable with reason, or available. Absence of TX capability is
not an error in an RX-only interface.

## Processing and transmission

Background adapters emit observations into bounded queues. Framing consumes
datagram boundaries or per-interface/connection stream state; it emits complete,
incomplete or rejected messages. A capture gap invalidates affected partial
frames and produces a resynchronization diagnostic before more decoding.
Classification assigns zero, one or multiple candidates explicitly. Decoding
checks ranges, byte order, numeric conversion and arithmetic before access.
Storage and application workers publish bounded read views to the GUI.

One-shot transmission requires an explicit user action against a validated
template and active interface with TX capability. A schedule requires a separate
explicit enable action. Bounded outgoing requests reference a template revision
and interface epoch; workers revalidate these before submission. Invalid or stale
requests fail visibly. Stop cancels pending scheduled requests before further
submission; an operation already submitted may complete and must be reported.
This adapter limitation must never be described as retracting bytes on the wire.
No decoding, storage writes or scheduling executes on the GUI thread.

## Bounds, loss and errors

Every queue, payload, metadata collection, partial stream, provenance list,
diagnostic buffer, sample window and retained observation/message set has finite
count and byte limits configured before allocation. Reject zero/invalid limits
where they would prevent progress. Exact defaults belong to configuration work.
Shared bytes are charged once to the owner; pinning data counts against its
budget and cannot bypass retention. Oversized input is rejected before copying.
Live history evicts oldest retained entries under either count or byte pressure.
Disk writes use a bounded queue and configured session/storage budget; reaching
that budget stops recording visibly, independently of live capture.

| Condition | Observable outcome and recovery |
| --- | --- |
| Backend reports loss | Separate backend-loss counter, interface/epoch and available count/time range. Unknown count stays unknown; no fabricated zero. |
| Ingress queue full | Drop incoming observation; increment application-drop counter and mark a gap for framing. Loss notification uses bounded/coalesced counters, so a full queue cannot hide it. |
| Display refresh skips entries | Increment display-decimation count separately. It does not imply capture or recording loss. |
| History eviction | Increment retention-eviction count; views show expired references. It is distinct from ingress drops and display decimation. |
| Storage queue full or write failure | Explicit recording-loss/error state with affected range when known; continue live capture within its own limits. Never claim a complete recording. |
| Malformed metadata/length | Reject with bounded diagnostic and validation-rejection count. Preserve available raw bytes only within budget; do not publish an invalid envelope. |
| Invalid field offset/overflow | Field error, source bytes unchanged; other independent fields may decode. Never read outside a source span. |
| Partial frame exceeds bound | Discard bounded partial state with a framing error; resynchronize by the configured framing rule. |
| Interface loss | Mark failed/closed, stop schedules, reject pending TX, report incomplete stream state. Explicit reopen is required. |
| Counter exhaustion | Emit terminal diagnostic and stop affected epoch instead of wraparound; diagnostics themselves remain bounded. |

Loss counters are scoped by session/interface/epoch and stage; different stages
must not be summed as if they represented distinct physical packets. One reported
backend gap can affect multiple derived messages. Diagnostics carry code, stage,
severity, bounded text, timestamp and optional entity references. Device data and
untrusted file text never become executable configuration or unbounded logs.

## Worked traces

All examples are synthetic. Each uses the same Session -> Interface ->
Observation -> Message -> Field -> Chart path; a transmit template is separate.

| Transport | Observation and derivation |
| --- | --- |
| Ethernet | Rx E1 contains `01 02 03 04`, link type supplied by adapter, captured/original length 4. With an explicit synthetic raw-frame classifier it yields M1 span E1[0..4]. It is not claimed to be a valid Ethernet header. A real link parser reports malformed header while HEX inspection remains available. |
| TCP | Rx T1=`41 42`, T2=`0A 43 0A` on one connection epoch. Newline framing yields `41 42 0A` from T1[0..2]+T2[0..1], then `43 0A` from T2[1..3]. A gap between chunks invalidates the partial first message. |
| UDP | Rx U1=`01 00 2A`, local/remote endpoints recorded, length 3. Datagram framing yields M2 span U1[0..3]; field at offset 2 length 1 is 42 and can feed a chart. Empty U2 remains a distinct empty datagram. |
| Serial | Rx S1=`AA 02`, S2=`10 20` under one config revision. An explicitly configured header/length rule yields M3 with spans S1[0..2]+S2[0..2]. Its length-two payload is `10 20`; timeout or reconnect produces incomplete status. |
| USB | Rx H1=`01 7F` on a simulated HID interrupt IN endpoint, completed, report id 1. Transfer framing preserves both bytes; a field selects offset 1 length 1. A failed transfer reports failure rather than valid zero-valued data. CDC instead uses the serial stream contract. |
| CAN | Rx C1 classic standard id 0x123, DLC 2, data `10 20`. Frame boundary yields M4; two byte fields can feed separate channels. Extended id uses the same path; an invalid standard id or DLC/data mismatch is rejected. |

For each valid message a named MessageType revision determines field layout.
For a big-endian two-byte field `10 20`, the unsigned value is 4128; selecting
offset 2 length 1 from that payload yields an error, not a value. Editing the
value creates a template draft; only explicit send can produce a Tx request.
A chart with moving-average window 2 on values 10, 14 yields warm-up 10 then 12,
with warm-up marked; a missing sample creates a gap and resets the window so an
average cannot silently span missing data. Session recording stores source and
revision provenance subject to its independent limits. Replay never arms TX.
