---
schema: fdtasks-spec/v3
id: EXM-004
title: "Implement structured diagnostics and metrics"
type: Task
area: foundation
component: "observability"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-002"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-004 — Implement structured diagnostics and metrics

## Goal

Make capture health, throughput, queue depth, drops, decode failures, and transmission errors observable.

## Accepted scope

- Add structured logs and in-memory metrics.
- Redact payloads and device identifiers by default.
- Expose health snapshots to the GUI.

## Acceptance criteria

- Metrics do not block capture paths.
- Tests verify redaction and counter behavior.

This specification is immutable after acceptance. Add events for later changes.
