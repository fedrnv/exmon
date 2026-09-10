---
schema: fdtasks-spec/v3
id: EXM-024
title: "Implement periodic transmission schedules"
type: Task
area: server
component: "scheduler"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-023"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-024 — Implement periodic transmission schedules

## Goal

Send configured messages at controlled intervals without blocking capture or GUI work.

## Accepted scope

- Support interval, start/stop, finite count, drift policy, and missed-deadline policy.
- Require explicit per-session activation.
- Provide immediate cancellation and measured timing.

## Acceptance criteria

- Schedules never start automatically after application launch or config import.
- Timing and cancellation tests pass under simulated load.

This specification is immutable after acceptance. Add events for later changes.
