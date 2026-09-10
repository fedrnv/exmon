---
schema: fdtasks-spec/v3
id: EXM-001
title: "Define the product domain model"
type: Task
area: foundation
component: "product-model"
initial_priority: P0
initial_status: Planned
initial_dependencies: []
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-001 — Define the product domain model

## Goal

Define transport-neutral identities and lifecycle for interfaces, observations, messages, message types, fields, charts, transmit templates, and sessions.

## Accepted scope

- Document RX/TX direction, timestamps, original bytes, metadata, loss, and error semantics.
- Define stable identifiers and bounded retention rules.
- Provide examples for Ethernet, TCP, UDP, serial, USB, and CAN.

## Acceptance criteria

- The model covers every accepted transport without transport-specific GUI branching.
- Queue overflow and malformed data have explicit observable outcomes.

This specification is immutable after acceptance. Add events for later changes.
