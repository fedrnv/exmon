---
schema: fdtasks-spec/v3
id: EXM-022
title: "Implement message decoding and encoding"
type: Task
area: server
component: "codec"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-021"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-022 — Implement message decoding and encoding

## Goal

Decode incoming messages and build outgoing bytes from field values.

## Accepted scope

- Preserve undecoded bytes and allow HEX editing.
- Support checksums through a constrained extension interface.
- Return field-level validation errors before transmission.

## Acceptance criteria

- Decode-encode round trips preserve unchanged messages.
- Edited fields update only declared bytes and dependent checksums.

This specification is immutable after acceptance. Add events for later changes.
