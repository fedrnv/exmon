# Run the virtual prototype

Install Rust 1.98.0 with rustfmt and Clippy and the Windows MSVC build tools.
Use Node.js for the architecture check. Clone this repository with its committed
Cargo.lock and run from the root.

```text
cargo +1.98.0 run -p exmon-gui --locked
```

The default desktop uses synthetic interfaces and works without the exchange
stand. Choose a transport and Mock 01 or Mock 02, then Start capture. Incoming
shows raw observations and classified message tabs. Select Sample telemetry to
see decoded fields, select a message and inspect its HEX/ASCII bytes. Rename the
type to verify that future messages use the chosen name.

Copy format to outgoing creates a draft. Use Field editor or HEX editor, Apply,
and inspect the resulting bytes. Enable Allow mock transmission, Connect mock
TX and Send once. Start schedule explicitly enables periodic sends. Stop
schedule revokes them. The mock sink reports actual submitted bytes and counts.

Open Charts, expand Select numeric series and select sequence or interface.
Set the sample window and average length and Apply chart window. Values and
moving averages update while capture continues. Settings exposes framing and
field definitions. Validate and preview before Apply. Applying settings revokes
active capture and transmission. Start capture again explicitly.

Packets provides a synthetic Ethernet/IPv4/UDP fixture, protocol tree,
HEX/ASCII selection, byte search, display filters and local PCAP/PCAPNG import.
Import reads recorded packets into the bounded inspector. Live inspection
returns to live observations. Imported bytes have no transmission authority.
The default live generator uses application payload patterns.

Sessions records raw observations and processed samples to a new local file.
Start recording, run capture and add an annotation, then Stop recording.
Recover session browses committed records and preserves interrupted or lossy
status. Export session data provides selected local CSV/JSON output. Reopening
the application leaves capture and transmission idle.

The status bar reports backend loss, application drops, display decimation,
history eviction and recording loss. HID reports gives an explicit unsupported
state in the default build. Optional native features need their own device,
driver and permission qualification.

The automated demonstration runs the same controls across one shell session.

```text
cargo +1.98.0 test -p exmon-gui --test prototype --locked -- --test-threads=1
cargo +1.98.0 test -p exmon-capture --locked
cargo +1.98.0 run -p exmon-gui --example native_sessions_smoke --locked
```

Transport tests include TCP/UDP local socket loopback, virtual serial and CAN,
simulated USB transfers and HID reports. Native platform and physical electrical
qualification follow EXM-035/036. The prototype acceptance also exercises the
existing GUI suites for naming, framing previews, field edits, axes, schedules,
inspection, persistence and unavailable backends.
