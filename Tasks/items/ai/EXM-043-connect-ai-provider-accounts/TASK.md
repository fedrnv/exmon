---
schema: fdtasks-spec/v3
id: EXM-043
title: "Connect selectable AI provider accounts"
type: Task
area: ai
component: "ai-provider-registry"
epic_id: EXM-EPIC-001
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: []
created_at: 2026-09-19T13:50:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-043 — Connect selectable AI provider accounts
## Goal
Provide one assistant provider abstraction with selectable OpenAI, Claude Code, Grok and OpenRouter connections/models, using the user's own account/subscription or API access.
## Scope and boundaries
Secrets use OS-protected storage where applicable and never enter logs/tasks/protocol exports; provider failure cannot break normal monitoring.
## Failure and recovery
Failures remain explicit and recoverable without corrupting captured/protocol state or causing unintended transmission.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused checks after each, then affected/dependent/invariant verification, diff review, cleanup and immutable evidence/events per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: medium/high according to external-data, inference and device-transmission impact.
| AC ID | Observable criterion | Test level/scenario | Expected result |
| --- | --- | --- | --- |
| AC-01 | All providers satisfy one adapter contract | focused/integration | PASS with deterministic evidence |
| AC-02 |  synthetic-secret tests prove redaction | integration/negative | PASS |
| AC-03 |  offline/provider failure leaves capture and analysis usable. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revisions, commands/manual scenarios, environment, outcomes, limitations, applicable UI visual evidence and cleanup.