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
