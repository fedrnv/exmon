---
schema: fdtasks-spec/v3
id: EXM-015
title: "Implement USB Bulk and Interrupt communication"
type: Task
area: server
component: "usb-native"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-006", "EXM-007"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-015 — Implement USB Bulk and Interrupt communication

## Goal

Communicate with explicitly selected USB devices through Bulk and Interrupt endpoints.

## Accepted scope

- Enumerate configurations, interfaces, and endpoints.
- Support bounded asynchronous IN and explicit OUT transfers.
- Handle claim/release, timeout, stall, disconnect, and kernel-driver conflicts.

## Acceptance criteria

- Descriptor and transfer fixtures cover normal and failure paths.
- Physical-device tests record OS-specific behavior when hardware is available.

This specification is immutable after acceptance. Add events for later changes.
