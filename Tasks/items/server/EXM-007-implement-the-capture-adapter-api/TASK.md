---
schema: fdtasks-spec/v3
id: EXM-007
title: "Implement the capture adapter API"
type: Task
area: server
component: "capture-core"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-001", "EXM-002"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-007 — Implement the capture adapter API

## Goal

Provide a common lifecycle and event API for all capture and transmit backends.

## Accepted scope

- Enumerate/open/configure/start/stop interfaces.
- Deliver timestamped bounded batches with transport metadata.
- Support cancellation, reconnect, error, and capability reporting.

## Acceptance criteria

- A mock adapter proves deterministic lifecycle behavior.
- Slow consumers produce measured drops rather than unbounded memory growth.

This specification is immutable after acceptance. Add events for later changes.
