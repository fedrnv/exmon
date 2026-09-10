---
schema: fdtasks-spec/v3
id: EXM-005
title: "Create deterministic traffic fixtures"
type: Task
area: foundation
component: "sample-data"
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-001"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-005 — Create deterministic traffic fixtures

## Goal

Provide non-sensitive fixtures for every transport and framing mode.

## Accepted scope

- Include normal, fragmented, malformed, truncated, burst, and timestamped samples.
- Define expected classified messages and field values.
- License or generate every fixture for repository use.

## Acceptance criteria

- Fixtures contain no captured personal or production data.
- Core tests can run without physical hardware.

This specification is immutable after acceptance. Add events for later changes.
