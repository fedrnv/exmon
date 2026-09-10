---
schema: fdtasks-spec/v3
id: EXM-021
title: "Implement binary field definitions"
type: Task
area: server
component: "field-schema"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-003", "EXM-020"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-021 — Implement binary field definitions

## Goal

Define typed fields inside messages for inspection, editing, and plotting.

## Accepted scope

- Support integers, floats, bits, byte arrays, text, scaling, units, signedness, and endianness.
- Support constants, enums, validity constraints, and derived fields.
- Validate overlap and out-of-range access.

## Acceptance criteria

- Known byte vectors decode and encode exactly.
- Invalid definitions cannot crash or read outside a message.

This specification is immutable after acceptance. Add events for later changes.
