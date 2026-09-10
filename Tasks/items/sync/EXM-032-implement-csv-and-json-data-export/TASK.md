---
schema: fdtasks-spec/v3
id: EXM-032
title: "Implement CSV and JSON data export"
type: Task
area: sync
component: "data-export"
initial_priority: P2
initial_status: Planned
initial_dependencies: ["EXM-021", "EXM-031"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-032 — Implement CSV and JSON data export

## Goal

Export selected observations, decoded fields, and chart series for external analysis.

## Accepted scope

- Stream exports with explicit timestamps, units, endianness-independent values, and schema version.
- Support time/type/field selection.
- Prevent formula injection in spreadsheet-oriented CSV.

## Acceptance criteria

- Exports match deterministic fixtures.
- Large exports use bounded memory and support cancellation.

This specification is immutable after acceptance. Add events for later changes.
