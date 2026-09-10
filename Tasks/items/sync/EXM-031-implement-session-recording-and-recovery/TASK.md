---
schema: fdtasks-spec/v3
id: EXM-031
title: "Implement session recording and recovery"
type: Task
area: sync
component: "session-storage"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-003", "EXM-004", "EXM-020"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-031 — Implement session recording and recovery

## Goal

Persist long monitoring sessions, definitions, names, annotations, and chart samples safely.

## Accepted scope

- Use SQLite or an equivalently transactional local store.
- Batch writes off the capture path and enforce retention limits.
- Detect interrupted sessions and recover committed data.

## Acceptance criteria

- Restart recovery is deterministic after simulated interruption.
- Storage backpressure is observable and cannot exhaust memory.

This specification is immutable after acceptance. Add events for later changes.
