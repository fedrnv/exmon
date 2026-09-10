---
schema: fdtasks-spec/v3
id: EXM-033
title: "Establish performance and soak benchmarks"
type: Task
area: quality
component: "performance"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-004", "EXM-019", "EXM-020", "EXM-026", "EXM-028"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-033 — Establish performance and soak benchmarks

## Goal

Measure sustainable capture, classification, decoding, storage, and rendering rates.

## Accepted scope

- Benchmark small packets, mixed types, fragmented streams, chart-heavy sessions, and slow disks.
- Define target hardware and accepted drop/latency/memory thresholds.
- Run multi-hour soak tests with bounded growth.

## Acceptance criteria

- Reports separate backend loss, application drops, and display decimation.
- Regressions above accepted thresholds block release.

This specification is immutable after acceptance. Add events for later changes.
