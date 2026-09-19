---
schema: fdtasks-spec/v3
id: EXM-077
title: "Integrate Digilent WaveForms instruments"
type: Task
area: server
component: "digilent-waveforms-adapter"
epic_id: EXM-EPIC-005
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-076"]
created_at: 2026-09-19T16:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-077 — Integrate Digilent WaveForms instruments
## Goal
Support compatible Digilent Analog Discovery/WaveForms devices through the official WaveForms SDK for analog acquisition, trigger and available generator/digital capabilities.
## Boundaries and compatibility
Use documented SDK contracts and report exact detected capabilities/model.
## Failure and recovery
Device/library/network failure cannot corrupt capture state or block unrelated EXM transports; ownership and reconnect are bounded.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused, affected, dependent and invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where native vendor libraries, USB and hardware timing are involved.
| AC ID | Observable criterion | Test level/scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | Mock/available-hardware contract tests cover acquisition/configuration | adapter contract/integration | PASS |
| AC-02 |  trigger semantics map correctly | negative/integration | PASS |
| AC-03 |  disconnect/error recovery is bounded. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revision, SDK/backend versions, device identity where available, commands/manual scenarios, timing limits, outcomes, limitations and cleanup.
