# Explicit one-shot transmission

`exmon_app::transmission::TxWorker` owns selected transport ports, preparation,
one-shot submission and bounded session attempt history on a background thread.
Construction starts an idle worker: it performs no discovery, open or send.
The [outgoing GUI](outgoing-editor.md) uses this command API with its separate
background editing controller. [Periodic scheduling](scheduling.md) uses this same worker and
submission path with separate explicit activation. Existing HID-specific controls keep
their separate selected-collection worker.

## Binding and explicit actions

`bind` is an explicit user action. Supply a nonzero `Binding` containing the
current logical session, interface, epoch, configuration and protocol revisions,
the current TX permission, an `Expected` selected identity, and a trusted factory.
The factory runs on the worker only after acquiring a revocable opening lease.
It returns the exact selected active `capture::transmit::Port`; it must never
fall back to another device or peer. A canceled queued factory is not executed;
an opening that already acquired its lease may finish and then be closed.

Factories are linked application code, not imported configuration, scripts or
dynamic plugins. Keep their discovery/open/accept work finite and inside an
explicitly selected setup action. Never construct an already active physical
adapter on the GUI thread merely to pass it to the worker. Physical open and
TX still require owner/user authorization and approved device settings.

After successful binding, a snapshot exposes a `Ticket` and actual identity.
Tickets belong to that worker instance and generation; recreating a worker
cannot revive an old ticket even when numeric IDs happen to repeat. A binding,
permission, configuration or template change revokes queued tickets. Callers
must supply revisions from their current trusted configuration state and issue
an explicit new bind after interface loss/reconnect. The API cannot detect an
application that fails to report an external configuration change.

`register` installs an immutable `TemplateSpec`; an existing ID requires a
strictly newer revision. Registration revokes earlier queued sends and creates
a new ticket only after successful worker validation. An invalid template leaves
TX revoked; explicit correction can install a valid newer revision while the
same connection remains available. Registration after Close cannot reopen it.
Read the current snapshot after registration completes. Neither binding,
registration, cloning, validation nor reconnect sends a payload automatically.

`send_once(ticket, template_id, revision)` is one explicit Send action. It queues
one attempt, without retries or schedules. At dispatch the worker rechecks the
worker/generation, session/interface/epoch/config/protocol key, exact immutable
template revision, actual native identity/state/capability, payload bounds and
field/checksum validation. The final atomic lease serializes submission entry
with revocation. A revocation that wins prevents submission; an already leased
operation may complete afterward and is reported honestly.

`close` revokes before processing the normal queue and never performs source I/O
on its caller. The worker cancels/tears down its selected port. Drop revokes,
closes and joins. Native operations still have their adapter-specific timeout;
there is no wire recall or hard real-time shutdown guarantee. Trusted sources,
factories and checksum extensions must honor their finite/nonblocking contracts.

## Raw and structured templates

A raw template contains explicitly reviewed literal API/wire bytes in `body` and
no `structured` definition. It still passes the selected backend's payload and
state checks. A structured template binds a protocol/registry/header snapshot;
the worker compiles the [message codec](message-codec.md), validates all fields
and declared checksums at registration and again before sending, then frames the
normalized body. Invalid fields/checksums return bounded indexed diagnostics;
they never become numeric zero. Checksum repair/editing remains explicit through
the message codec; Send does not repair a bad captured packet silently.

`TemplateSpec::incoming` clones normalized immutable body bytes with an explicit
format, protocol revision and `SourceRef`. It grants no send authority, changes
no observation and infers no target. Use the message codec to edit that copied
body and obtain a validated snapshot, then register a new template revision.

`protocol::outbound::encode` reconstructs datagram, exact fixed-length,
delimiter, marker, length-prefix or idle-burst framing. For length-prefix rules,
provide the exact `offset + width` header bytes: unrelated header bytes remain
unchanged and only the length field is regenerated. Other rules require an empty
header. Escape decoding semantics match the receiver; payload control-prefix
bytes are protected when necessary. The encoder verifies exactly one intended
message through the production Framer, rejecting ambiguous/unrepresentable
unescaped sequences instead of sending a different message. Both decoded-frame
and actual 65536-byte submission limits apply. Idle-gap templates reject an early
next Send with Busy instead of scheduling it; a new explicit connection resets
that connection's gap state. Physical timing still needs stand qualification.

