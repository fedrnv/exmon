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

Each development Epic has exactly one dedicated full canonical gate task and one separate
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

## FDTasks v4.2 maintenance exception: EPIC-999

Owner-authorized policy from `fedrnv/FDTasks@3ff6a1162f76cbcbc8a7a90ca1527f1b4753cda4`:

- Bugs belonging to development in an open Epic are added to that Epic.
- Bugs related to a closed Epic, bugs with unclear Epic association, and isolated
  minor improvements go to `EXM-EPIC-999`. Link the original Epic when known;
  do not reopen the closed Epic because of a later bug.
- EPIC-999 stays open permanently and is excluded from the one-active-development-
  Epic limit. It has no mandatory overall gate/review and no Epic closure request.
- Ordinary maintenance tasks require testing changed functionality and directly
  affected behaviour before task closure, with exact evidence. Full project tests
  and independent review require an explicit verification task in EPIC-999; one
  task may request both. Its scope/dependencies cover the selected work only.
- Unrelated new maintenance items do not reopen earlier verification tasks.
  First verified item closure follows normal ITEM allocation; EPIC never increments
  for EPIC-999. Existing development Epics retain full gates and independent review.

This explicit exception supersedes blanket per-item full-test/review requirements
and test-deferral rules only for EPIC-999. Preserve project-specific test commands,
domain/security checks for affected behaviour, resource supervision, immutable
history and build/release authorization. Runtime descriptions are not evidence of
support: report any incompatible tooling; never invent successful verification.
