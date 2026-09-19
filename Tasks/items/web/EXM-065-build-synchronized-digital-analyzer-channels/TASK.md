---
schema: fdtasks-spec/v3
id: EXM-065
title: "Build synchronized digital analyzer channels"
type: Task
area: web
component: "digital-analyzer"
epic_id: EXM-EPIC-004
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-064","EXM-028"]
created_at: 2026-09-19T15:55:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-065 — Build synchronized digital analyzer channels
## Goal
Display selected boolean/bit/threshold signals as stacked digital channels synchronized with ordinary plots on one time axis, using gray for 0 and red for 1.
## Boundaries and compatibility
Zoom, pan, cursor and selection share authoritative timestamps across analog/digital views.
## Failure and recovery
Invalid expressions/data, cancellation and restart preserve authoritative capture state and produce explicit bounded diagnostics.
## Ordered implementation
Planner decomposes into bounded microtasks; focused checks follow each change, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high for real-time processing, synchronization and scripting.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | 0/1 rendering is exact | deterministic unit/integration fixture | PASS |
| AC-02 |  multiple channels align with plot time | integration/negative | PASS |
| AC-03 |  cursor/zoom selection remains synchronized. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands, fixtures, numerical/timing tolerances, environment, outcomes, limitations, applicable visual evidence and cleanup.
