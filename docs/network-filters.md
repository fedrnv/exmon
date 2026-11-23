# Network capture filters

The shell's network controls apply to the selected mock Ethernet, TCP or UDP
interface. Enter comma-separated numeric ports (either endpoint, OR), optional
numeric source/destination/either IP addresses and select IP protocol/direction.
All different predicates are ANDed. Empty entries mean any; ports include the
full 0..65535 range. At most 32 entries are accepted before canonical sorting and
deduplication. Address predicates must use one IP family. Invalid text is shown
before capture starts and disables Start; direct start calls validate again.

The Ready label describes the draft. Starting/Running labels use the worker's
accepted immutable `FilterPlan`, never a fresh interpretation of draft fields.
Controls stay disabled until capture has stopped. Applying a changed interface
or filter requires an explicit Start and binds a new generation; an old worker
cannot report the new plan Running. Settings are ephemeral and do not persist or
auto-start on reopening. The mock worker uses exact post-capture filtering and
says so. Backend/application losses, filtering exclusions and display decimation
are distinct counters. RX counts describe input before postfiltering.

Mock Ethernet observations contain EXM-005-style application bytes, not Ethernet
wire headers. Network predicates on these bytes therefore match nothing; select
mock TCP/UDP to exercise socket metadata (ports12000/12001), or use wire/replay
sources with the filtered Ethernet adapter. The shell still enables no physical
network capture or transmission. HID and other non-network interfaces do not
inherit a network filter from a prior selection.

The shared pipeline is:

1. `domain::network_filter::NetworkFilter` validates bounded typed settings.
2. `protocol::network_filter::FilterPlan` canonicalizes them, builds a conservative
   BPF prefilter and performs exact post-capture matching via the EXM-009 decoder.
3. `app::network_filters::FilteredEthernet` asks its source to validate the BPF
   without opening a live interface, installs the same expression in capture
   options and always applies the exact postfilter. It exposes the accepted plan,
   actual mode, active state and rejection count. Poll errors/cancellation stop
   capture; invalid caller batch arguments do not falsely change running state.

NativePcap compiles validation on an Ethernet dead handle. It also validates raw
option expressions before live open. Compiler/configuration errors fail closed;
only an explicit `Unsupported` from the source selects the displayed post-only
fallback. Installation/activation errors never produce an active filter. Callers
must display the adapter's actual mode and active state along with the immutable
plan. `Options::filter` must be empty when constructing `FilteredEthernet` so a
second hidden expression cannot diverge from the plan. Raw Ethernet APIs remain
raw and carry no implicit typed filter.

BPF pushes down supported untagged IPv4 protocol, address and TCP/UDP port
predicates, plus untagged IPv6 base-header address predicates. IPv6 transport
and outer 802.1Q/802.1ad frames are admitted broadly because
classic port primitives do not follow IPv6 extensions and the `vlan` primitive
changes subsequent compiler offsets. The bounded decoder resolves VLAN nesting
and extension headers afterward. Thus BPF alone is a superset, while BPF plus
postfilter is equivalent to post-only on identical captured bytes. BPF does not
infer direction or interface identity; those predicates use source context and
observation metadata. The physical interface selected must agree with a plan's
optional interface ID, otherwise open fails.

Direction `Unknown` is a separate exact value, never RX/TX. Wire source/destination
addresses are packet addresses. Socket local/remote endpoints map to source and
destination only for known RX/TX; either endpoint and port matching still work
for Unknown direction. A missing, truncated, malformed or non-atomic fragmented
transport header cannot satisfy a port constraint. IP metadata can still satisfy
an IP predicate when safely decoded. An incomplete IPv6 extension chain leaves
the upper-layer protocol unresolved while preserving safely decoded base addresses.
Unknown protocols/data remain accepted by an unconstrained plan.
Filtering does not require valid captured checksums, since
offload may leave them incomplete; the decoder retains their independent status.
Snapshot length therefore affects available filter metadata; no missing port is
guessed. There is no fragment reassembly or TCP reconstruction here.

Capture and postfiltering run on the owning worker, never the GUI thread. Output
and input budgets are finite: a rejected batch is not refilled in a loop. Plan
and filter Debug output redact endpoints. Human-readable summaries and BPF text
are for local display, not logs or telemetry.

Local acceptance includes exact synthetic IPv4/IPv6 TCP/UDP cases, VLAN/extensions,
unknown direction, fragments, all prefixes, invalid settings, native Npcap dead
compiler/interpreter equivalence, filtered lifecycle failure/cancellation, actual
mock worker and GUI interactions. The existing hidden native eframe smoke checks
the production controls on Windows. Rendered light/dark, 1x/1.5x images and verbose
logs stay under ignored `target/`; immutable task evidence records hashes and
outcomes. Native tests use the explicitly provisioned KM-TEST virtual adapter;
IP forwarding is required disabled on this virtual fixture. The native filtered
adapter test receives exact matching IPv4/IPv6 frames and rejects alternate ports,
including the broad VLAN branch. Physical and macOS qualification remain deferred
to EXM-035/036.

API references: [libpcap filter language](https://npcap.com/guide/wpcap/pcap-filter.html),
[pcap dead capture and compilation](https://docs.rs/pcap/2.5.0/pcap/struct.Capture.html),
[offline BPF interpreter](https://docs.rs/pcap/2.5.0/pcap/struct.BpfProgram.html).
