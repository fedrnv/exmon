---
schema: fdtasks-spec/v3
id: EXM-064
title: "Unify packet fields and derived signals"
type: Task
area: server
component: "signal-model"
epic_id: EXM-EPIC-004
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-021","EXM-022"]
created_at: 2026-09-19T15:55:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-064 — Unify packet fields and derived signals
## Goal
Introduce one timestamped Signal/Variable model for decoded packet fields and future derived values, including type, unit, source identity, validity and update semantics.
## Boundaries and compatibility
Raw packet evidence remains authoritative; signal conversion never mutates captures.
## Failure and recovery
Invalid expressions/data, cancellation and restart preserve authoritative capture state and produce explicit bounded diagnostics.
## Ordered implementation
Planner decomposes into bounded microtasks; focused checks follow each change, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high for real-time processing, synchronization and scripting.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Field updates produce typed timestamped signals | deterministic unit/integration fixture | PASS |
| AC-02 |  missing/invalid fields propagate explicit validity | integration/negative | PASS |
| AC-03 |  source traceability is exact. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands, fixtures, numerical/timing tolerances, environment, outcomes, limitations, applicable visual evidence and cleanup.
