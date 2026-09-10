---
schema: fdtasks-event/v3
event_id: EVT-20260910T000000Z-7F39B2A1
task_id: EXM-036
event_type: dependency-added
created_at: 2026-09-10T00:00:00Z
actor: "Fedor and Codex"
base_revision: null
server_revision: null
---

# Require the complete virtual prototype before stand validation

## Payload

Add effective dependency `EXM-039`. Physical-interface validation begins after the complete mock/replay/loopback prototype is available. This preserves the accepted EXM-036 specification while recording the prototype-first execution order required by ADR-0003.

## Verification

EXM-039 and ADR-0003 exist on `main`; no physical validation result is claimed by this event.
