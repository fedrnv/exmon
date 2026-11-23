# Preserved message editing and checksums

`exmon_protocol::codec::Codec` compiles a protocol's field definitions and
checksum plan. Decode normalized framed message bytes on a worker into a
`Draft`. It owns bounded original/current buffers and borrows its immutable
codec, so subsequent configuration/registry changes cannot reinterpret it.
The source observation is never modified. This module does no I/O or scheduling.

`Draft::encode` reproduces current bytes exactly, including undecoded bytes,
truncated fields, invalid text, nonfinite IEEE encodings and bad checksums.
Decoding or encoding never repairs an incoming message. `original` remains the
initial snapshot. The initial revision is zero; successful byte changes increment
it once per transaction. No-op edits retain the revision; exhaustion fails
without mutation. The draft retains no undo or observation history.

## Explicit editing and validation

`Draft::edit` applies an ordered batch of `Edit::Field`, `Number`, `Label` or
`Hex`. Field operations use the exact raw/engineering rules in
[binary fields](binary-fields.md). Engineering edits require an explicit absolute
tolerance; inspect the returned numeric report for the actual quantized value.
HEX replaces existing bytes at a checked offset, with no insertion, deletion or
padding. It accepts contiguous hexadecimal pairs and ASCII whitespace between
pairs. Prefixes, separators, odd nibbles and non-ASCII whitespace are rejected.
Within one batch the last explicit edit wins.

HEX may deliberately create an invalid draft for inspection and repair. Every
successful transaction returns all field/checksum diagnostics and re-evaluates
derived fields. Structural/edit/algorithm failures roll back the entire batch,
including earlier field and checksum writes. Constants, enums, raw constraints,
bad text and derived failures remain visible; errors are never converted to zero.
Existing unrelated validity errors do not prevent repairing a different field.

`inspect` returns a bounded report. `prepare` returns either that report in
`Invalid`, or a `Validated` byte/revision snapshot. Every field's validity rules
and every declared checksum must pass. Raw bytes/text have no numeric chart
value; raw IEEE NaN/infinity are numerically unavailable but remain valid unless
constraints reject them. A numeric projection precision error alone does not
invalidate an otherwise valid raw wire value. Derived computation failures do
invalidate the derived field.

The validated snapshot owns its bytes: subsequent edits cannot alter it. It is
not interface permission, framing serialization, a send command or schedule
authorization. The application must bind the intended current template/config
revision and interface epoch, validate actual transport capabilities/lengths and
require explicit user authorization immediately before submission. Codec validity
does not prove an unknown protocol, physical device or electrical setting valid.

## Declarative checksum plan

Protocol schema version 1 has an optional `checksums` list, omitted when empty:

```json
{
  "id": "sum",
  "algorithm": "sum8",
  "offset": 3,
  "length": 1,
  "endian": "big",
  "ranges": [{"offset": 0, "length": 3}]
}
```

Offsets refer to normalized message bytes. Input ranges are positive, ordered,
disjoint and read in declaration order. Own-output coverage is rejected.
Checksum outputs must not overlap each other. An inspection field may be wholly
contained in an output; a field straddling its boundary is rejected. All user
edits to checksum-owned bytes/fields are read-only, even if they propose the
existing value. Checksums can also cover otherwise undecoded bytes.

Coverage of another checksum's output establishes a dependency. Cycles are
rejected; updates run in deterministic topological order. A checksum is updated
only when its covered bytes change, including changed prerequisite outputs.
Independent outputs and preexisting mismatches outside affected dependencies
remain untouched. `repair_checksums` explicitly recalculates every checksum.
If a required covered/output range is missing, the update fails atomically;
unchanged truncated bytes remain inspectable and fail preparation.

The domain validator is shared by configuration and runtime compilation.
Definitions have unique IDs/algorithm keys of 1..128 ASCII alphanumeric,
underscore or hyphen bytes. Output widths are 1/2/4/8 with explicit endian.
Unknown algorithm availability is resolved at codec compilation, not JSON load;
portable definitions may name application extensions absent on this host.

## Constrained extension boundary

`checksum::Registry` contains only trusted application-linked Rust `Algorithm`
implementations. Registration fixes an immutable output width. The callback gets
`Input`, a finite, repeatable iterator exposing only declared covered bytes, and
returns a u64 or fixed `Failure`. The host alone writes the declared output and
rejects an out-of-width result. Unknown algorithms/width mismatches fail
compilation. Ordinary unwind is caught as a fixed error; abort/OOM is not caught.
Rust's configured panic hook still runs during unwind, so trusted extensions
must not panic or include sensitive input in diagnostics.

Implementations must be pure, deterministic, bounded, nonblocking and panic-free.
Native code is trusted: this interface does not sandbox its CPU, allocations,
internal state or I/O. No scripts, dynamic libraries, external lookups or imported
executable callbacks are loaded from configuration. Only explicitly registered
code can run. The builtins do no I/O: `sum8` is the byte sum modulo 256, `xor8`
is byte XOR, and `crc16_ccitt_false` uses width 16, polynomial 0x1021, initial
0xffff, no reflection and xor-out zero. For ASCII `123456789`, they produce
0xdd, 0x31 and 0x29b1 respectively. Endian affects storage, not coverage order.

## Resource and verification bounds

Limits are 65536 message bytes, 256 edits/batch, 65536 total replacement bytes,
196608 total HEX input bytes/batch, 32 checksums/protocol, 32 ranges/checksum,
1 MiB aggregate declared coverage and 64 registered algorithms. Admission precedes
cloning; source/current/candidate buffers and reports are bounded. Builtin update
plus final inspection visits at most twice the declared coverage, with eight
fixed bit steps/byte for CRC. Callers must bound draft/schema counts, queues and
report retention and keep processing off the GUI thread.

Run `cargo test -p exmon-protocol --test codec --locked` for independent edit and
checksum vectors, 132 real framing/classification corpus cases, deterministic
hostile data/definition mutations, extension failures, resource bounds and exact
rollback. `cargo test -p exmon-protocol --lib --locked` checks revision exhaustion;
`cargo test -p exmon-storage --test configuration --locked` checks persistence
and admission. These are Windows VPS/synthetic checks, not physical or macOS
qualification.
