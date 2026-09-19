---
schema: fdtasks-spec/v3
id: EXM-074
title: "Prove real-time signal analysis and visualization Epic"
type: Task
area: quality
component: "epic-gate"
epic_id: EXM-EPIC-004
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-064","EXM-065","EXM-066","EXM-067","EXM-068","EXM-069","EXM-070","EXM-071","EXM-072","EXM-073"]
created_at: 2026-09-19T15:55:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-074 — Prove real-time signal analysis and visualization Epic
## Goal
Run the complete canonical gate on one frozen candidate for synchronized digital/analog visualization, derived real-time mathematics, triggers, statistics and packet inspection.
## Acceptance matrix
| AC ID | Criterion | Scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | Raw and derived signals remain timestamp/provenance correct | deterministic multi-stream fixtures | PASS |
| AC-02 | Digital analyzer, plots and packet inspector stay synchronized | native UI interaction | PASS |
| AC-03 | Expression/window/cross-stream math matches numeric tolerances under load | canonical processing suites | PASS |
| AC-04 | Triggers, counters and statistics survive recovery without duplicates | recovery/invariant suites | PASS |
## Completion
Freeze source SHA; record full commands, environments, numeric/timing tolerances, visual evidence, performance results, limitations and cleanup. Any functional failure requires correction and a fresh complete gate.
