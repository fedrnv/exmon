---
schema: fdtasks-event/v3
event_id: EVT-9aeaac2fe0484d40bec431a85a09c2c3
task_id: EXM-034
event_type: readiness-accepted
created_at: 2026-11-26T00:58:01Z
actor: Codex exmon parser verification
base_revision: 42ad81e3e6ea3f83d8da8fe76efd2c00f6671a01
task_sha256: ec190e4b23617168731f5f488e0815e8b103eb19fad6154e262f5c78b3e8cd4f
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
atomicity:
  outcomes: [Repeatable defensive parser robustness verdict for the prototype]
  packets: [Local deterministic mutation campaign with common replay and resource limits]
  independent_acceptance: false
  boundary_reason: The common runner records corpus identity, progress and resource usage for a single parser robustness verdict
---

Published dependencies EXM-005, EXM-018, EXM-019, EXM-021 and EXM-022 are Done. Use the manual Windows development path under FDTasks 5.7. Reuse the protocol and storage entry points from the application integration test layer. Synthetic input only. No hardware, transmission, new dependency, hosted workflow or release action.

One isolated deterministic campaign on Ethernet/IP/TCP/UDP decoding, classic PCAP and PCAPNG import, every stream framing rule and datagram handling, field definitions and exact codec edits including checksums. Mix valid synthetic kernel inputs with reproducibly changed lengths, boundary bytes and unknown or malformed data. Check input/return bounds and have deterministic smoke scenarios exercise successful paths. Accepted errors remain distinguishable from panic, stalls, memory excess or violated invariants. Preserve seed and case identity on failures and keep independent minimized synthetic regression tests for discovered defects.

Acceptance duration is 15 seconds per target family (four families) with minimum 256 generated checked cases per family. Use fixed seeds and bounded input sizes, one process, a 100-second supervisory wall deadline, 90 CPU seconds and a 256 MiB private-memory ceiling. Capture exact toolchain, source, seeds, counts and outcomes. AC-01 requires deterministic replay and a focused regression for any discovered crash. AC-02 requires full supervised duration without unresolved faults, workspace format/Clippy, architecture, and affected protocol or storage regressions. Allowed paths are application integration test, local supervisor, docs/fuzzing.md, and new EXM-034 records. Narrow bug fixes with their regressions require evidence and accepted scope as applicable. No runtime/config schema changes are planned.
