---
schema: fdtasks-spec/v3
id: EXM-048
title: "Prove initial Exchange Monitor functional release"
type: Task
area: quality
component: "epic-gate"
epic_id: EXM-EPIC-001
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-001","EXM-002","EXM-003","EXM-004","EXM-005","EXM-006","EXM-007","EXM-008","EXM-009","EXM-010","EXM-011","EXM-012","EXM-013","EXM-014","EXM-015","EXM-016","EXM-017","EXM-018","EXM-019","EXM-020","EXM-021","EXM-022","EXM-023","EXM-024","EXM-025","EXM-026","EXM-027","EXM-028","EXM-029","EXM-030","EXM-031","EXM-032","EXM-033","EXM-034","EXM-035","EXM-036","EXM-037","EXM-038","EXM-039"]
created_at: 2026-09-19T14:25:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-048 — Prove initial Exchange Monitor functional release
## Goal
Run the complete canonical FDTasks v4.2 gate for all EXM-001..039 on one frozen candidate.
## Acceptance matrix
| AC ID | Criterion | Scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | All Epic 001 member acceptance criteria have current executable evidence | complete canonical gate | PASS |
| AC-02 | Supported capture/transmit/import/export/UI/platform/security scenarios pass | full project suites and required manual hardware checks | PASS or explicit policy-approved limitation |
| AC-03 | Cleanup leaves no owned disposable residue and evidence binds exact source | cleanup/evidence audit | PASS |
## Completion
Record source/policy SHA, exact commands/manual scenarios, environments, outcomes, limitations, visual evidence and cleanup. Functional failure requires correction and a fresh complete gate.
