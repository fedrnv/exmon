---
schema: fdtasks-spec/v3
id: EXM-057
title: "Multiple simultaneous virtual devices"
type: Task
area: server
component: "multi-device-simulation"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-053","EXM-054"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-057 — Multiple simultaneous virtual devices
## Goal
Run multiple isolated virtual devices/connections concurrently with separate state, variables, scenarios and traffic histories.
## Boundaries and compatibility
One simulator cannot read or mutate another except through explicit protocol traffic; resource limits are enforced.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Concurrent fixtures preserve isolation | deterministic focused/integration fixture | PASS |
| AC-02 |  independent stop/restart works | negative/integration | PASS |
| AC-03 |  resource-limit breach fails one bounded session safely. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
