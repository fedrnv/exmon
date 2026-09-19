---
schema: fdtasks-spec/v3
id: EXM-067
title: "Evaluate bounded derived signal expressions"
type: Task
area: server
component: "derived-expression-engine"
epic_id: EXM-EPIC-004
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-064"]
created_at: 2026-09-19T15:55:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-067 — Evaluate bounded derived signal expressions
## Goal
Let users define derived variables from one or more signals using safe arithmetic, comparison, boolean and bitwise expressions, constants and selected mathematical functions.
## Boundaries and compatibility
Expression engine is sandboxed with no arbitrary filesystem/process/network access and has bounded execution complexity.
## Failure and recovery
Invalid expressions/data, cancellation and restart preserve authoritative capture state and produce explicit bounded diagnostics.
## Ordered implementation
Planner decomposes into bounded microtasks; focused checks follow each change, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high for real-time processing, synchronization and scripting.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Arithmetic/boolean/bitwise vectors are exact | deterministic unit/integration fixture | PASS |
| AC-02 |  invalid/type-unsafe expressions fail before activation | integration/negative | PASS |
| AC-03 |  resource limits terminate pathological expressions. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands, fixtures, numerical/timing tolerances, environment, outcomes, limitations, applicable visual evidence and cleanup.
