---
schema: fdtasks-spec/v3
id: EXM-002
title: "Initialize the Rust workspace"
type: Task
area: foundation
component: "workspace"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-001"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-002 — Initialize the Rust workspace

## Goal

Create the compilable Cargo workspace and enforce architectural boundaries.

## Accepted scope

- Create crates for domain, capture, protocol, storage, application, and GUI.
- Add formatting, linting, testing, logging, error, and serialization baselines.
- Exclude build products and sensitive captures from Git.

## Acceptance criteria

- The workspace builds and tests on the development host.
- Dependency direction prevents GUI code from entering capture and protocol crates.

This specification is immutable after acceptance. Add events for later changes.
