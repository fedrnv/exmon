---
schema: fdtasks-spec/v3
id: EXM-051
title: "Dynamic packet fields counters and checksums"
type: Task
area: server
component: "dynamic-packet-fields"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-022","EXM-050"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-051 — Dynamic packet fields counters and checksums
## Goal
Compute packet fields per transmission including counters, sequence IDs, timestamps, lengths, CRC/checksums, random/ranged values and field dependencies.
## Boundaries and compatibility
Expressions are deterministic when seeded and cannot access arbitrary host resources.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Known vectors compute exact values | deterministic focused/integration fixture | PASS |
| AC-02 |  counter/wrap behavior is correct | negative/integration | PASS |
| AC-03 |  invalid dependency graphs fail before send. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
