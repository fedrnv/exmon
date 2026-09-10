---
schema: fdtasks-spec/v3
id: EXM-003
title: "Define versioned configuration schemas"
type: Task
area: foundation
component: "configuration"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-001"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-003 — Define versioned configuration schemas

## Goal

Define portable, forward-detecting configuration for interfaces, filters, framing, classification, fields, charts, and transmit schedules.

## Accepted scope

- Use versioned Serde schemas with validation and migration boundaries.
- Separate reusable protocol definitions from machine-specific device bindings.
- Support safe defaults and explicit unknown-version failure.

## Acceptance criteria

- Valid configurations round-trip deterministically.
- Invalid, unsupported, and unsafe configurations produce bounded actionable errors.

This specification is immutable after acceptance. Add events for later changes.
