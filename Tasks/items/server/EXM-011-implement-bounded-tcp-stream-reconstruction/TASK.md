---
schema: fdtasks-spec/v3
id: EXM-011
title: "Implement bounded TCP stream reconstruction"
type: Task
area: server
component: "tcp-reassembly"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-009"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-011 — Implement bounded TCP stream reconstruction

## Goal

Reconstruct bidirectional TCP byte streams for application-message framing.

## Accepted scope

- Handle ordering, retransmission, overlap policy, gaps, FIN/RST, and timeout.
- Key flows by endpoints and interface context.
- Bound memory and expose incomplete stream regions.

## Acceptance criteria

- Deterministic tests cover fragmentation, retransmission, gaps, and teardown.
- Resource limits prevent hostile streams from exhausting memory.

This specification is immutable after acceptance. Add events for later changes.
