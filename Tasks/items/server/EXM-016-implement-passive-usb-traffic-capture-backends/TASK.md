---
schema: fdtasks-spec/v3
id: EXM-016
title: "Implement passive USB traffic capture backends"
type: Task
area: server
component: "usb-capture"
initial_priority: P2
initial_status: Planned
initial_dependencies: ["EXM-006", "EXM-007"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-016 — Implement passive USB traffic capture backends

## Goal

Capture observable host USB traffic where platform facilities permit it.

## Accepted scope

- Support Linux usbmon and evaluate supported Windows and macOS backends.
- Keep passive capture distinct from active USB communication.
- Report capability and installation requirements accurately.

## Acceptance criteria

- Supported platforms preserve transfer metadata and payload bytes.
- Unsupported platform/backend combinations fail explicitly, not silently.

This specification is immutable after acceptance. Add events for later changes.
