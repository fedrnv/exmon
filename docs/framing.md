# Configurable message framing

`exmon_protocol::framing::Framer` derives application messages from immutable
capture/stream units. Use one worker-owned instance per ordered stream,
direction, capture epoch and framing configuration revision. The caller owns
that association and bounds the number of instances. Native TCP/serial read
chunks are not messages. Passive TCP data must first pass reconstruction;
forward every permanent Gap to the framer. Framing performs no I/O, spawning,
scheduling, classification, field decoding or transmission.

Construct with `from_protocol(&protocol)` or `new(rule, escape)`. The domain
configuration and runtime use the same `Framing::limit` validation. Existing
schema-one files and four original framing variants still load. Optional
`protocol.escape` defaults to None and is omitted when absent on save; its fields
are unsigned bytes `byte` and `xor`. New `markers` and `idle_gap` variants remain
explicit unsupported input for older readers, rather than being silently mapped
to a different rule. Loading any configuration still cannot enable TX.

## Rules

| Configuration kind | Normalized message bytes | Completion and recovery |
| --- | --- | --- |
| `datagram` | Entire decoded logical native frame | `push_frame` supplies one complete boundary, including an empty frame. Generic stream `push` is rejected. |
| `fixed` | Exactly `length` decoded bytes | Emit at the configured length; after explicit loss, restart alignment at the next captured byte. EOF drops an incomplete candidate. |
| `length_prefix` | Payload after the `offset + width` header | Read unsigned width 1/2/4 in configured endian; value counts payload bytes, excluding header. Zero payload is valid. If header plus payload exceeds max_length, discard one decoded byte with InvalidLength and try the next header position. This declared resync policy does not prove protocol alignment. |
| `delimiter` | Decoded bytes before the unescaped delimiter | Delimiter is 1..16 bytes. Emit empty messages between delimiters. An oversized candidate is discarded through the next complete unescaped delimiter, retaining only bounded delimiter lookahead. |
| `markers` | Decoded bytes between unescaped start/end markers | Each marker is 1..16 bytes. Ignore/report leading noise. A nested start abandons the prior candidate; possible end-marker prefixes delay that decision, and a complete end wins. Overflow discards until a new start or complete end. Markers are not shared or overlapped between adjacent frames. |
| `idle_gap` | Decoded bytes in one monotonic burst | A positive gap_ns closes the previous burst before new bytes at that exact deadline. Overflow discards the burst until the gap. Empty input does not extend the burst; explicit EOF closes a valid pending burst. |

`max_length` counts the entire decoded frame, including delimiter, marker or
length-header bytes, not just normalized payload. Fixed length is its own limit;
native frames have the existing 65536-byte domain limit. Escaped wire bytes may
take twice the decoded space. A prefix width/header and both markers must fit
the frame limit. Delimiters/markers cannot contain the reserved escape prefix.

An escape prefix consumes the next raw octet and XOR-decodes it. That decoded
octet is protected from delimiter/marker recognition, even across input chunks
or observation IDs. Length fields count decoded bytes. Dangling escapes are
reported and their candidate is discarded at gap/EOF/idle completion; no zero
byte or valid partial message is invented. There is no implicit checksum or
protocol validation: a framed candidate can still be unclassified or invalid
at later layers.

## Source ranges and lifecycle

`push(Source { observation, offset }, monotonic_ns, bytes)` accepts arbitrary
read fragments of one stream. For a split within one retained unit, keep the
same source ID and advance its offset. Holding input bytes, IDs/offsets and
semantic times constant makes read partitioning irrelevant to the output.
`push_frame` boundaries and idle times are semantic input, so changing them can
legitimately change messages. Source IDs refer to retained input units; for
reconstructed TCP they may be derived reconstruction-data units, not a claim of
an original TCP packet offset. Retain the original observations and any upstream
provenance index separately.

Each Message contains normalized immutable bytes, whole-frame captured stream
range `wire`, ordered/merged observation `sources`, one absolute raw range per
normalized byte in `byte_ranges`, and a monotonic completion time. Source spans
include discarded framing delimiters/headers and escape prefixes. A protected
decoded byte can map to two raw positions in different observations. Empty
native frames retain an empty source span identifying their observation.
The stream coordinate counts submitted captured bytes, including explicitly
discarded captures; it is not physical TCP sequence space and does not invent
coordinates for missing bytes. Diagnostic ranges identify discarded captured
regions/boundaries. Discard tails use exact absolute stream ranges without
retaining unbounded payload or per-observation history.

`observe(id, &observation)` forwards domain monotonic time and bytes. A known
capture truncation or incomplete USB completion invalidates pending state and
skips the incomplete unit with Gap/IncompleteCapture. Native framing rejects
TCP/serial observations because their read chunks are not native frames. A
native observation with unknown original length remains inspectable with an
UnknownOriginalLength diagnostic; that diagnostic must not be presented as
proof of complete on-wire capture. Source capture timestamps never drive idle
timers. The input observation is unchanged.

`gap(now)` clears partial/protected/discard state and emits an explicit boundary;
never omit a backend/application loss notification. `tick(now)` closes due idle
bursts at their exact deadline, even if called later. `finish(now)` reports EOF,
releases pending references and seals the instance. Construct a new instance
for another epoch/configuration. Backward time, oversized input and source or
stream arithmetic overflow fail before mutation. Marker/fixed/delimiter EOF
does not turn an incomplete candidate into a complete message.

## Bounds and verification

One input call accepts at most 65536 raw bytes. Pending decoded state is bounded
by the configured frame limit (hard 65536); at most one transient extra cell is
used to detect overflow. Escape state adds one source position. Control patterns
are at most 16 bytes; marker recovery keeps at most start+end-1 lookahead cells.
Length-header resync uses constant-width reads and O(1) front removal. No input
chunk-count limit changes the output for one-byte fragmentation. Each decoded
cell owns at most two source positions; capacity growth stays within a fixed
multiple of the hard limit. `stats()` exposes current pending counts and discard
state. These are per-instance state bounds, not a process RSS ceiling.

Messages/events are returned to the caller rather than queued internally.
Per call, total returned message payload is bounded by old pending decoded bytes
plus new raw input, and event count by twice input length plus four (finish/tick
are constant-sized). Provenance output is proportional to those bounded bytes.
Callers must consume results through bounded downstream storage; retaining every
message or creating unbounded instances would defeat the overall application
budget. Debug output redacts message/configuration bytes; explicit byte/source
inspection remains a deliberate access to captured content.

Verification exercises all read partitions of short independent examples,
all pairs of binary markers of lengths 1..3, escaped cross-observation bytes,
all length widths/orders, native/idle semantics, loss/EOF/oversize recovery,
the complete accepted 132-case traffic corpus through production framing,
real TCP reconstruction Data/Gap output, exact source ranges, maximum
65536-byte messages and deterministic hostile inputs. Configuration tests cover
old defaults and new variants/escape roundtrips, invalid values and launch-safe
schedules. This is Windows VPS pure processing qualification; physical/native
stand/platform validation and later GUI/classifier integration remain separate.
