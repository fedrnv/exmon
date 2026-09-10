---
schema: fdtasks-spec/v3
id: EXM-025
title: "Build the main application shell"
type: Task
area: web
component: "shell"
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-002", "EXM-007"]
created_at: 2026-09-10T00:00:00Z
created_by: "Fedor and Codex"
---

# EXM-025 — Build the main application shell

## Goal

Create the single-window egui desktop shell with interface selection and capture controls.

## Accepted scope

- Top bar selects transport/interface and exposes effective filters.
- Main area is vertically split into incoming left and outgoing right panels.
- Persist safe window/layout preferences.

## Acceptance criteria

- The shell remains responsive while mock capture produces sustained batches.
- No capture work runs on the GUI thread.

This specification is immutable after acceptance. Add events for later changes.
