---
schema: fdtasks-spec/v3
id: EXM-040
title: "Record protocol analysis sessions"
type: Task
area: server
component: "protocol-analysis-capture"
epic_id: EXM-EPIC-001
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-007","EXM-019"]
created_at: 2026-09-19T13:50:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-040 — Record protocol analysis sessions
## Goal
Create reproducible analysis sessions from live device traffic with ordered inbound/outbound packets, timestamps, transport/framing metadata, raw bytes, decoded fields and operator action markers.
## Scope and boundaries
Capture data stays local unless explicitly attached to an AI request; no automatic transmission.
## Failure and recovery
Failures remain explicit and recoverable without corrupting captured/protocol state or causing unintended transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence/events per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high according to external-data, inference and device-transmission impact.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Ordered fixture traffic reproduces exact bytes/direction/timing | focused/integration | PASS with deterministic evidence |
| AC-02 |  action markers preserve ordering | integration/negative | PASS |
| AC-03 |  interrupted/reopened sessions preserve prior data. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, limitations, applicable UI visual evidence and cleanup.