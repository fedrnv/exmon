# Streaming PCAP and PCAPNG files

`exmon_storage::pcap` reads and writes capture streams without opening network
interfaces or requiring a capture driver. Run storage operations on a worker.
The caller supplies an explicitly authorized `Read` or `Write` handle; the codec
does not open paths, overwrite destinations, resolve names, execute references,
decompress input or authorize transmission. File selection, privacy notices and
safe destination creation belong to the application integration. For a new file,
use `File::options().write(true).create_new(true)`; overwriting requires a separate
explicit user decision. Check the final `finish`/copy result and label partial
output incomplete after any failure. Flush is checked, but does not imply fsync
or atomic destination replacement.

## Import and inspection

Construct `Reader::new(input, Limits::default())`, then consume `next_entry()` one
entry at a time. Entries own exact raw bytes, file offsets and section numbers.
`content()` describes headers, interfaces, packets and opaque blocks; `payload()`
excludes record headers, packet padding and options. Retaining returned entries
is a caller-owned storage decision that needs its own queue/history limits.

PCAP 2.4 supports both byte orders and both microsecond and nanosecond magic
values. The complete link-information word and file-header reserved fields stay
in the raw header; packet metadata uses its 16-bit LinkType. PCAPNG supports
section byte-order changes, bounded per-section interfaces, section length
validation, interface options, Enhanced Packet Blocks, Simple Packet Blocks and
obsolete Packet Blocks. Read versions 1.0 and compatible 1.2; generated output
uses 1.0. Unknown links, blocks, options and padding remain available as bytes.
They are not decoded as an assumed Ethernet frame or treated as executable data.

Native `Timestamp` values preserve the exact PCAP seconds/fraction or PCAPNG
64-bit ticks, decimal/binary resolution and signed seconds offset. Exact
`nanoseconds()` conversion returns None for negative/overflowing values or
fractions that cannot be represented without rounding. An SPB has no timestamp;
none is invented. A backwards capture timestamp is preserved. Imported capture
time must never drive a monotonic transmission scheduler.

`entry.observation(monotonic_ns)` bridges a valid packet to the existing domain
model: caller-supplied monotonic time, separate exact source timestamp, original
length, raw link type, direction when present and unchanged packet bytes. It
rejects missing/unrepresentable time and invalid packet semantics. Unknown link
types remain typed raw observations for inspection; protocol decoding can report
them unsupported. This is distinct from changing the source file's link type.

Captured length greater than original/snapshot, invalid PCAP timestamp fractions
and oversized PCAPNG packet payloads are inspection issues on retained records;
they cannot become valid domain observations. Truncated headers/data, impossible
lengths, missing interfaces, invalid options/trailers/sections and unsupported
versions return fixed `Code` errors with exact byte offsets and the bytes consumed
for that unit. Parsing stops after a fatal error, without guessed resynchronizing.
The reader's next call reports State. A declared over-limit record is rejected
before allocating its body; its consumed header and the original file remain
the inspection sources. Error Display/Debug and Entry Debug redact raw data and
never include an arbitrary OS error string or path. Explicit `raw()`/`payload()`
access intentionally exposes sensitive capture data to an authorized viewer.

## Complete and selected export

`copy_all(&mut reader, output)` starts from an unread stream and makes a byte-exact
copy, including opaque metadata and retained semantic record issues. A structural
input or output error is terminal and leaves an explicitly incomplete output.

`copy_selected(&mut reader, output, predicate)` calls the predicate for each
packet. It preserves interface IDs/order, sections and opaque metadata while
removing unselected packets. Selection is not anonymization: comments, names,
opaque blocks and custom options may still contain sensitive data. The caller
must explicitly authorize that metadata as well as the selected packet bytes.
The rewrite marks section lengths unspecified, normalizes compatible minor 2
headers to minor 0, removes explicitly non-copyable custom blocks/options, and
upgrades obsolete PB packets to EPB while retaining a known packet-drop count.
Other recognized metadata and unknown options/padding are retained. Opaque block
internals are not interpreted. Complete archival copying performs no rewriting.

`Encoder::new(output, Encoding::PcapNg)` creates a new section.
`write_observation(source_id, &observation)` maps explicit numeric source IDs to
bounded interface IDs, writes their LinkTypes and records nanosecond source time,
original length, direction and exact bytes. A changed LinkType for an existing
source is an error. Supply selected observations or the full stream as required.
Only domain Ethernet/raw-link observations are accepted; active TCP/UDP socket
application bytes cannot be relabelled as Ethernet packets. Missing capture
source time/known original length and link values outside 16 bits are errors.

`Encoding::Pcap { link_type }` writes classic nanosecond PCAP. Its representable
contract is one source/link, Unknown direction and unsigned 32-bit seconds.
Known RX/TX direction, another source/link or an out-of-range time is rejected
instead of silently losing metadata; use PCAPNG for those observations. Imported
classic files already have Unknown direction and can be copied losslessly.
Neither encoding silently substitutes monotonic time for an absent capture clock
or claims an unknown original length was complete. Non-I/O validation errors
write no packet and allow a corrected call. A partial write/I/O failure poisons
the writer; further calls return State. `finish()` checks flush and returns the
owned output handle on success.

## Resource and verification boundary

Default/hard raw block size is 1 MiB; configured limits may lower it to 28 bytes.
PCAP packet bodies and valid domain observations are limited to 65536 bytes.
Malformed PCAPNG packet bytes may still be inspected within the raw block limit.
Interface count defaults to 64 and is capped at 256 per section; option parsing
is capped at 1024 entries per recognized option area. All loops are bounded by
record bytes/counts, with no recursion. The reader defaults to 1 TiB total input
and 100 million entries (hard entry cap 1 billion); callers can select tighter
finite budgets. At a total-byte boundary, at most one extra byte is read to
distinguish EOF from excess input. Eight Interrupted retries are allowed per
bounded I/O operation; persistent interruption becomes an explicit I/O error.
This bounds codec state and per-call allocations, not OS process RSS or caller
retention. Output is streamed with checked byte offsets; destination quota and
cancellation between entries remain the caller's responsibility.

The local suite uses independent synthetic literal/builders, four classic format
variants, both PCAPNG byte orders, multiple sections/interfaces, exact native
clocks, malformed records and every input prefix, deterministic mutations,
short/failed I/O, hard caps and a million-record generated stream without history.
An additional Windows Npcap/libpcap offline oracle reads generated PCAP/PCAPNG and
independently writes PCAP for the production reader. No physical traffic or stand
is involved. This is not a full fuzz/security audit or cross-platform native
qualification; those remain separate tasks, as do GUI and replay integration.

Format references: [PCAP draft 09](https://www.ietf.org/ietf-ftp/internet-drafts/draft-ietf-opsawg-pcap-09.html)
and [PCAPNG draft 06](https://www.ietf.org/archive/id/draft-ietf-opsawg-pcapng-06.html).
Raw archival preservation does not claim semantic decoding of every extension.
