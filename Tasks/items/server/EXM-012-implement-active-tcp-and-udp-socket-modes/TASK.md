---
schema: fdtasks-spec/v3
id: EXM-012
title: "Implement active TCP and UDP socket modes"
type: Task
area: server
component: "socket-monitor"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-007", "EXM-009"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-012 — Implement active TCP and UDP socket modes

## Goal

Allow Exchange Monitor to listen, connect, receive, and send through normal sockets without raw-capture privileges.

## Accepted scope

- Support TCP client/server and UDP bound/connected modes.
- Support IPv4/IPv6 and configurable local/remote endpoints.
- Expose connection lifecycle and errors.

## Acceptance criteria

- Loopback integration tests verify bidirectional byte fidelity.
- Socket shutdown and reconnect do not leak tasks or descriptors.

This specification is immutable after acceptance. Add events for later changes.
