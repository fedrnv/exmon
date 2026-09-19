---
schema: fdtasks-spec/v3
id: EXM-046
title: "Prove protocol reconstruction and AI assistant Epic"
type: Task
area: quality
component: "epic-gate"
epic_id: EXM-EPIC-001
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-040","EXM-041","EXM-042","EXM-043","EXM-044","EXM-045"]
created_at: 2026-09-19T13:50:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-046 — Prove protocol reconstruction and AI assistant Epic
## Goal
Run the complete canonical gate on one frozen candidate: live/session capture → protocol reconstruction → export/re-import, plus provider adapters, right-side assistant, exchange analysis and outgoing packet draft creation.
## Boundaries
Prove credential redaction, explicit external-context disclosure, failure/recovery and transmission authorization. AI-generated traffic must never reach a physical device without explicit authorized test action.
## Acceptance matrix
| AC ID | Criterion | Scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | Protocol capture/reconstruction/export round trip | canonical deterministic fixture | PASS |
| AC-02 | AI analysis and draft-authoring workflow | mock providers + native GUI | PASS |
| AC-03 | Secrets/data-egress/send authorization invariants | security fixtures | PASS |
| AC-04 | Supported desktop build/UI checks | canonical project checks | PASS or policy-approved documented limitation |
## Completion
Freeze source SHA; record complete commands, environments, visual evidence, outcomes, limitations and cleanup. Any functional failure rejects the candidate and requires correction plus a fresh full gate.
