---
schema: fdtasks-spec/v3
id: EXM-014
title: "Implement USB HID support"
type: Task
area: server
component: "usb-hid"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-006", "EXM-007"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-014 — Implement USB HID support

## Goal

Enumerate selected HID devices and monitor/send HID reports safely.

## Accepted scope

- Expose VID, PID, usage, report ID, direction, and report bytes.
- Support explicit open and report transmission.
- Handle device removal and permission errors.

## Acceptance criteria

- Mock and approved physical-device tests verify report fidelity.
- The UI never opens or transmits to an unselected HID device.

This specification is immutable after acceptance. Add events for later changes.
