---
schema: fdtasks-spec/v3
id: EXM-027
title: "Build outgoing message tabs and editor"
type: Task
area: web
component: "outgoing-ui"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-022", "EXM-023", "EXM-024", "EXM-025"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-027 — Build outgoing message tabs and editor

## Goal

Provide one outgoing tab per transmit template with raw and structured editing.

## Accepted scope

- Create manually or clone from incoming.
- Edit fields or HEX, validate, send once, configure schedule, and stop schedule.
- Display bound interface, TX count, last result, and validation errors.

## Acceptance criteria

- Invalid data cannot be transmitted.
- Clone-edit-send works end to end with a mock transport.

This specification is immutable after acceptance. Add events for later changes.
