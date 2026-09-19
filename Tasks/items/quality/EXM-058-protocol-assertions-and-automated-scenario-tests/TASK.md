---
schema: fdtasks-spec/v3
id: EXM-058
title: "Protocol assertions and automated scenario tests"
type: Task
area: quality
component: "protocol-test-runner"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-054","EXM-056"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-058 — Protocol assertions and automated scenario tests
## Goal
Define assertions over received packets, ordering, timing, fields, state and expected absence; execute scenarios as repeatable PASS/FAIL protocol tests with diagnostics.
## Boundaries and compatibility
Assertions use captured evidence and explicit tolerances; timeout/unknown is not reported as PASS.
## Failure and recovery
Failures, cancellation and restart preserve authoritative state and prevent unintended or duplicate transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each and affected/dependent/invariant verification before closure, then diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where physical transmission or concurrency is involved.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Known good scenario passes | deterministic focused/integration fixture | PASS |
| AC-02 |  wrong value/order/timing fails with exact reason | negative/integration | PASS |
| AC-03 |  reports bind scenario/protocol revisions. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, timing tolerances, limitations, applicable UI visual evidence and cleanup.
