---
schema: fdtasks-spec/v3
id: EXM-055
title: "Record edit and replay traffic"
type: Task
area: server
component: "traffic-replay"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-031","EXM-050"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-055 — Record edit and replay traffic
## Goal
Turn captured exchanges into editable replay scenarios, preserving timing optionally while allowing packet/field edits before replay.
## Boundaries and compatibility
Original recordings remain immutable; replay targets and timing mode are explicit before activation.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Exact replay preserves order | deterministic focused/integration fixture | PASS |
| AC-02 |  edited replay changes only selected data | negative/integration | PASS |
| AC-03 |  interrupted replay resumes/restarts only by explicit policy. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
