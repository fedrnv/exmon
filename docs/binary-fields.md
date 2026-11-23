# Binary field definitions

`exmon_domain::fields::validate` is shared by configuration validation and
`exmon_protocol::fields::Schema`. Compile with `Schema::for_protocol` after
framing/classification selection, then call `decode` on normalized message bytes
on a worker. Offsets refer to those bytes. This module performs no I/O,
transmission, scheduling, capture mutation or GUI work. Whole-message editing,
HEX transactions and checksum dependencies use the [message codec](message-codec.md).

## Wire layout and values

Fields retain schema version 1. Absent extensions preserve old defaults and are
omitted on save. Each wire field has a positive byte length and checked range
inside the framing limit. Unsigned/signed integers have byte widths 1/2/4/8;
float has width 4/8. Bytes, ASCII and UTF-8 have an exact fixed byte length.
Text is validated strictly: no replacement characters, implicit padding or
truncation. ASCII permits NUL. Endian applies to numeric containers, not text.

Optional `bits: {"offset": 4, "width": 12}` selects bits from an integer
container, numbered from its least significant bit after endian decoding.
Widths are 1..64 and must fit the container. Signed bitfields use two's
complement sign extension at the selected width. Overlap is checked in physical
message bits, including containers with different endian settings. Disjoint
bitfields may share storage; overlapping physical bits are rejected.

Typed literals use tagged JSON:

```json
{"kind":"unsigned","value":18446744073709551615}
```

Other tags are `signed` with an i64 value, `float32` with u32 `bits`, `float64`
with u64 `bits`, `bytes` with a byte array `value`, and `text` with string `value`.
IEEE literals preserve exact bits, including negative zero and NaN payloads.
Use an integer-preserving JSON parser/editor for 64-bit values and float bits.

`Decoded.raw` exposes a typed wire value; `raw_number` exposes its unscaled
numeric interpretation when available. `number` independently reports the
engineering projection or an explicit error. Non-numeric text/bytes and
nonfinite IEEE values never become zero. NaN/infinity remain raw-readable and
raw-roundtrippable unless validity rules reject them. Debug formatting of
literals, numbers and decoded results does not include payload values.

## Constants, labels, validity and units

`constant` is an exact raw literal. `enumeration` contains entries such as
`{"value":{"kind":"unsigned","value":1},"name":"Ready"}`. Labels and
values must be unique; labels are 1..128 bytes without controls. Float wire
constants/enumeration use bit equality, including distinct NaN encodings and
signed zero. Numeric derived constants/enumeration use numeric equality.

`constraints` optionally contains typed `min`/`max`, `finite: true` and
`enum_closed: true`. Bounds are inclusive, unscaled numeric values; their types
must fit the field. Closed enumeration requires a nonempty set with at least
one admissible value. Contradictory constants and constraints fail admission.
Wire bytes/text support exact constants and enumeration, not numeric bounds.
Captured invalid values remain inspectable alongside bounded `violations`;
they cannot supply a valid numeric dependency or be written through the codec.

Engineering values use `raw * scale + bias`; defaults are 1 and 0. Unit is
display metadata, at most 32 bytes without controls. Identity projection
preserves exact integers as i128; checked conversion to f64 rejects integers
that would lose precision. Mixed numeric comparisons do not round integers
through f64. Zero scale remains readable as constant bias but cannot be inverted.
Nonfinite projections and arithmetic overflow produce explicit errors.

## Derived fields

A virtual field has `representation: "derived"`, `offset: 0`, `length: 0`,
no bit range and a `derived` RPN operation list. For example, this expression
doubles the engineering value of field `input`:

```json
[
  {"op":"field","id":"input"},
  {"op":"literal","value":{"kind":"unsigned","value":2}},
  {"op":"mul"}
]
```

Operations are `field`, `literal`, `add`, `sub`, `mul`, `div`, `neg`, `abs`,
`min`, `max`. Binary operations pop right then left; division is left/right.
The expression must leave exactly one numeric result. Forward references are
allowed; missing/non-numeric references, malformed stacks and cycles are
rejected. Evaluation follows a validated topological order with a bounded
stack, without recursive expression evaluation or executable code.

Integer operations stay exact in checked i128 where possible. Non-integral
division uses checked f64 conversion. Division by zero, overflow, nonfinite
results and failed dependencies report errors. A derived field's constraints
apply to its computed unscaled result, followed by its own scale/bias.
Derived values have `raw: None` and are read-only.

## Field writes and resource limits

`encode_field` accepts an exact typed literal; `encode_label` resolves a declared
label. Both validate before writing and preserve unrelated bytes and bits.
`encode_number` accepts an engineering `Number` plus an explicit finite,
nonnegative absolute tolerance. It returns the actual projected value; tolerance
zero requires exact equality. Quantization, out-of-range inverses, nonfinite
input and noninvertible scaling fail without changing the message. The caller
owns its editing buffer; these functions never submit it to a transport.

Admission limits are 256 fields, 256 enumeration entries per field, 64 operations
per expression, a positive message limit at most 65536 bytes, and 1 MiB total
variable definition data per schema. Budget checks precede schema cloning.
Graph, stack, occupancy map and reports have explicit bounds. JSON storage has
its separate 1 MiB input/output cap. Callers must bound schema counts, worker
queues and retained reports; the codec retains no message history.

Run `cargo test -p exmon-protocol --test fields --locked` for independent integer,
IEEE and bit-layout vectors, all 132 accepted corpus cases through framing and
classification, checked arithmetic, hostile definitions/truncations, budgets and
transactional-write guards. Configuration admission and deterministic persistence
are covered by `cargo test -p exmon-storage --test configuration --locked`.
