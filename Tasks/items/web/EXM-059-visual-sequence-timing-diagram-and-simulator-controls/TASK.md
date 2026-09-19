---
schema: fdtasks-spec/v3
id: EXM-059
title: "Visual sequence timing diagram and simulator controls"
type: Task
area: web
component: "simulator-ui"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-054","EXM-058"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-059 — Visual sequence timing diagram and simulator controls
## Goal
Provide a timeline/sequence diagram for Host/Device/virtual peers with packet direction, timestamps, latency, state transitions and controls for simulator/scenario execution.
## Boundaries and compatibility
Visualization is derived from authoritative events; UI controls obey the same transmission authorization as backend operations.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Diagram ordering/timing matches fixture | deterministic focused/integration fixture | PASS |
| AC-02 |  state transitions are visible | negative/integration | PASS |
| AC-03 |  start/pause/stop/manual actions are reflected exactly. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
