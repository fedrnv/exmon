# Development

## Baseline

- Stable Rust toolchain and Cargo workspace.
- Desktop GUI: egui/eframe.
- Async/background work: Tokio where asynchronous I/O is appropriate.
- Capture: libpcap on Linux/macOS and Npcap on Windows.
- Serialization: Serde with versioned schemas.
- Storage: bounded in-memory buffers first; SQLite-backed sessions when implemented.

## Layering

1. `domain`: transport-neutral frames, messages, fields, samples, filters, and schedules.
2. `capture`: backend traits and platform adapters.
3. `protocol`: framing, classification, reassembly, decoding, encoding, and checksums.
4. `storage`: session, configuration, PCAP/PCAPNG, CSV, and JSON persistence.
5. `app`: commands, bounded event queues, lifecycle, and error reporting.
6. `gui`: rendering and user interaction only.

## Quality gates

For each completed task:

- format and lint the complete workspace;
- run unit and relevant integration tests;
- run regression tests for already implemented transports;
- document exact verification in an immutable task event;
- keep generated build outputs and sensitive captures out of Git.

Release candidates require clean builds and smoke tests on Windows, macOS Intel/Apple Silicon as applicable, and Linux. Tests requiring physical RS-485, USB, or CAN hardware must be recorded as blocked until the declared test devices are available.
