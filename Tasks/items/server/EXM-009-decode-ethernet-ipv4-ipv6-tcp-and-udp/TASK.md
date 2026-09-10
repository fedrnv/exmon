---
schema: fdtasks-spec/v3
id: EXM-009
title: "Decode Ethernet, IPv4, IPv6, TCP, and UDP"
type: Task
area: server
component: "ip-decode"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-005", "EXM-008"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-009 — Decode Ethernet, IPv4, IPv6, TCP, and UDP

## Goal

Decode common network layers without losing original bytes.

## Accepted scope

- Parse VLAN, IPv4, IPv6, TCP, and UDP metadata.
- Validate lengths and checksums where available.
- Retain unknown protocols and malformed frames.

## Acceptance criteria

- Known fixtures decode exactly.
- Malformed traffic cannot panic or escape configured bounds.

This specification is immutable after acceptance. Add events for later changes.
