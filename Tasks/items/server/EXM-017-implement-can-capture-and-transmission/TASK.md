---
schema: fdtasks-spec/v3
id: EXM-017
title: "Implement CAN capture and transmission"
type: Task
area: server
component: "can"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-006", "EXM-007"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-017 — Implement CAN capture and transmission

## Goal

Monitor and transmit CAN frames through supported OS adapters.

## Accepted scope

- Support classic CAN and design an extension boundary for CAN FD.
- Expose channel, ID, flags, DLC, direction, timestamp, and data.
- Provide SocketCAN first and isolated adapters for other platforms.

## Acceptance criteria

- Virtual CAN integration tests verify RX/TX and filtering.
- Unavailable drivers or hardware produce actionable capability errors.

This specification is immutable after acceptance. Add events for later changes.
