---
schema: fdtasks-spec/v3
id: EXM-061
title: "AI assisted simulator and scenario generation"
type: Task
area: ai
component: "ai-simulator-builder"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-045","EXM-053","EXM-054","EXM-058"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-061 — AI assisted simulator and scenario generation
## Goal
Let the Epic 002 assistant propose virtual-device state machines, responses, scenarios and assertions from selected recorded exchanges and reconstructed protocols.
## Boundaries and compatibility
AI output is a reviewable draft; it cannot activate transmission, fault injection or a simulator without explicit operator action.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Known exchange yields valid reviewable draft | deterministic focused/integration fixture | PASS |
| AC-02 |  provenance links inputs | negative/integration | PASS |
| AC-03 |  malformed AI output is rejected | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
