---
schema: fdtasks-event/v3
event_id: EVT-<UUID-or-ULID>
task_id: <PROJECT>-<ID>
event_type: <type>
created_at: YYYY-MM-DDTHH:MM:SSZ
actor: "actor"
base_revision: "<expected-full-main-sha>"
server_revision: null
---

# Event title

## Payload

One logical change or result. Use explicit `status_from`/`status_to` fields for a
state transition where the repository validator supports them.

## Verification

Exact commands/results and durable evidence references, or explicit unavailable
verification. Never invent a server revision or passing result.
