---
schema: fdtasks-epic/v4
id: <PROJECT>-EPIC-<ID>
title: "Delivery outcome"
initial_status: Planned
initial_dependencies: []
created_at: YYYY-MM-DDTHH:MM:SSZ
created_by: "actor"
---

# Epic outcome

## Maintenance exception

For `<PROJECT>-EPIC-999`, replace the delivery outcome with permanent maintenance:
isolated bugs, minor improvements, bugs whose related Epic is closed, and bugs
without a clear relationship to an open development Epic. Bugs belonging to an
open development Epic stay there. Link a closed original Epic without reopening it.

EPIC-999 remains open, does not count against the active-development-Epic limit,
and has no mandatory overall gate or review tasks. Its ordinary items require
testing changed functionality and directly affected behaviour. Full testing and/or
independent review require an explicit verification task inside EPIC-999; one may
request both. Replace the development-only membership/gate/closure sections below
accordingly. Never close EPIC-999 or generate its Epic version request.


## Outcome

Non-executable delivery and integration container.

## Accepted membership

Immutable links to medium implementation items plus exactly one full-gate task
and one independent-review task. Later additions use membership events.

## Gate and review

Policy SHA: <full SHA>. Full-gate task: <ID/link>. Review task: <ID/link>.
The gate depends on implementation/correction completion; review depends on the
gate. Re-evaluate membership from all events.

Epic closure requires every member Done, a current complete passing gate, a
current independent Accepted review, resolved findings, cleanup and evidence.
Candidate changes create correction items and reopen/repeat gate and review.
