---
schema: fdtasks-spec/v3
id: EXM-008
title: "Implement Ethernet interface discovery and raw capture"
type: Task
area: server
component: "ethernet"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-006", "EXM-007"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-008 — Implement Ethernet interface discovery and raw capture

## Goal

Capture raw Ethernet traffic from selectable interfaces on Windows, macOS, and Linux.

## Accepted scope

- Use Npcap on Windows and libpcap-compatible APIs on macOS/Linux.
- Support promiscuous mode where permitted, snapshot length, timeout, and BPF filter.
- Report unavailable privileges and unsupported capabilities.

## Acceptance criteria

- Fixtures and platform smoke tests verify capture metadata and byte fidelity.
- Capture can stop cleanly under load.

This specification is immutable after acceptance. Add events for later changes.
