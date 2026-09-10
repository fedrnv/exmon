---
schema: fdtasks-spec/v3
id: EXM-035
title: "Validate Windows, macOS, and Linux behavior"
type: Task
area: quality
component: "cross-platform"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-008", "EXM-013", "EXM-014", "EXM-015", "EXM-016", "EXM-017", "EXM-025"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-035 — Validate Windows, macOS, and Linux behavior

## Goal

Verify consistent product behavior and document platform-specific capabilities.

## Accepted scope

- Test install, launch, interface enumeration, permissions, capture, transmission, file dialogs, charts, settings, and shutdown.
- Cover Apple Silicon and current supported Windows/Linux targets; add Intel macOS if required.
- Use manual-only GitHub Actions or documented local commands.

## Acceptance criteria

- A signed-off compatibility matrix records passes, limitations, and required drivers.
- Core saved profiles remain portable across supported systems.

This specification is immutable after acceptance. Add events for later changes.
