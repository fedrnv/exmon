---
schema: fdtasks-spec/v3
id: EXM-037
title: "Package native desktop applications"
type: Task
area: operations
component: "packaging"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-025", "EXM-035"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-037 — Package native desktop applications

## Goal

Produce installable artifacts with required runtime libraries and clear driver requirements.

## Accepted scope

- Package Windows, macOS, and Linux variants.
- Detect or explain Npcap/libpcap and permission prerequisites.
- Keep optional USB/CAN backends capability-driven.

## Acceptance criteria

- A clean machine can install, launch, and uninstall the application.
- Missing optional drivers do not prevent unrelated transports from working.

This specification is immutable after acceptance. Add events for later changes.
