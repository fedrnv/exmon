---
schema: fdtasks-spec/v3
id: EXM-019
title: "Implement configurable message framing"
type: Task
area: server
component: "framing"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-003", "EXM-005", "EXM-011", "EXM-013"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-019 — Implement configurable message framing

## Goal

Convert datagrams, frames, and byte streams into application messages using configurable rules.

## Accepted scope

- Support native frame boundary, fixed length, length field, start/end markers, delimiter, and idle-gap framing.
- Support escaping and maximum-message limits.
- Retain framing diagnostics and source byte ranges.

## Acceptance criteria

- Chunk boundaries do not change framed output.
- Malformed and oversized streams recover according to declared policy.

This specification is immutable after acceptance. Add events for later changes.
