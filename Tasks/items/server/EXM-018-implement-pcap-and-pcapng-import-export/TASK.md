---
schema: fdtasks-spec/v3
id: EXM-018
title: "Implement PCAP and PCAPNG import/export"
type: Task
area: server
component: "pcap"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-008", "EXM-009"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-018 — Implement PCAP and PCAPNG import/export

## Goal

Load and save network sessions without losing timestamps or link-layer information.

## Accepted scope

- Stream large files with bounded memory.
- Preserve unknown link types and malformed records for inspection.
- Export selected or complete network observations.

## Acceptance criteria

- Round-trip tests preserve supported records.
- Truncated and hostile files fail safely with location-aware errors.

This specification is immutable after acceptance. Add events for later changes.
