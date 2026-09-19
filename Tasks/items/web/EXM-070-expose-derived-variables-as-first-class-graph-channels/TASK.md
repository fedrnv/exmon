---
schema: fdtasks-spec/v3
id: EXM-070
title: "Expose derived variables as first-class graph channels"
type: Task
area: web
component: "derived-signal-ui"
epic_id: EXM-EPIC-004
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-067","EXM-068","EXM-069","EXM-028"]
created_at: 2026-09-19T15:55:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-070 — Expose derived variables as first-class graph channels
## Goal
Make user-created derived variables appear in the same signal/channel selector as packet fields and allow plotting them as analog graphs or compatible digital channels.
## Boundaries and compatibility
Deleting/editing a formula cannot silently corrupt saved layouts; dependency changes are explicit.
## Failure and recovery
Invalid expressions/data, cancellation and restart preserve authoritative capture state and produce explicit bounded diagnostics.
## Ordered implementation
Planner decomposes into bounded microtasks; focused checks follow each change, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high for real-time processing, synchronization and scripting.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Derived variable appears immediately | deterministic unit/integration fixture | PASS |
| AC-02 |  graph matches computed values | integration/negative | PASS |
| AC-03 |  dependency/edit/delete behavior is deterministic. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands, fixtures, numerical/timing tolerances, environment, outcomes, limitations, applicable visual evidence and cleanup.
