---
schema: fdtasks-spec/v3
id: EXM-034
title: "Fuzz parsers, framing, decoders, and capture imports"
type: Task
area: quality
component: "fuzzing"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-005", "EXM-018", "EXM-019", "EXM-021", "EXM-022"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-034 — Fuzz parsers, framing, decoders, and capture imports

## Goal

Prove malformed and hostile bytes cannot panic, hang, or violate bounds.

## Accepted scope

- Fuzz network decoding, PCAP/PCAPNG, framing modes, field definitions, encoding, and checksums.
- Retain minimized non-sensitive regressions.
- Apply CPU, memory, and message-size limits.

## Acceptance criteria

- Every discovered crash becomes a deterministic regression test.
- The defined fuzz duration completes without unresolved crashes.

This specification is immutable after acceptance. Add events for later changes.
