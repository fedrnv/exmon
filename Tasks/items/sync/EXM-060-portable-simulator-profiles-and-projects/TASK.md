---
schema: fdtasks-spec/v3
id: EXM-060
title: "Portable simulator profiles and projects"
type: Task
area: sync
component: "simulator-profile"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-053","EXM-054","EXM-056"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-060 — Portable simulator profiles and projects
## Goal
Save/load a versioned portable simulator profile containing protocol reference, virtual devices, state machines, packet templates, scenarios, assertions and fault profiles.
## Boundaries and compatibility
Credentials, machine-local secrets and unrelated captures are excluded; incompatible versions fail closed.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Round trip is semantically equivalent | deterministic focused/integration fixture | PASS |
| AC-02 |  deterministic normalized export | negative/integration | PASS |
| AC-03 |  corrupt/unsupported profile leaves active project unchanged. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
