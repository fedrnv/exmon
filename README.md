# Exchange Monitor

Exchange Monitor is a cross-platform Rust desktop application for capturing, classifying, decoding, visualizing, creating, and transmitting data across network and hardware communication interfaces.

## Product scope

- Windows, macOS, and Linux.
- Ethernet raw capture and PCAP/PCAPNG workflows.
- TCP and UDP monitoring with port filters and TCP stream reconstruction.
- Serial interfaces including RS-232, RS-422, RS-485, and USB CDC.
- USB HID and USB Bulk/Interrupt.
- CAN capture and transmission where supported.
- Automatic message classification by transport metadata, length, header, and configurable framing.
- Named message types, field definitions, HEX/ASCII inspection, charts, moving averages, manual transmission, and scheduled transmission.
- A single desktop window with interface selection, incoming and outgoing split panels, one tab per message type, and dedicated chart tabs.

Planning follows the append-only FDTasks v3 repository format under `Tasks/`.

## Technology

The accepted starting architecture is Rust with `egui/eframe`. Platform capture backends and permissions are isolated behind transport adapters. See `Tasks/decisions/`.

## Status

The domain contract is documented in [docs/domain-model.md](docs/domain-model.md).
Platform permissions, capture privacy and explicit transmission requirements are
defined in [the product safety policy](docs/security-and-transmission.md).
The six-layer Rust workspace includes a native egui shell with bounded background
mock capture, interface selection, incoming/outgoing panels and safe layout
preferences. Run it with `cargo run -p exmon-gui --locked`; see
[desktop shell details](docs/desktop-shell.md). Optional selected HID report
controls use `--features native-hid`; physical qualification remains pending.

## Local development

Install stable Rust with rustfmt and Clippy, plus Node.js for the architecture
check. Run from the repository root:

```text
cargo build --workspace --locked
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
node scripts/check-architecture.mjs
```

Dependencies point inward: GUI -> application -> capture/protocol/storage ->
domain. Capture, protocol and storage cannot depend on each other or on the GUI.
All crates inherit workspace lints. Cargo.lock is committed for reproducibility;
target directories, local IDE files and sensitive capture files are ignored.
Synthetic fixtures requiring capture file extensions must be explicitly reviewed
before adding them. The GUI crate contains the native egui/eframe entry point
and main shell. Message editors, classification, charts and physical adapter
integration remain separate accepted tasks.
