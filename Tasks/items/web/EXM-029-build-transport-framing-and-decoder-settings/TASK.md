---
schema: fdtasks-spec/v3
id: EXM-029
title: "Build transport, framing, and decoder settings"
type: Task
area: web
component: "settings"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-003", "EXM-019", "EXM-021", "EXM-025"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-029 — Build transport, framing, and decoder settings

## Goal

Provide validated settings windows for interfaces, filters, framing, classification, fields, and safety limits.

## Accepted scope

- Preview framing and decoding against selected captured bytes.
- Distinguish reusable protocol profiles from machine-specific bindings.
- Apply, cancel, import, export, and restore safe defaults.

## Acceptance criteria

- Invalid settings explain the exact field and do not alter the active configuration.
- Preview results match the capture pipeline.

This specification is immutable after acceptance. Add events for later changes.