Formats preserve transport-specific meaning:

| Format/port | Contract |
| --- | --- |
| Serial/TCP | One bounded write; actual accepted prefix returned, including zero. No implicit suffix retry. TCP fixes one connection/epoch, never another server peer. |
| UDP | Explicit exact destination, connected-peer consistency and IPv4/IPv6 size limits; empty datagram is a valid zero-byte acceptance. |
| HID | Literal Bytes(Hid) uses reviewed HIDAPI report-ID-first output bytes. `HidUnnumbered` explicitly prepends one zero to a normalized body. It is rejected on a numbered port. Adapter validates numbering/capacity on the exact selected collection. |
| CAN | Typed classic identifier/flags/DLC/data, including RTR with no payload. Error-frame TX, untyped CAN bytes and unsupported FD modes are rejected. Success means driver acceptance, not bus ACK. |
| USB bulk/interrupt | Exact selected device/configuration/interface/alternate/OUT endpoint. Submission stays Pending until matching controller completion. Counts, timeout, stall, removal and cancellation remain distinct. |
| Passive capture | Ethernet/usbmon observation does not supply a transmit port or permission. No raw injection or vendor backend is invented. |

## Session history and resources

Each attempt/event carries its monotonic sequence, optional monotonic timestamp,
attempt ID, binding, template/revision, optional source reference, requested byte
count when the template can be resolved, and explicit outcome. Unresolved stale
or queue-rejected requests retain an unknown count. Rejections have no successful TX observation.
Pending async submission is distinct from terminal acceptance/failure. Complete
outcomes retain actual known counts; a short/zero count is not full completion
unless the requested payload was itself that size (e.g. an empty UDP/RTR unit).
Backend failures or unreconciled cancellation/timeouts preserve an unknown count
as None, never fabricated zero. The worker's one-second pending watchdog cancels
an unresponsive trusted port and revokes the binding.
Transient Busy preserves an otherwise unchanged live port for a later explicit
Send action; it never starts a retry itself.

Known accepted prefixes use the existing domain `Observation` with Direction::Tx
and typed metadata in the same session/epoch. These are API acceptance records,
not independent physical capture or delivery proof; HID includes its API ID
convention. Original wire length remains unknown. Failed USB partial completions
retain their prefix with incomplete metadata. Already leased synchronous writes
may report acceptance after Close. Shutdown checks for an available terminal
async completion; otherwise cancellation records unknown acceptance. Unexpected
worker panic revokes tickets and exposes unreconciled outcome loss, without
inventing a successful observation. Native code/panic hooks are trusted and must
not expose private payloads.

The command queue holds four items. Admission limits normalized/wire data to
65536 bytes, field/checksum definitions to their existing bounded contracts and
templates to 64 with a conservative aggregate 1 MiB charge for retained data,
compiled nodes/indexes and copies. Raw/unused protocol allocations are released
after compilation. Native factory/algorithm state and adapter queues have their
own trusted bounds; this is not a total process RSS guarantee.

History retains at most 128 events and 1 MiB of charged data; oldest entries are
evicted with a separate counter. Queue rejection and unreconciled history loss
are separate counters. Readers copy at most four records per call and receive
fixed/redacted Debug diagnostics. Sequence/generation exhaustion revokes rather
than wraps. Callers must bound retained snapshots/records and must not persist
active tickets, pending sends or enabled authority in configuration.
Snapshot reads return None during brief lock contention instead of reporting
false zero counters; keep the last displayed snapshot. Health counters saturate.

Run the `transmission` app tests, `transmit` capture tests and `outbound` protocol
tests with `--locked` for actual Windows loopback and deterministic audited source
fixtures, independent frame vectors, cancellation barriers, partial/async/error
outcomes, revision identity and resource limits. Physical/native platform and
electrical qualification remain EXM-035/036 on the later stands.
