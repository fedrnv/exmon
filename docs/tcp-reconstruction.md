# Bounded TCP reconstruction

`exmon_protocol::tcp_reassembly::Reassembler` is a passive processing component.
Call it on the capture/processing worker, not the GUI thread. It performs no I/O,
transmission, DNS lookup, thread creation or message framing. `push_observation`
uses the actual EXM-009 decoder on Ethernet DLT1 observations; `push` accepts a
validated normalized `Segment`. Socket observations contain application bytes,
not TCP sequence headers, and are explicitly unusable here. IP fragments,
unsupported links/protocols and incomplete/malformed TCP headers are also
unusable. Original observations remain borrowed and unchanged in their caller.

A `Context` supplies selected interface identity and capture epoch. A flow key
combines that context, canonically sorted source/destination IP+port endpoints,
and up to eight VLAN TPID/VID pairs. VLAN priority/DEI do not change identity.
Different interfaces, epochs and VLANs never share state. Flow IDs increase within
one reassembler instance; each incarnation has independent A-to-B and B-to-A
sequence spaces. Mixed address families and identical endpoints are rejected
because the latter do not identify two distinguishable wire directions.

Sequence numbers map into checked monotonic 64-bit coordinates relative to each
direction's current frontier. The low 32 bits are the wire sequence number.
Initial coordinates have a 2^32 bias to permit bounded older segments around zero.
The configured window stays below the ambiguous half-space; exactly 2^31 distance,
positions outside the window and arithmetic exhaustion are explicitly rejected.
SYN precedes data and consumes one sequence number; FIN follows declared payload
and consumes one after preceding bytes or explicit gaps have been committed.

If a direction starts without SYN, `UnknownPrefix` precedes its data. The first
observed sequence anchors that direction; older data cannot be prepended later.
A matching delayed SYN can adopt the anchor but does not retract the earlier
unknown-prefix report. A differing SYN in an already observed direction closes
the previous incarnation with a `Replaced` boundary. A peer SYN initializes the
other direction. Identical ISN reuse or late duplicates after closure cannot be
reliably distinguished without more capture history; no unbounded tombstones or
TIME-WAIT database is retained. New untracked data starts a new midstream ID.

The fixed overlap policy is first observed bytes win. Buffered intersections are
compared and reported as identical or conflicting. In-order input is delivered
around existing buffered spans, preserving earlier bytes. Bytes before the
committed frontier are never emitted twice or rewritten. Their overlap is
`UnverifiablePast`: no payload history is retained to compare them, including
late input for a previously committed gap. Conflicting and unverifiable overlap
set the flow's ambiguity flag independently of missing-stream completeness.

Out-of-order bytes wait for recovery. `Truncated` is a packet-level notice for
declared payload absent from the capture; it does not yet force a framing reset.
Missing bytes can still arrive. A bounded peer ACK can extend the known missing
peer range, but creates no bytes. Invalid/ambiguous ACK evidence is reported and
ignored independently of the segment's own data. ACK-only packets for unknown
flows create no state. Direction is determined by wire endpoints, never guessed
from local/remote labels or observation RX/TX.

Once loss becomes permanent, a `Gap` with exact sequence range is emitted BEFORE
following `Data`. A consumer must reset application framing across a Gap, an
unknown prefix or a flow incarnation boundary. Never concatenate data across a
Gap or fill it with zeros. Gap deadlines do not refresh while an existing hole
remains; retransmission/ACK floods cannot postpone them indefinitely. Timers run
before processing input at the same timestamp, so arrival at the exact deadline
follows the already committed boundary. `tick` also progresses timers without
traffic; `finish` explicitly flushes and ends all flows at capture end.

Each direction half-closes when FIN becomes contiguous. Both FINs close the flow;
the reverse direction can continue after one FIN. Repeated FIN at the same
position and ACK-only control at FIN+1 are valid. Declared payload extent is kept
separate from ACK/sequence evidence, so a reordered ACK including FIN's sequence
number does not manufacture an extra missing byte. A FIN may not truncate a
previously observed payload extent, including capture-truncated bytes.
A FIN contradicting prior data or another FIN closes with
`ControlConflict`; data extending beyond an established FIN is rejected. An
observed RST immediately aborts a tracked flow and flushes captured pending data
with explicit gaps. This is an observation policy, not proof that an endpoint's
TCP state machine accepted that RST. Closed records report reason, incomplete
status and ambiguity separately. Only both observed FINs, known prefixes and no
committed gaps yield a complete stream; this is not an integrity/authenticity
claim. Captured checksum/offload policy remains the caller's responsibility via
the existing decoder; reconstruction does not silently discard checksum-invalid
captures or authenticate SYN/ACK/RST control traffic.

All calls receive a caller-supplied monotonic processing time in nanoseconds.
Device/source timestamps never drive timers. Backward time fails without moving
the clock or changing flow state. Idle closure, capacity eviction, replacement,
RST, conflict and capture end all flush explicit known gaps. Unknown tails are
represented by incomplete closure rather than an invented finite range.

Limits are per instance and validated before use:

| Resource | Default | Hard ceiling |
| --- | ---: | ---: |
| Live flows | 256 | 1024 |
| Queued payload bytes, total | 4 MiB | 8 MiB |
| Queued payload bytes, one bidirectional flow | 128 KiB | 1 MiB |
| Queued chunks, total | 2048 | 4096 |
| Queued chunks, one direction | 64 | 128 |
| Declared input payload bytes | 65536 | 65536 |
| Sequence distance | 1 MiB | 2^30 |

All count/byte limits are positive. Per-flow bytes cannot exceed total bytes;
per-direction chunks cannot exceed total chunks; the sequence window must cover
the input cap. Gap and idle timeouts are positive (defaults 2s and 60s).
Out-of-order holes are counted before copying; pressure rejects the new bytes
with an explicit notice while preserving their known extent for later gaps.
In-order data bypasses queued storage. Flow capacity evicts the least recently
seen flow, using flow ID to break ties deterministically. No recursion, unbounded
history or internal output queue exists. Expiry scans bounded live metadata and
only removes/reinserts due flows.

Returned events are separately owned by the caller. For each call their data is
bounded by previously queued bytes plus at most one input payload. Event count
is bounded by `Limits::output_event_bound()` (2*total_chunks + 8*flows +
2*direction_chunks + 32). Fixed flow keys, bounded map nodes, a bounded temporary
hole list and finite event-vector capacity account for metadata beyond payload
limits; these are not an OS process RSS limit. Consume/move results into a bounded
downstream channel rather than accumulating results indefinitely. `Stats` exposes
live buffer/count usage, emitted bytes, committed gaps, overlaps, data-segment
rejections, evictions, ignored untracked controls and unusable observations.
ACK rejection notices do not imply that their segment's valid data was rejected.
Debug output redacts endpoints and payloads. Call `finish` to observe pending
boundaries before dropping the engine; drop frees memory but cannot return events.

Local tests run on Windows without devices. They cover actual IPv4/IPv6 decoding,
options/extensions/VLAN, both directions, ordering/overlaps/gaps, wrap, controls,
truncation and timer edges. A separate per-byte first-seen reference checks 128
deterministic randomized overlap orders. Resource tests fill 8 MiB globally,
1 MiB per flow, 4096 chunks and 1024 flows, then verify rejection/eviction and
bounded flush output. Deterministic hostile segments and wire mutations exercise
the production boundaries. These checks are not the independent EXM-034 fuzz/
security gate, a physical/macOS/Linux-native qualification or a release claim.

Sequence/control reference: [TCP specification, RFC 9293](https://www.rfc-editor.org/rfc/rfc9293.html).
