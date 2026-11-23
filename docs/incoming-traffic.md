# Incoming traffic inspection

The desktop prototype sends bounded captured batches to a separate
`exmon_app::incoming::Worker`. That worker validates project definitions, compiles
the existing classifier/codec, frames streams and prepares fields and endpoint
labels. The GUI only retains prepared immutable rows and formats visible HEX and
ASCII lines. No capture, classification, decoding, storage writes or transmission
runs on the GUI thread. The default synthetic sample definitions recognize the
32-byte sample and eight-byte CAN sample. Their sequence/counter fields exercise
the real decoder; they do not describe an unknown physical device protocol.

Raw traffic contains original observations. Type tabs contain normalized message
instances, identified by the classifier's stable rule or automatic-shape key.
TCP/serial fragments remain raw until the explicitly configured framing policy
produces a message. The prototype uses fixed32; `Worker::configured` accepts a
validated project and an explicit optional stream framing/escape policy. Missing
framing, stream gaps/context changes, invalid timestamps and malformed boundaries
never turn arbitrary chunks into complete messages. Source generation, interface,
epoch, observation sequence and framing source spans remain attached to instances.
Unknown original lengths stay unknown, including when an earlier stream fragment
has unknown completeness. Incomplete native records retain their separate group.
Invalid numeric values and field/checksum failures remain explicit, without zero
substitution. Unsupported endpoint metadata is labelled unavailable, rather than
inventing Ethernet addresses or USB destinations.

Click Raw traffic or a type tab, then an instance row. Selecting a tab initially
selects its latest retained instance. Later rows, names and counters do not move
that selection. One selected immutable instance remains pinned after history
eviction, with an explicit notice. Pause display discards newly delivered view
rows while capture, processing and counters continue. Resume shows subsequent
rows; it does not replay a paused backlog. Source restart clears selection and
rejects older generation results. Clear view clears retained inspection history.

The detail inspector shows timestamp/direction in the instance list, source and
destination metadata, capture/original/normalized lengths, protocol and revision,
HEX/ASCII and decoded fields. Source details collapse by default in a narrow
panel. Scroll the detail area to reach every field; HEX uses eight bytes per row
in narrow panels and sixteen otherwise. Field previews mark omitted long byte or
text values explicitly; full normalized data remains in the virtual HEX view.
Long labels/field values truncate with hover details. No payload is written to
diagnostic logs or preferences.

Rename type is an explicit worker command. It validates the name, updates the
configured protocol name or canonical automatic-group name, and publishes a new
classifier revision. Old instances retain their original revision/name. The
worker exposes an immutable configuration snapshot for later explicit saving;
this view does not silently write a project or add names to layout preferences.
Copy format to outgoing retains the received normalized bytes, protocol/field
definitions and source provenance as one immutable draft. It supplies no target,
TX ticket/permission or schedule and performs no send. An explicit copy request
also creates a tab in the [outgoing editor](outgoing-editor.md), where validation,
target binding and transmission remain separate user actions.

Input is limited to eight queued batches of at most32 observations and65536 total
payload bytes each. Processing caps each observation at64 framed output messages,
and retains at most96 prepared rows/1MiB per output batch, with eight output
batches. Extra normalized outputs/oversized prepared rows count as processing
skips; input admission drops and output view skips are distinct. These are
presentation-processing limits, not capture loss. Up to256 configured type groups
and64 automatic shape groups retain bounded counts and elapsed-worker-average
message rates. Additional automatic shapes increment group overflow and remain
inspectable in raw traffic. There is no unbounded learned group cache.

The view retains at most1024 combined raw/message entries and8MiB of charged
prepared data. A pin and an outgoing draft each retain at most one admitted entry;
compiled configuration/protocol definitions have a separate1MiB JSON admission
budget. Shared Arc allocations may be conservatively charged more than once.
The existing framer's input/pending/source limits bound temporary work separately;
trusted native libraries, renderer memory and caller-retained snapshots have their
own budgets. Capture backend loss, application drops, capture display decimation,
processor input drops, processing skips, view skips, group overflow, history
eviction and pause skips are reported separately. Rates describe processed
messages after filtering/admission, not inferred wire throughput.

Run the `incoming` app and GUI integration tests with `--locked`. GUI tests use
the real background capture/processing workers, click production controls and
check a bounded one-second responsiveness scenario after warm-up (>20 frames,
200ms maximum frame). Wgpu images cover900x600/1360x900, dark/light and1x/1.5x DPI;
the narrow detail screenshot scrolls to parsed fields. Run
`cargo run -p exmon-gui --example native_incoming_smoke --locked` for the hidden
native Windows screenshot smoke with an eight-second bound. Detailed streams and
images stay in ignored target output. These checks qualify the Windows virtual
prototype; physical/native stand qualification, other OS/packaged targets and
broader performance/soak or release acceptance remain separate035/036/037 tasks.

The worker also prepares checked numeric engineering projections for
[chart tabs](charts.md). Batches reach the independent chart worker before
incoming display pause/retention; raw observations remain unchanged.

[Validated settings](settings.md) provide staged profile/binding/safety edits and
a selected-raw-capture preview through the same incoming processor. Successful
Apply stops capture/TX and clears stale source generations; restart explicitly.
