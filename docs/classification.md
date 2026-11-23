# Explainable message classification

`exmon_protocol::classification::Classifier` is an immutable worker-side rule
snapshot. Construct it from a validated ProjectConfig and an explicit caller
revision. It retains bounded compiled rule/name data, not messages, observations,
devices, history or automatically learned groups. Classification never captures,
transmits, opens paths or enables schedules. Callers bound queues, retained results
and concurrent classifier revisions; keep processing off the GUI thread.

## Rules and explanations

Each Protocol supplies stable `id`, user-visible `name`, and Classification:

- Existing optional positive exact `length` and byte `prefix` retain their meaning.
- `offset` defaults to zero. The complete header range must fit the framing limit
  and any specified exact length. An empty prefix has no header predicate and
  cannot have a nonzero offset.
- `mask` defaults empty, meaning FF for every prefix octet. Otherwise its length
  must equal the prefix length. Compare `(actual & mask) == (expected & mask)`;
  ignored bits cannot split a configured type. Zero mask bits still require the
  corresponding input bytes to exist.
- Optional `direction` and `transport` constraints are ANDed with byte/length
  conditions. Higher signed `priority` wins; the default is zero. Comparison does
  not negate priorities, so both i32 extremes are valid.

Typed transport predicates distinguish Ethernet link type; TCP/UDP local/remote
addresses and ports; serial configuration revision; USB endpoint; HID vendor,
product, usage page, usage and report ID; USB trace event/transfer/endpoint/bus/
device; and CAN channel/ID/extended/FD/remote/error/DLC. Omitted fields are
wildcards. Address and port constraints must agree; contradictory CAN selectors
and invalid USB endpoints are rejected. A transport predicate never matches a
different metadata variant merely because its numeric fields happen to agree.

Rules run in descending priority, then stable lexicographic ID order. Every rule
returns a RuleCheck: first failed predicate, or a match marked selected/shadowed.
The fixed predicate evaluation order is incomplete input, transport, direction,
length, header range, header contents. `criteria` bits identify configured
transport/direction/length/header/mask checks as bits 0..4. `explanation()` gives
a fixed readable reason; `condition(id)` exposes the exact immutable rule of
this revision for explicit inspection of expected ranges, masks and values.
Default Debug output does not print those values, names, IDs or endpoints.

One highest-priority match produces Matched and its configured rule ID/name.
Several highest-priority matches produce Ambiguous with all top candidate IDs,
sorted deterministically; no first-array-element tie-break hides ambiguity.
Lower-priority matches remain explained as shadowed. No match produces Unmatched
with individual failures. Classification returns a result for empty messages too;
the existing schema's explicit length predicate remains positive. Neither
Ambiguous nor Unmatched input is dropped or relabelled as a verified type.

## Input quality and source retention

`classify_message(message, metadata, direction, quality)` consumes an EXM-019
normalized message view. The caller supplies the retained stream's coherent
transport context and Complete, Unknown or Incomplete quality. Rule offsets refer
to normalized application bytes, while the original Message retains raw source
ranges. Keep its framing/loss diagnostics; classification cannot reconstruct
missing upstream provenance or infer a complete protocol from a header match.

`classify_observation` is for native record boundaries. TCP/serial read chunks
return NeedsFraming; they must pass the framer first. Known truncation and failed
USB completion yield Incomplete, with every rule blocked by IncompleteInput.
An unknown original length yields Unknown quality, even if the captured bytes
match a rule. A match is not proof of complete wire capture, checksum correctness
or device support. Oversized input/invalid typed metadata return fixed errors;
the caller still owns the unchanged source for an error/inspection row. The
low-level `classify` method uses explicitly supplied normalized bytes and quality.

Decisions retain revision, outcome, quality, typed group identity, display name,
candidate IDs and bounded explanations. They retain no full payload or source
addresses; automatic keys/labels can contain up to 16 header octets.
Source observations/messages are unchanged and must stay available in the caller's
bounded storage. Retain rule/configuration revisions when historical reinspection
is required; constructing another revision does not mutate existing decisions.

## Automatic groups and names

Unmatched, Ambiguous and Incomplete results have distinct automatic-group states.
Their IDs use a complete canonical binary representation encoded as lower-case
hex after `auto-v1-`, without a truncated hash or order-dependent allocation.
Encoding is state, direction, transport tag/signature, captured length (big-endian
u32), actual retained header length, and header octets. Configurable header_bytes
is 0..16, default 4. Equal features give the same ID on reload/reordering;
changing ignored payload bytes cannot change the group. This is a declared shape
grouping policy, not inference that an unknown protocol has a particular layout.
Use explicit masked rules when value bytes should not define the type.

Stable signatures are Ethernet LinkType; TCP/UDP ports; serial kind; USB endpoint;
HID vendor/product/usage page/usage/report; USB trace event/transfer/endpoint; and
CAN channel/ID/flags/DLC. Numeric fields use fixed-width big-endian encodings.
Connection IDs, host addresses, serial configuration revisions and USB bus/device/
trace tags are excluded from automatic identity. Explicit typed rules can select
those supported fields when needed. Direction and quality outcome remain visible.
Custom automatic names attach to the shape identity across rule revisions, not
to the particular list of current ambiguous candidates.

`ProjectConfig::rename_type` edits a configured Protocol.name.
`rename_auto_group` edits or adds one explicitly named canonical automatic ID.
Both validate before mutation and reject unknown rule IDs, malformed automatic
keys, duplicate stored IDs, empty/control-containing or over-128-byte names, and
the 257th automatic name. Existing entries can be renamed at capacity. New
classifiers pick up edits; old classifier/decision snapshots keep their names.
The ordinary bounded `save_project`/`load_project` JSON APIs persist them without
touching machine bindings or TX permissions. No filesystem write happens in a
rename method. The application must provide the user action and destination.

Defaults and newly absent fields are skipped so existing schema-one data remains
readable and retains old serialized defaults. Older applications reject new
fields/variants explicitly. Automatic IDs contain header fragments and names may
contain sensitive user text; they are deliberately redacted in Debug and require
the same care as protocol definitions when exporting configuration. Selection
of a name is not permission to upload data. GUI naming/tabs remain later work.

## Bounds and verification

At most 256 validated rules, 1 MiB aggregate prefix+mask data, 256 named automatic
groups and 65536 input bytes are accepted. Validate borrowed input/budgets before
copying compiled rules. Matching work is bounded by total rule bytes and count;
there is no recursion, pattern execution, regex engine or per-message cache.
Each decision has at most 256 checks/contenders and bounded ID/name strings.
Canonical keys are at most 76 bytes of text. Caller-retained decisions/instances
need independent limits; these are not an OS process RSS guarantee. The storage
layer's existing 1 MiB JSON document limit remains separate and may be reached
before the typed pattern budget when byte arrays are serialized.

Tests include independent mask/reference matching, signed priority extremes,
ties/order/reload/name stability, all transport variants, full canonical keys and
volatile-field exclusions, native/stream/quality distinctions, bad inputs and
hard caps, redacted Debug, high-cardinality input without classifier history,
and all 132 accepted traffic cases through the production framer/classifier.
Direct storage tests cover old defaults, typed selectors, explicit rename
roundtrips and invalid configurations with default-disabled TX/schedules.
Windows VPS pure processing qualification does not replace physical/platform,
fuzz/security, GUI, release or independent Epic gate/review acceptance.
