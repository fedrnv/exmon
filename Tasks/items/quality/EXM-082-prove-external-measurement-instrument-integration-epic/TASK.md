---
schema: fdtasks-spec/v3
id: EXM-082
title: "Prove external measurement instrument integration Epic"
type: Task
area: quality
component: "epic-gate"
epic_id: EXM-EPIC-005
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-076","EXM-077","EXM-078","EXM-079","EXM-080","EXM-081"]
created_at: 2026-09-19T16:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-082 — Prove external measurement instrument integration Epic
## Goal
Run the complete canonical gate on one frozen candidate across the common instrument API, all four adapters and EXM timeline integration.
## Acceptance matrix
| AC ID | Criterion | Scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | Common capability contract works across supported adapters | mock + available hardware matrix | PASS |
| AC-02 | Analog acquisition/trigger data enters EXM Signal model correctly | end-to-end capture | PASS |
| AC-03 | Device loss/reconnect/native library failures remain isolated | recovery matrix | PASS |
| AC-04 | Timeline mapping exposes clock uncertainty and synchronizes inspection | timing/UI scenario | PASS |
## Completion
Freeze source SHA and record SDK/backend/device versions, commands, environments, timing tolerances, outcomes, limitations, visual evidence and cleanup.
