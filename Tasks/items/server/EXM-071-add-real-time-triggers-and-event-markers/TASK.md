---
schema: fdtasks-spec/v3
id: EXM-071
title: "Add real-time triggers and event markers"
type: Task
area: server
component: "signal-triggers"
epic_id: EXM-EPIC-004
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-067","EXM-066"]
created_at: 2026-09-19T15:55:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-071 — Add real-time triggers and event markers
## Goal
Define conditions over raw/derived signals that create timestamped event markers and optionally start/stop recording or increment counters.
## Boundaries and compatibility
Trigger side effects are explicit, bounded and separately authorized; no transmission action is implied.
## Failure and recovery
Invalid expressions/data, cancellation and restart preserve authoritative capture state and produce explicit bounded diagnostics.
## Ordered implementation
Planner decomposes into bounded microtasks; focused checks follow each change, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high for real-time processing, synchronization and scripting.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Threshold/boolean fixtures fire at exact transitions | deterministic unit/integration fixture | PASS |
| AC-02 |  debounce/rearm policy is correct | integration/negative | PASS |
| AC-03 |  restart/replay does not duplicate effects. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands, fixtures, numerical/timing tolerances, environment, outcomes, limitations, applicable visual evidence and cleanup.
