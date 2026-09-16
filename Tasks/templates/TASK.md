---
schema: fdtasks-spec/v3
id: <PROJECT>-<ID>
title: "Observable medium outcome"
type: Task
area: <area>
component: <component>
epic_id: <PROJECT>-EPIC-<ID>
size: medium
initial_priority: P1
initial_status: Planned
initial_dependencies: []
created_at: YYYY-MM-DDTHH:MM:SSZ
created_by: "actor"
---

# <PROJECT>-<ID> — Outcome

## Goal

One independently verifiable outcome.

## Inputs and source baseline

Exact source revision, specifications, contracts and prior evidence.

## Scope

Included behaviour only.

## Boundaries and compatibility

Explicit exclusions, security, data and compatibility boundaries.

## Failure and recovery

Error, cancellation, retry and preservation behaviour.

## Ordered implementation

Planner decomposition, sequential microtasks, focused checks, affected/dependent
verification, diff review, cleanup, immutable evidence/event/version request and
direct publication according to repository policy.

## Readiness, impact and acceptance matrix

Policy SHA: <full SHA>. Risk: <low/medium/high + reason>.
Affected components and dependent callers: <paths/scenarios>.
UI/data/concurrency/compatibility/security impact: <assessment>.

| AC ID | Observable criterion/tolerance | Test level and scenario | Trusted command or manual steps | Fixture/environment | Expected result |
| --- | --- | --- | --- | --- | --- |
| AC-01 | <criterion> | <unit/integration/UI/etc.> | <command/steps> | <fixture> | <result> |

## Completion evidence and cleanup

Record exact source, environment, outcomes, limitations, review and inspected
visual evidence when applicable. Never leave placeholders in a ready task.
This specification is immutable after acceptance; later changes are events.
