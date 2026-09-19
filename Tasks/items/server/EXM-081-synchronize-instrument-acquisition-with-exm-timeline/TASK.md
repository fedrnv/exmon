---
schema: fdtasks-spec/v3
id: EXM-081
title: "Synchronize instrument acquisition with EXM timeline"
type: Task
area: server
component: "instrument-timeline"
epic_id: EXM-EPIC-005
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-076","EXM-065","EXM-069"]
created_at: 2026-09-19T16:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-081 — Synchronize instrument acquisition with EXM timeline
## Goal
Map instrument samples, trigger events and capture blocks onto the EXM authoritative timeline so analog channels can be inspected with packets, digital channels and derived signals.
## Boundaries and compatibility
Clock domains and uncertainty are explicit; host receipt time is never silently presented as hardware sample time.
## Failure and recovery
Device/library/network failure cannot corrupt capture state or block unrelated EXM transports; ownership and reconnect are bounded.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused, affected, dependent and invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where native vendor libraries, USB and hardware timing are involved.
| AC ID | Observable criterion | Test level/scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | Deterministic clock fixture maps samples correctly | adapter contract/integration | PASS |
| AC-02 |  uncertainty/drift are exposed | negative/integration | PASS |
| AC-03 |  synchronized cursor selects corresponding packet and waveform state. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revision, SDK/backend versions, device identity where available, commands/manual scenarios, timing limits, outcomes, limitations and cleanup.
