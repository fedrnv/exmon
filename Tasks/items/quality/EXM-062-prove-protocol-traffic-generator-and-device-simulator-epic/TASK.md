---
schema: fdtasks-spec/v3
id: EXM-062
title: "Prove protocol traffic generator and device simulator Epic"
type: Task
area: quality
component: "epic-gate"
epic_id: EXM-EPIC-003
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-050","EXM-051","EXM-052","EXM-053","EXM-054","EXM-055","EXM-056","EXM-057","EXM-058","EXM-059","EXM-060","EXM-061"]
created_at: 2026-09-19T15:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-062 — Prove protocol traffic generator and device simulator Epic
## Goal
Run the complete canonical gate on one frozen candidate and prove Exchange Monitor can replace/stimulate arbitrary protocol peers with deterministic, safe and observable traffic.
## Acceptance matrix
| AC ID | Criterion | Scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | Manual/periodic/reactive/stateful/scenario/replay workflows operate end to end | canonical deterministic protocol fixtures | PASS |
| AC-02 | Faults, timing, concurrency and assertions behave within defined tolerances | stress/fault fixtures | PASS |
| AC-03 | No scenario or AI draft bypasses transmission authorization | security/invariant suite | zero unauthorized sends |
| AC-04 | Simulator profile round-trip and UI sequence/timing views match authoritative events | integration/UI | PASS |
## Completion
Freeze source SHA and record full commands, environments, timing tolerances, visual evidence, outcomes, limitations and cleanup. Any functional failure requires correction and a fresh complete gate.
