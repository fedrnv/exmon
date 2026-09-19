---
schema: fdtasks-spec/v3
id: EXM-044
title: "Build right-side AI assistant panel"
type: Task
area: web
component: "ai-assistant-ui"
epic_id: EXM-EPIC-001
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-043","EXM-025"]
created_at: 2026-09-19T13:50:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-044 — Build right-side AI assistant panel
## Goal
Add a collapsible/resizable right-side chat panel with provider/model selection, local conversation history, and explicit attachment of selected packets, ranges, protocol elements or analysis sessions.
## Scope and boundaries
Opening the panel sends no capture data externally; attached context is visible before sending.
## Failure and recovery
Failures remain explicit and recoverable without corrupting captured/protocol state or causing unintended transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence/events per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high according to external-data, inference and device-transmission impact.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | Panel does not disrupt monitoring | focused/integration | PASS with deterministic evidence |
| AC-02 |  exact selected context is shown | integration/negative | PASS |
| AC-03 |  provider failure preserves draft/history. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, limitations, applicable UI visual evidence and cleanup.