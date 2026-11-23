# Outgoing templates and explicit transmission

The right desktop panel provides one tab per outgoing template. Create a raw
message, create a zero-initialized body from a configured protocol, or click
Copy format to outgoing on a normalized incoming instance. The copy preserves
received bytes, immutable definitions, source revision/quality and capture
provenance. Incoming data remains unchanged. No creation, copy, edit, validation,
checksum repair or connection sends a payload or activates a schedule.

The Windows prototype exposes only clearly named synthetic byte-stream targets
Mock TX01/02. They are independent of the selected RX transport; copying passive
Ethernet, CAN or USB bytes does not supply a physical transmit port. Select the
target, enable Allow mock transmission, then explicitly Connect mock TX. Permission
alone does not connect. Send once submits one current validated revision. It has
no automatic retry. The public application factory path retains the existing
exact selected identity/capability/permission contract for later approved adapter
integration; physical target controls are not implied by the synthetic selector.

The mock TX session belongs to this editor instance, with a fresh binding epoch
on explicit connection. Received references keep their original capture session.
New capture runs, RX selection/view changes, Stop capture, permission revocation
and Disconnect TX revoke the outgoing connection. Application restart starts
unbound with schedules disabled. Neither payloads nor permission/tickets/active
schedules persist in layout preferences.

## Editing and validation

`exmon_app::outgoing::Editor` owns a bounded background editing worker. It parses
HEX, compiles validated definitions, applies raw/engineering/enum field edits via
the existing codec, repairs checksums only on explicit request, rebuilds framing
and publishes immutable snapshots. The existing TX worker owns adapters, final
validation, one-shot submissions and the periodic engine. The GUI performs no
codec/framing/capture/storage or timer work.

Choose HEX editor or Field editor. Apply the current edit explicitly. In the
field editor, raw typed values preserve integer precision; Engineering value
uses the codec's checked inverse transform with a1e-9 tolerance. Enum label uses
the exact configured name. Read-only/derived fields, malformed values, range and
quantization failures are rejected. Invalid applied drafts show field/checksum/
framing errors and cannot send. Failed syntax/type/ownership edits retain prior
bytes with an explicit dirty/error state. Validate current data discards uncommitted
editor text and validates those retained bytes; it does not silently repair data.

Whole-message HEX can change length within the protocol's admitted bounds.
Existing checksum output bytes cannot be manually changed or deleted. Changed
shared-prefix runs pass through the codec; its updated checksum outputs are
copied into the resized candidate. Unprotected unknown tails remain intact unless
explicitly edited. A resize cannot invent a missing protected checksum output.
Static checksum coverage or fields extending past a shorter body remain invalid;
preexisting mismatches require explicit Repair checksums. Fixed-length framing
still requires the declared exact size. No second checksum algorithm is added.

Length-prefix protocols require an explicitly supplied Preserved framing header;
the editor never guesses bytes absent from a normalized copy. The existing
encoder updates only the declared length field and reconstructs one qualified
frame. Other framing modes require an empty preserved header. Normalized incoming
HID/CAN/USB data does not imply physical output conventions or permission.

Every text change revokes actual queued TX authority before acknowledging the
change, including malformed/oversized input and admission failure. Per-template
edit generations prevent stale worker results overwriting newer text, while
independent template creations remain independent. A short publication fence
protects final registration from newer edits; no decoding or native I/O runs
under that fence. Serial registration acknowledgement avoids invalidating other
queued template registrations. Validation and explicit binding can advance a
template revision even when its bytes are unchanged.

`TxWorker::invalidate` revokes tickets/grants immediately and retains an idle
selected port for fresh validation. Canceling a pending transfer may close that
port and require explicit reconnect. Editing while an asynchronous connection is
still opening cancels that opening; wait for the confirmed connection or reconnect
explicitly. Cached confirmed tickets avoid treating snapshot lock contention as a
functional send/setup error; the actual TX API and final adapter dispatch always
recheck current authority. No cache can resurrect a revoked ticket.

## Periodic transmission and results

Configure interval, optional finite count/lateness, drift and missed-deadline
policy, then explicitly Start schedule. The existing periodic engine starts one
interval after activation. Blank count means unlimited and blank lateness means
the interval default; malformed numeric strings never become zero/defaults.
Changing an option stops the selected schedule immediately. Stop schedule revokes
its handle independently of the editor command queue and fences older queued
enables. Independent tabs have independent enable versions. Any template edit,
interface/configuration loss, disconnect or shutdown revokes existing grants;
there is no automatic rearm. See [deadline semantics](scheduling.md).

The panel shows the actual confirmed binding, validation/dirty state, observed
full-acceptance TX count, last known/partial/unknown result and schedule attempts/
misses/stop state. Driver acceptance is not on-wire delivery. History loss or
eviction marks counts as observed minima rather than inventing missing outcomes.
Backend Busy never triggers a one-shot retry. An operation already leased before
revocation can finish honestly; Stop cannot recall accepted bytes.

## Bounds and verification

At most64 templates and1MiB conservative aggregate charged drafts, compiled
definitions and prepared text are retained. Editor commands are limited to eight;
body/header to65536 bytes and each HEX/value string to196608 bytes. Counts,
identity changes and time arithmetic retain the existing checked/saturating
contracts. Template snapshots share immutable Arc data. UI text/widget work has
explicit finite input limits; adapter/native/renderer state, external retained
snapshots and existing TX history have their separate budgets. No raw payload
logs, implicit project save, dynamic code or backend inference is introduced.

Focused app/GUI outgoing tests cover actual clone-edit-validate-bind-send bytes,
invalid/readonly/checksum/framing/header paths, multiple templates, schedules and
queue/permission/source-loss/cancellation cases. The direct transmission and
scheduling suites protect the new invalidation boundary; incoming/shell/filter/HID
and implemented transport regressions protect shared consumers. GUI interaction
and wgpu screenshots cover900x600/1360x900, dark/light,1x/1.5x DPI, including normal
scrolling on small windows. Run the bounded hidden native Windows scenario with
`cargo run -p exmon-gui --example native_outgoing_smoke --locked` for an actual
received clone, field edit, explicit synthetic bind and one send. Other-platform,
physical/driver/electrical and packaged/release qualification remain035/036/037;
no such pass is inferred from the virtual Windows prototype.
