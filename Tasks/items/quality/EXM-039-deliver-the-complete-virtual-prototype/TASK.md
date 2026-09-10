---
schema: fdtasks-spec/v3
id: EXM-039
title: "Deliver the complete virtual prototype"
type: Task
area: quality
component: "prototype"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-002", "EXM-003", "EXM-004", "EXM-005", "EXM-007", "EXM-019", "EXM-020", "EXM-021", "EXM-022", "EXM-023", "EXM-024", "EXM-025", "EXM-026", "EXM-027", "EXM-028", "EXM-029", "EXM-030", "EXM-031"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-039 — Deliver the complete virtual prototype

## Goal

Provide a complete end-to-end prototype that can be run and debugged before access to the physical exchange stand.

## Accepted scope

- Exercise the production domain, protocol, application, storage, and GUI paths through deterministic mock and replay adapters.
- Include Ethernet/IP fixtures, TCP/UDP loopback, virtual serial/CAN where available, simulated USB capabilities, and PCAP replay.
- Demonstrate interface selection, RX/TX tabs, classification and naming, framing, field decoding/editing, HEX/ASCII inspection, charts and moving averages, one-shot and scheduled transmission, settings, persistence, and diagnostics.
- Produce a documented demonstration scenario and retain non-sensitive verification evidence.

## Acceptance criteria

- A clean development machine can launch the prototype and execute the complete demonstration without physical exchange hardware.
- Automated end-to-end tests exercise capture/replay through GUI-visible state and outbound transmission through a mock or loopback sink.
- All expected prototype functions are usable; hardware-only limitations are explicitly labelled and do not crash or block unrelated functions.
- Format, lint, unit, integration, and relevant regression tests pass with bounded queues and no unbounded memory growth.

This specification is immutable after acceptance. Add events for later changes.
