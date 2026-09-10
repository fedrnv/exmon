---
schema: fdtasks-spec/v3
id: EXM-020
title: "Implement automatic message classification"
type: Task
area: server
component: "classification"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-019"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-020 — Implement automatic message classification

## Goal

Group identical message types using transport metadata, length, headers, masks, and user rules.

## Accepted scope

- Provide deterministic rule priority and fallback grouping.
- Allow assigning and persisting a user-visible name.
- Show why a message matched a type.

## Acceptance criteria

- Repeated equivalent messages select the same type.
- Ambiguous and unmatched observations remain visible and explainable.

This specification is immutable after acceptance. Add events for later changes.
