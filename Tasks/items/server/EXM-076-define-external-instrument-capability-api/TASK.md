---
schema: fdtasks-spec/v3
id: EXM-076
title: "Define external instrument capability API"
type: Task
area: server
component: "instrument-api"
epic_id: EXM-EPIC-005
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-064"]
created_at: 2026-09-19T16:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-076 — Define external instrument capability API
## Goal
Define discovery, connect/disconnect, channel metadata, ranges/coupling, sample rate, acquisition/streaming, trigger, timestamps and optional generator/digital capabilities behind one versioned adapter contract.
## Boundaries and compatibility
Adapters expose capabilities explicitly; unsupported operations fail predictably and vendor libraries remain isolated.
## Failure and recovery
Device/library/network failure cannot corrupt capture state or block unrelated EXM transports; ownership and reconnect are bounded.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused, affected, dependent and invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where native vendor libraries, USB and hardware timing are involved.
| AC ID | Observable criterion | Test level/scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | Mock instruments prove discovery/acquisition/trigger/capability negotiation | adapter contract/integration | PASS |
| AC-02 |  unsupported features fail closed | negative/integration | PASS |
| AC-03 |  reconnect preserves no stale ownership. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revision, SDK/backend versions, device identity where available, commands/manual scenarios, timing limits, outcomes, limitations and cleanup.
