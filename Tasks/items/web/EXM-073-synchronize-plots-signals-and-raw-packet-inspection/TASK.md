---
schema: fdtasks-spec/v3
id: EXM-073
title: "Synchronize plots signals and raw packet inspection"
type: Task
area: web
component: "synchronized-inspection"
epic_id: EXM-EPIC-004
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-065","EXM-070","EXM-030"]
created_at: 2026-09-19T15:55:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-073 — Synchronize plots signals and raw packet inspection
## Goal
Link timeline cursor/selection to the corresponding raw packet, decoded fields, digital states and derived values so the operator can inspect one moment across all views.
## Boundaries and compatibility
Selection uses authoritative timestamps and preserves ambiguity when multiple observations share a time.
## Failure and recovery
Invalid expressions/data, cancellation and restart preserve authoritative capture state and produce explicit bounded diagnostics.
## Ordered implementation
Planner decomposes into bounded microtasks; focused checks follow each change, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high for real-time processing, synchronization and scripting.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Cursor selects expected packet/value set | deterministic unit/integration fixture | PASS |
| AC-02 |  nearest/exact policy is visible | integration/negative | PASS |
| AC-03 |  ambiguous timestamps remain inspectable. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands, fixtures, numerical/timing tolerances, environment, outcomes, limitations, applicable visual evidence and cleanup.
