---
schema: fdtasks-spec/v3
id: EXM-023
title: "Implement safe transmission commands"
type: Task
area: server
component: "transmit"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-007", "EXM-012", "EXM-013", "EXM-014", "EXM-015", "EXM-017", "EXM-022"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-023 — Implement safe transmission commands

## Goal

Send raw or structured messages through the selected compatible transport.

## Accepted scope

- Support one-shot send and cloning an incoming format into an outgoing template.
- Validate transport binding, payload, connection state, and permissions.
- Record TX observations and errors in the same session model.

## Acceptance criteria

- A message cannot be sent through an incompatible or stale interface binding.
- Successful and failed attempts are distinguishable in history.

This specification is immutable after acceptance. Add events for later changes.
