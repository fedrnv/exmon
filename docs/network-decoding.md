# Bounded network decoding

`exmon_protocol::network::Decoder` inspects Ethernet II wire frames without I/O,
transmission, reassembly or copying the captured payload. Run it in a processing
worker, never on the GUI thread. `decode_observation` accepts only Ethernet
observations with DLT_EN10MB (1). Socket observations already contain application
bytes and are explicitly unsupported here; source/destination wire addresses
are not assumed to be local/remote endpoints.

The returned `Packet` borrows the entire original input. `raw()` includes all
captured bytes even for malformed, unknown, oversized and unsupported input.
`payload()` and `payload_range()` identify the deepest safely parsed payload,
excluding Ethernet padding after the IP PDU and bytes after a declared UDP PDU.
Always examine `diagnostic`, `capture_truncated` and checksum states before using
metadata. A missing diagnostic means structural parsing completed, not that
checksums passed. A diagnostic preserves the headers decoded before the failure;
it does not discard an observation or change its timestamps/direction.

Supported metadata:

- Ethernet addresses and inner EtherType; stacked 802.1Q/802.1ad VLAN TPID/TCI,
  VLAN ID, priority and drop eligibility.
- IPv4 endpoints, protocol, DSCP/ECN, TTL, identification, lengths, flags,
  byte-based fragment offset, variable header and checksum.
- IPv6 endpoints, traffic class, flow label, hop limit and payload length;
  bounded hop-by-hop, destination options, routing, fragment and AH spans.
  AH framing/alignment is inspected; cryptographic authentication is not performed.
- TCP ports, sequence/acknowledgement, flags, window, urgent pointer, bounded
  header/options and checksum; UDP ports, length and checksum.

All header lengths, offsets and generic option TLVs are checked before reads.
Standard fixed-size TCP option lengths are checked too. Ethernet FCS is not
identified or verified. IEEE 802.3 LLC frames, other EtherTypes/IP protocols, ESP
and IPv6 jumbograms remain visible as explicitly unsupported/encrypted data.
Non-atomic fragments preserve fragment metadata and bytes without interpreting
them as whole TCP/UDP segments; IPv6 atomic fragments may continue decoding.
There is no IP fragment reassembly, TCP stream reconstruction, IPv6 tunnel
decoding, protocol-option semantic evaluation or complete RFC conformance checker.

IPv4 headers and complete TCP/UDP segments use the Internet checksum, including
the IP pseudoheader and odd-byte padding. IPv6 extensions are excluded from the
upper-layer checksum length. IPv4 UDP zero means `NotPresent`; IPv6 UDP zero is
`Invalid`. Missing covered bytes and source-route/active IPv6 routing/Home Address
endpoint ambiguity yield `Unavailable`, never an inferred `Valid`.
Checksum `Invalid` describes captured bytes: host checksum offload can leave
checksums unfilled, so this alone does not prove wire corruption. IP and transport
checksum results are independent. Original-length capture metadata can flag
truncation of trailing padding even when a complete PDU has a verifiable checksum.

`Decoder::new(Limits)` validates configuration. Defaults and hard ceilings are
65,536 input bytes, eight VLAN tags, sixteen IPv6 extensions and 4,096 interpreted
header bytes (including Ethernet and extensions). Callers may lower them, with a
minimum input limit of one and header limit of fourteen; zero tags/extensions is
valid. A limit stops parsing before the next allocation/read. Only bounded
metadata vectors allocate; input and payload are borrowed. Header walks always
advance, checksum work is linear in the bounded input, and parsing is nonrecursive.
Debug output redacts endpoints and captured contents.

The synthetic wire fixtures in `crates/protocol/tests/network.rs` use documentation
IP addresses and locally administered MACs. They are separate from EXM-005's
application payload corpus. Exact layer/option/VLAN/fragment/checksum assertions,
all fixture prefixes, deterministic mutations/noise and configured/hard limits
exercise this decoder on Windows without device access. This is targeted parser
verification, not the independent EXM-034 fuzz/security gate or a physical/macOS
qualification claim.

Wire format references: [IPv4 (RFC 791)](https://www.rfc-editor.org/rfc/rfc791),
[IPv6 (RFC 8200)](https://www.rfc-editor.org/rfc/rfc8200),
[TCP (RFC 9293)](https://www.rfc-editor.org/rfc/rfc9293),
[UDP (RFC 768)](https://www.rfc-editor.org/rfc/rfc768),
[Internet checksum (RFC 1071)](https://www.rfc-editor.org/rfc/rfc1071),
[AH alignment (RFC 4302)](https://www.rfc-editor.org/rfc/rfc4302),
[Home Address pseudoheader (RFC 6275)](https://www.rfc-editor.org/rfc/rfc6275).
