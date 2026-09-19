---
schema: fdtasks-spec/v3
id: EXM-047
title: "Independently review protocol reconstruction and AI assistant Epic"
type: Task
area: quality
component: "epic-level-zero-review"
epic_id: EXM-EPIC-001
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-046"]
created_at: 2026-09-19T13:50:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-047 — Independently review protocol reconstruction and AI assistant Epic
## Goal
Perform an independent Level-0 review of the exact EXM-046 candidate.
## Scope
Review actual diff/evidence, inference traceability, protocol export schema, provider credential handling, external-data disclosure, AI packet-authoring safety, GUI behavior and complete gate evidence. Reviewer must be independent from implementation and cannot replace missing executable evidence with model judgment.
## Acceptance matrix
| AC ID | Criterion | Scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | Review targets exact gated source | evidence identity check | exact match |
| AC-02 | No unresolved blocking findings | independent Level-0 review | Accepted |
| AC-03 | Findings become medium Corrections in this Epic and force fresh gate/review | lifecycle check | FDTasks v4.2 preserved |
## Completion
Store immutable REQUEST/RESULT review artifacts and evidence. Only an Accepted result for the current tested source may close this task.
