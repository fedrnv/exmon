---
schema: fdtasks-spec/v3
id: EXM-066
title: "Compute signal change and edge counters"
type: Task
area: server
component: "signal-event-metrics"
epic_id: EXM-EPIC-004
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-064"]
created_at: 2026-09-19T15:55:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-066 — Compute signal change and edge counters
## Goal
Track changes, rising/falling edges, 0→1 and 1→0 counts, time since last change and transition/update rates for selected fields/signals.
## Boundaries and compatibility
Counters use explicit reset/session boundaries and cannot double-count replayed observations.
## Failure and recovery
Invalid expressions/data, cancellation and restart preserve authoritative capture state and produce explicit bounded diagnostics.
## Ordered implementation
Planner decomposes into bounded microtasks; focused checks follow each change, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high for real-time processing, synchronization and scripting.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Known transition fixture yields exact counters | deterministic unit/integration fixture | PASS |
| AC-02 |  reset boundary is deterministic | integration/negative | PASS |
| AC-03 |  replay/recovery produces no duplicates. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands, fixtures, numerical/timing tolerances, environment, outcomes, limitations, applicable visual evidence and cleanup.
