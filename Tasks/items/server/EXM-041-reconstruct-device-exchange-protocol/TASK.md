---
schema: fdtasks-spec/v3
id: EXM-041
title: "Reconstruct device exchange protocol"
type: Task
area: server
component: "protocol-reconstruction"
epic_id: EXM-EPIC-001
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-040","EXM-020","EXM-021","EXM-022"]
created_at: 2026-09-19T13:50:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-041 — Reconstruct device exchange protocol
## Goal
Build an editable protocol model from recorded exchanges: message types, direction, framing, fields, constants/variables, request-response relationships and observed sequences, with confidence and source-evidence links.
## Scope and boundaries
Insufficient or contradictory evidence remains Unknown/ambiguous; never fabricate certainty.
## Failure and recovery
Failures remain explicit and recoverable without corrupting captured/protocol state or causing unintended transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence/events per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high according to external-data, inference and device-transmission impact.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Known synthetic protocol is reconstructed | focused/integration | PASS with deterministic evidence |
| AC-02 |  every inferred rule links to observations | integration/negative | PASS |
| AC-03 |  conflicting fixtures remain explicitly unresolved. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, limitations, applicable UI visual evidence and cleanup.