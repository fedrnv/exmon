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


## FDTasks v4.2 tasks, testing and versions

The normative lifecycle is `Tasks/tasks_file_structure_v4.md`. Executable
Tasks, Bugs and Corrections are medium-sized children of one non-executable Epic
and carry stable acceptance criteria with an AC-to-test readiness matrix.
Microtasks run focused checks; medium-item closure runs the deterministically
selected focused, affected, dependent and invariant set required by this
repository's verification policy.

Each Epic has exactly one dedicated full canonical gate task and one separate
independent Level-0 review task. The review depends on a successful current gate.
Any correction or candidate change reopens both; repeat the complete gate and a
fresh immutable review round before Epic closure. Existing accepted open Epics
gain missing topology only through new items and append-only membership events.

The canonical version is `MAJOR.RELEASE.EPIC.ITEM`. First verified closure of a
medium item creates one immutable request for ITEM; first verified Epic closure
creates one request for EPIC and resets ITEM. Reopening and reclosing the same
identity never allocates again. The closing agent does not assign versions or
edit generated state/history. The path-scoped automatic allocator is the sole
version writer and the only automatic GitHub Actions exception. Builds, tests,
canonical gates, releases and deployments remain manual.
