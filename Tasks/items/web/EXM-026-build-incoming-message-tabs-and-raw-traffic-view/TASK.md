---
schema: fdtasks-spec/v3
id: EXM-026
title: "Build incoming message tabs and raw traffic view"
type: Task
area: web
component: "incoming-ui"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-010", "EXM-020", "EXM-025"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-026 — Build incoming message tabs and raw traffic view

## Goal

Display raw observations and one incoming tab per classified message type.

## Accepted scope

- Show timestamp, direction, source/destination, protocol, length, HEX/ASCII, parsed fields, counts, and rates.
- Allow naming types, selecting instances, pausing display, and copying a format to outgoing.
- Virtualize or bound high-volume tables.

## Acceptance criteria

- Incoming groups update without losing selected context.
- High-rate fixtures keep capture active and GUI responsive.

This specification is immutable after acceptance. Add events for later changes.
