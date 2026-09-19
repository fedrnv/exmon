---
schema: fdtasks-spec/v3
id: EXM-079
title: "Integrate Red Pitaya instruments"
type: Task
area: server
component: "red-pitaya-adapter"
epic_id: EXM-EPIC-005
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-076"]
created_at: 2026-09-19T16:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-079 — Integrate Red Pitaya instruments
## Goal
Support Red Pitaya using its documented network/API interfaces for acquisition, trigger and available generation without custom FPGA or device firmware in this Epic.
## Boundaries and compatibility
Treat Red Pitaya as an external stock instrument; custom firmware/FPGA is explicitly excluded.
## Failure and recovery
Device/library/network failure cannot corrupt capture state or block unrelated EXM transports; ownership and reconnect are bounded.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused, affected, dependent and invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where native vendor libraries, USB and hardware timing are involved.
| AC ID | Observable criterion | Test level/scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | Network fixture/hardware test validates acquisition | adapter contract/integration | PASS |
| AC-02 |  trigger/configuration map correctly | negative/integration | PASS |
| AC-03 |  reconnect and timeout preserve EXM state. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revision, SDK/backend versions, device identity where available, commands/manual scenarios, timing limits, outcomes, limitations and cleanup.
