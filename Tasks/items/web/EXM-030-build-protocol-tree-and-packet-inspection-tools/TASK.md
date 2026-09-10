---
schema: fdtasks-spec/v3
id: EXM-030
title: "Build protocol tree and packet inspection tools"
type: Task
area: web
component: "wireshark-view"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-009", "EXM-018", "EXM-026"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-030 — Build protocol tree and packet inspection tools

## Goal

Provide Wireshark-like inspection for supported captured network traffic.

## Accepted scope

- Show expandable protocol layers linked to highlighted HEX byte ranges.
- Support search, copy, selection export, and display filters.
- Preserve unsupported protocol bytes for raw inspection.

## Acceptance criteria

- Layer selection highlights the correct byte range.
- Imported and live-captured frames use the same inspection model.

This specification is immutable after acceptance. Add events for later changes.
