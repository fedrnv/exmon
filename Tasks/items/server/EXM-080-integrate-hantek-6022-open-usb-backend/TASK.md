---
schema: fdtasks-spec/v3
id: EXM-080
title: "Integrate Hantek 6022 open USB backend"
type: Task
area: server
component: "hantek-6022-adapter"
epic_id: EXM-EPIC-005
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-076"]
created_at: 2026-09-19T16:05:00Z
created_by: "Fedor and ChatGPT"
---
# EXM-080 — Integrate Hantek 6022 open USB backend
## Goal
Support Hantek 6022BE/BL using the open libusb/OpenHantek-compatible protocol/firmware path for low-cost USB acquisition.
## Boundaries and compatibility
Open backend is isolated and device/firmware identity is validated before use.
## Failure and recovery
Device/library/network failure cannot corrupt capture state or block unrelated EXM transports; ownership and reconnect are bounded.
## Ordered implementation
Planner decomposes into bounded microtasks; run focused, affected, dependent and invariant verification, diff review, cleanup and immutable evidence per FDTasks v4.2.
## Readiness, impact and acceptance matrix
Policy: Tasks/tasks_file_structure_v4.md. Risk: high where native vendor libraries, USB and hardware timing are involved.
| AC ID | Observable criterion | Test level/scenario | Expected |
| --- | --- | --- | --- |
| AC-01 | USB fixture/available hardware captures expected samples | adapter contract/integration | PASS |
| AC-02 |  unsupported controls are explicit | negative/integration | PASS |
| AC-03 |  unplug/replug recovery is bounded. | recovery/invariant | PASS |
## Completion evidence and cleanup
Record exact source/policy revision, SDK/backend versions, device identity where available, commands/manual scenarios, timing limits, outcomes, limitations and cleanup.
