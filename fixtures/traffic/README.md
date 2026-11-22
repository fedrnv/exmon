# Synthetic traffic corpus

`corpus.json` is generated exclusively from constants in
`scripts/generate-traffic-fixtures.mjs`. No observation was captured from a
person, device or production network. This original synthetic fixture corpus is
dedicated to the public domain under CC0-1.0; see `LICENSE`. The generator is the
provenance source and uses no randomness, current timestamps or external inputs.

Run `node scripts/generate-traffic-fixtures.mjs --check` to compare committed
bytes with regeneration. Intentional changes use `--write` and require review
of both generator and changed expectations. The JSON is source fixture data,
not a build output. No privileges, drivers or devices are needed.

The 132 cases cover Ethernet, TCP, UDP, serial, USB and CAN; datagram, fixed,
delimiter and length-prefix framing; normal, fragmented, malformed, truncated,
eight-observation burst and timestamped scenarios. TCP/serial exclude datagram
mode. Every remaining transport/mode pair has all six scenarios. Bytes represent
post-adapter application payloads or ordered chunks, not complete link headers,
TCP handshake/reassembly or PCAP files. Do not treat them as evidence that a
physical adapter, electrical configuration or real protocol has been verified.
USB CDC can use the serial corpus; HID/interrupt payloads use USB metadata.

Each case declares transport metadata, framing/scenario, relative monotonic
nanosecond timestamps, per-case sequence, direction, captured byte arrays,
original length, expected classified messages/fields and terminal outcome.
Identities are synthetic case names. No device selectors or network addresses
are needed. CAN payload sizes fit classic CAN; id 0x123 is synthetic. Prefix and
delimiter framing bytes are application data independent of the transport.

The synthetic message is `A5 00 2A`, type `synthetic_value`, with the unsigned
big-endian field at offset 1 of length 2 equal to 42. Fixed framing uses three
bytes. Delimiter framing appends LF and strips it from the message. Length-prefix
framing prepends a one-byte payload length (excluding the prefix). Datagram mode
uses one complete observation per candidate. The classification rule also
requires exactly three message bytes and prefix A5.

Fragmented chunks join only for stream framing. Two short datagrams stay
incomplete and must not join. Malformed cases retain framing but change A5 to A4,
so classification returns no message. Truncated observations report a larger
original length; reject them and reset partial framing state rather than
inventing missing bytes. Bursts retain eight individual observations and expected
messages with deterministic timestamps. Timestamped cases use 123456789 ns;
all other times are sequence * 1000 ns, relative to a synthetic epoch, not UTC.

`cargo test -p exmon-storage --test traffic_fixtures --locked` independently
checks corpus coverage, ordering, truncation and framing/classification/field
expectations using a small test-only reference reader. This checks fixture
integrity; later production framing/adapter tasks must run the same corpus
through their actual implementation before claiming product verification.
