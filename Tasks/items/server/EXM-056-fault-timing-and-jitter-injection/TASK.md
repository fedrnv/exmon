---
schema: fdtasks-spec/v3
id: EXM-056
title: "Fault timing and jitter injection"
type: Task
area: server
component: "fault-injection"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-054","EXM-055"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-056 — Fault timing and jitter injection
## Goal
Inject controlled CRC/length corruption, dropped/duplicated/reordered packets, delays, jitter, timeout, disconnect, burst and bandwidth constraints.
## Boundaries and compatibility
Fault profiles are opt-in, visible, bounded and never alter source captures/protocol definitions.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Each deterministic fault fixture produces only requested fault | deterministic focused/integration fixture | PASS |
| AC-02 |  seeded jitter is reproducible | negative/integration | PASS |
| AC-03 |  disabling profile restores clean exchange. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
