---
schema: fdtasks-spec/v3
id: EXM-069
title: "Synchronize calculations across packet streams"
type: Task
area: server
component: "cross-stream-alignment"
epic_id: EXM-EPIC-004
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-067","EXM-068"]
created_at: 2026-09-19T15:55:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-069 — Synchronize calculations across packet streams
## Goal
Support derived calculations using fields from different packet/message streams with explicit latest-value, nearest-timestamp and interpolation alignment policies.
## Boundaries and compatibility
No hidden interpolation; provenance records exact source samples/policy for each derived value.
## Failure and recovery
Invalid expressions/data, cancellation and restart preserve authoritative capture state and produce explicit bounded diagnostics.
## Ordered implementation
Planner decomposes into bounded microtasks; focused checks follow each change, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high for real-time processing, synchronization and scripting.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Cross-stream fixture yields expected results per policy | deterministic unit/integration fixture | PASS |
| AC-02 |  stale/missing inputs are explicit | integration/negative | PASS |
| AC-03 |  provenance identifies source samples. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands, fixtures, numerical/timing tolerances, environment, outcomes, limitations, applicable visual evidence and cleanup.
