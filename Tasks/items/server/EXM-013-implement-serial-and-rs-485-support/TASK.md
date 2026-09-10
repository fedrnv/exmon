---
schema: fdtasks-spec/v3
id: EXM-013
title: "Implement serial and RS-485 support"
type: Task
area: server
component: "serial"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-006", "EXM-007"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-013 — Implement serial and RS-485 support

## Goal

Monitor and transmit serial streams including RS-232, RS-422, RS-485, and USB CDC devices.

## Accepted scope

- Enumerate ports and expose baud, data bits, parity, stop bits, flow control, and timeouts.
- Support platform RS-485 direction-control capabilities where available.
- Detect disconnect/reconnect without losing unrelated sessions.

## Acceptance criteria

- Virtual serial tests verify framing input and TX output.
- Physical RS-485 validation is recorded when hardware is available.

This specification is immutable after acceptance. Add events for later changes.
