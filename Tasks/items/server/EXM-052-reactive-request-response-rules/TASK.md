---
schema: fdtasks-spec/v3
id: EXM-052
title: "Reactive request response rules"
type: Task
area: server
component: "reactive-responder"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-050","EXM-051"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-052 — Reactive request response rules
## Goal
Define rules such as receive message X then send Y after a configured delay, with field extraction/matching and response templating.
## Boundaries and compatibility
Rules are explicit, bounded and observable; unmatched traffic never triggers hidden sends.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Matching request triggers one expected response | deterministic focused/integration fixture | PASS |
| AC-02 |  delay tolerance is verified | negative/integration | PASS |
| AC-03 |  ambiguous/unmatched rules fail safely. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
