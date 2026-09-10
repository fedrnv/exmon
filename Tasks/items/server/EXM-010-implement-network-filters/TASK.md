---
schema: fdtasks-spec/v3
id: EXM-010
title: "Implement network filters"
type: Task
area: server
component: "network-filter"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-008", "EXM-009"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-010 — Implement network filters

## Goal

Filter captured traffic by interface, direction, protocol, addresses, and ports.

## Accepted scope

- Compile supported filters to BPF when possible.
- Apply equivalent safe post-capture filters where necessary.
- Expose filter validation before capture starts.

## Acceptance criteria

- Port filtering works for IPv4 and IPv6 TCP/UDP.
- Displayed filter state matches the effective capture filter.

This specification is immutable after acceptance. Add events for later changes.
