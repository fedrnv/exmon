---
schema: fdtasks-spec/v3
id: EXM-050
title: "Manual periodic and scheduled transmission"
type: Task
area: server
component: "traffic-scheduler"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-023","EXM-024"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-050 — Manual periodic and scheduled transmission
## Goal
Send configured packets on button press, once, periodically, N times or on an explicit schedule with start/stop/pause controls.
## Boundaries and compatibility
No transmission occurs without an enabled operator-owned session; schedules are bounded and cancellable.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Manual send emits exactly once | deterministic focused/integration fixture | PASS |
| AC-02 |  periodic/N-repeat timing is bounded and counted | negative/integration | PASS |
| AC-03 |  stop/restart cannot duplicate sends. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
