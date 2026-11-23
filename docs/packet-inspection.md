# Packet inspection

Open **Packets** to inspect raw observations independently of the Traffic display
pause. Start capture uses the shell's synthetic adapters. Their arbitrary byte
patterns can correctly produce decoder diagnostics. **Load sample packet** instead
loads a labelled Ethernet/IPv4/UDP fixture without opening or transmitting on an
interface. **Live inspection** returns to captured observations and clears the
inspection context. Neither command starts capture or changes a transmit schedule.

Expand layers in the protocol tree. Selecting a layer or field highlights its
validated captured byte range and jumps the HEX page to its start. Click a byte
to select it; Shift-click extends the selection. HEX pages contain at most 256
bytes and preserve access to the complete admitted packet. A selected packet is
pinned even when hidden by a display filter or evicted from history. A new capture
generation clears live inspection; imported and synthetic views remain explicitly
separate until Live inspection is requested.

The shared application model uses the existing bounded Ethernet/IP/TCP/UDP
decoder for both live observations and imported PCAP/PCAPNG packets. VLANs,
IPv4 options, IPv6 extensions and TCP options retain checked offsets. Unsupported,
encrypted, fragmented, malformed and truncated packets retain all admitted raw
bytes and diagnostics. A socket observation contains application bytes, so socket
metadata has no selectable header range. Capture checksum status can depend on
offload and does not certify wire validity. Source timestamps are explicitly
unknown when absent; import does not invent UTC or monotonic timing.

**Find bytes** searches ASCII by default; enable **HEX pattern** for byte pairs,
with optional whitespace. Patterns contain 1..256 decoded bytes. Search uses
linear-time matching, includes overlaps, and returns at most 256 matches with an
explicit indication of further results. **Find next** cycles these matches.
**Copy HEX** and **Copy ASCII** require an explicit click; ASCII renders
nonprintable bytes as dots. Both operate on the selected raw byte range.

Display filters accept up to eight whitespace-separated AND terms, at most 1024
bytes in total. This is a bounded subset, not the Wireshark filter language:

| Term | Meaning |
| --- | --- |
| `ethernet`, `ipv4`, `ipv6`, `tcp`, `udp`, `raw` | Validated captured layer / raw fallback |
| `diagnostic` | Decoder diagnostic or capture truncation |
| `host=IP`, `src=IP`, `dst=IP` | Captured IP header address |
| `port=NUMBER` | Either captured TCP/UDP port |
| `len>=NUMBER`, `len<=NUMBER` | Captured byte length |
| `contains=HEX` | Raw bytes contain this HEX pattern (no internal whitespace) |

An empty filter shows all retained packets. Apply is explicit; an invalid filter
retains the previous valid filter. Filtering affects only this display and does
not alter capture, Traffic grouping, storage or transmission.

Expand **Import / export files** to enter an explicit local path. Import accepts
regular PCAP/PCAPNG files up to 32 MiB, 10,000 entries, 1 MiB blocks and 64
interfaces. Invalid records or limits reject the entire import and retain the
previous view. A valid simple packet block without timestamp remains inspectable.
Imported mode ignores live packets, counted separately. Reimport establishes a
new context and selection even when the previous view was also imported.
**Export selected bytes** writes the exact nonempty raw selection to a new local
binary file. Existing files are never overwritten. This operation does not
rewrite a PCAP container or claim to preserve its metadata. On a write failure an
incomplete newly created file may remain and is reported explicitly.

Raw packet bytes, addresses and clipboard/file contents may be sensitive. No
automatic copy, export, import, device open or TX is performed. Local-path checks
reject Windows device, UNC, alternate-stream and reserved device paths. File
selection itself remains an explicit user action.

The inspection worker owns decoding, parsing, filtering, search, copy formatting
and file I/O. GUI work is bounded presentation. History retains at most 512 frames
or an 8 MiB conservative charge, plus one UI-pinned frame. Shared snapshots carry
Arc references rather than payload copies; references retained by an external
caller/renderer have their own lifetime. Each frame contains at most 256 nodes
and 65,536 raw bytes. Input queues hold eight batches of at most 96 rows / 1 MiB;
commands and replies each hold eight bounded entries. The UI issues one operation
at a time. Inspection input drops, history evictions, display filtering and
ignored live packets are distinct from capture backend loss and decimation.

Focused checks: `cargo test -p exmon-app --test inspection --locked` and
`cargo test -p exmon-gui --test inspection --locked -- --nocapture`.
The latter exercises actual controls and saves the size/theme/DPI matrix under
`target/exm030-visual`. `cargo run -p exmon-gui --example native_inspection_smoke --locked`
runs a hidden native window, renders a synthetic packet, saves
`target/exm030-native.png`, and exits within eight seconds. None of these checks
qualifies physical hardware or macOS.
