---
schema: fdtasks-spec/v3
id: EXM-042
title: "Export reconstructed protocol to file"
type: Task
area: sync
component: "protocol-export"
epic_id: EXM-EPIC-001
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-041"]
created_at: 2026-09-19T13:50:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-042 — Export reconstructed protocol to file
## Goal
Export the reconstructed/operator-corrected protocol as a deterministic versioned portable file and support safe re-import with semantic round-trip equivalence.
## Scope and boundaries
No credentials or unrelated raw captures are embedded by default; unsupported/corrupt versions fail closed.
## Failure and recovery
Failures remain explicit and recoverable without corrupting captured/protocol state or causing unintended transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence/events per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high according to external-data, inference and device-transmission impact.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Identical state exports deterministically | focused/integration | PASS with deterministic evidence |
| AC-02 |  export/import preserves semantics | integration/negative | PASS |
| AC-03 |  invalid versions leave active protocol unchanged. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, limitations, applicable UI visual evidence and cleanup.