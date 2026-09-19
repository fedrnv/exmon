---
schema: fdtasks-spec/v3
id: EXM-083
title: "Independently review external instrument integration Epic"
type: Task
area: quality
component: "epic-level-zero-review"
epic_id: EXM-EPIC-005
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-082"]
created_at: 2026-09-19T16:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-083 — Independently review external instrument integration Epic
## Goal
Perform independent Level-0 review of the exact EXM-082 candidate.
## Acceptance matrix
| AC ID | Criterion | Scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | Review source equals gated source | evidence identity | exact match |
| AC-02 | Adapter isolation, timing, native-library safety and user-visible capability behavior have no blocking finding | independent review | Accepted |
| AC-03 | Findings create medium Corrections in Epic 005 and force fresh gate/review | lifecycle | FDTasks v4.2 preserved |
## Completion
Store immutable review REQUEST/RESULT; only Accepted review for current tested source closes this item.
