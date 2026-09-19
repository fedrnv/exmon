---
schema: fdtasks-spec/v3
id: EXM-078
title: "Integrate PicoScope instruments"
type: Task
area: server
component: "picoscope-adapter"
epic_id: EXM-EPIC-005
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-076"]
created_at: 2026-09-19T16:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-078 — Integrate PicoScope instruments
## Goal
Support PicoScope families through PicoSDK with model/capability discovery, analog acquisition, streaming/block capture and triggers where supported.
## Boundaries and compatibility
Series-specific differences stay behind capability mapping; never claim unsupported bandwidth/rate/modes.
## Failure and recovery
Device/library/network failure cannot corrupt capture state or block unrelated EXM transports; ownership and reconnect are bounded.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused, affected, dependent and invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where native vendor libraries, USB and hardware timing are involved.
| AC ID | Observable criterion | Test level/scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | SDK fixture/hardware contract validates capture | adapter contract/integration | PASS |
| AC-02 |  range/rate/trigger mapping is exact | negative/integration | PASS |
| AC-03 |  device loss recovers safely. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revision, SDK/backend versions, device identity where available, commands/manual scenarios, timing limits, outcomes, limitations and cleanup.
