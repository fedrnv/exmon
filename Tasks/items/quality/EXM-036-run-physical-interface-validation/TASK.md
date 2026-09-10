---
schema: fdtasks-spec/v3
id: EXM-036
title: "Run physical interface validation"
type: Task
area: quality
component: "hardware-matrix"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-013", "EXM-014", "EXM-015", "EXM-017", "EXM-023"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-036 — Run physical interface validation

## Goal

Validate RS-485, USB CDC, HID, Bulk/Interrupt, and CAN with declared devices and loopback peers.

## Accepted scope

- Record device/adapter model, driver, OS, wiring, rates, test vectors, and observed errors.
- Test reconnect, malformed input, sustained load, and bidirectional traffic.
- Keep sensitive serial numbers and captures out of Git.

## Acceptance criteria

- Each supported hardware class has reproducible evidence on available target OSes.
- Unavailable hardware remains explicitly blocked rather than assumed working.

This specification is immutable after acceptance. Add events for later changes.
