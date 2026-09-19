---
schema: fdtasks-spec/v3
id: EXM-053
title: "Stateful virtual device simulator"
type: Task
area: server
component: "device-state-machine"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-052"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-053 — Stateful virtual device simulator
## Goal
Model virtual-device states, transitions, state variables and state-dependent responses so Exchange Monitor can replace a protocol peer.
## Boundaries and compatibility
State transitions are explicit and persisted; invalid transitions cannot silently mutate simulator state.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Deterministic fixture follows expected states | deterministic focused/integration fixture | PASS |
| AC-02 |  state-dependent responses differ correctly | negative/integration | PASS |
| AC-03 |  restart restores configured state policy. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
