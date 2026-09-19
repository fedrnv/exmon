---
schema: fdtasks-spec/v3
id: EXM-072
title: "Build packet and field statistics metrics dashboard"
type: Task
area: web
component: "statistics-dashboard"
epic_id: EXM-EPIC-004
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-064","EXM-066","EXM-068"]
created_at: 2026-09-19T15:55:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-072 — Build packet and field statistics metrics dashboard
## Goal
Show real-time statistics for packet types and fields: counts, packets/s, bytes/s, interval min/max/mean, jitter/errors plus signal current/min/max/mean/RMS/stddev/update/change rates.
## Boundaries and compatibility
Metrics expose window/session/reset definitions and distinguish unavailable from zero.
## Failure and recovery
Invalid expressions/data, cancellation and restart preserve authoritative capture state and produce explicit bounded diagnostics.
## Ordered implementation
Planner decomposes into bounded microtasks; focused checks follow each change, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high for real-time processing, synchronization and scripting.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Known traffic fixture yields exact packet metrics | deterministic unit/integration fixture | PASS |
| AC-02 |  signal statistics meet tolerance | integration/negative | PASS |
| AC-03 |  reset/window controls are consistent. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands, fixtures, numerical/timing tolerances, environment, outcomes, limitations, applicable visual evidence and cleanup.
