---
schema: fdtasks-spec/v3
id: EXM-028
title: "Build real-time chart tabs"
type: Task
area: web
component: "charts"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-021", "EXM-025", "EXM-026"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-028 — Build real-time chart tabs

## Goal

Plot selected numeric fields from selected message types with controllable scales and moving averages.

## Accepted scope

- Support multiple series, time/index X axis, zoom, pan, autoscale, manual ranges, units, and legends.
- Support configurable sample windows and moving-average lengths.
- Bound and decimate retained/rendered points without changing stored raw observations.

## Acceptance criteria

- Charts remain responsive for long fixture sessions.
- Moving-average values match deterministic reference vectors.

This specification is immutable after acceptance. Add events for later changes.
