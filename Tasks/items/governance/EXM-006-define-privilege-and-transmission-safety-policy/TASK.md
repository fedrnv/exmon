---
schema: fdtasks-spec/v3
id: EXM-006
title: "Define privilege and transmission safety policy"
type: Task
area: governance
component: "security"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-001"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-006 — Define privilege and transmission safety policy

## Goal

Specify platform permissions, driver trust, capture privacy, and safeguards for active transmission.

## Accepted scope

- Cover Npcap/libpcap, serial, USB, and CAN permissions.
- Require explicit enablement for periodic transmission.
- Define capture-file handling and redaction policy.

## Acceptance criteria

- The policy lists platform-specific privilege boundaries.
- No active transmission can start implicitly on application launch.

This specification is immutable after acceptance. Add events for later changes.
