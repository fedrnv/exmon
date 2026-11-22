---
schema: fdtasks-event/v3
event_id: EVT-0a65911acb4f478da14d0b22281d09ae
task_id: EXM-007
event_type: readiness-accepted
created_at: 2026-11-22T17:56:46Z
actor: Codex exmon capture core development
base_revision: 056c2e6477e2437a6e87cc8af7244c37c9e88880
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
task_sha256: a295d2c5d79b8c3f90dfb4dbad8e16738b173730d81512fcb17c42488b5e8861
atomicity:
  outcomes: [Bounded capture adapter lifecycle and event contract]
  packets: [Implement shared observations and adapter session with deterministic mock verification]
  independent_acceptance: false
  boundary_reason: Observation producer and bounded session consumer form one adapter API
---

Dependencies EXM-001 and EXM-002 published Done (aca7a30/e5b79fe). EXM-006 product safety policy published Done at 9c7dd5c; current main adds only allocator metadata. EXM-007 Planned/unowned, medium in EXM-EPIC-001 by accepted migration. No other development Epic active. Root AGENTS.md/DEVELOPMENT.md, Tasks/AGENTS.md, FDTasks 5.7 adoption, ADR-0001/2/3 and docs/security-and-transmission.md apply; no deeper rules.

Allowed: crates/domain observation module/export, crates/capture API/session/mock tests and manifest, Cargo.lock if needed, docs/capture-api.md, this task's new evidence/events and first request. Implement transport-neutral validated observations, bounded batches and queue, enumerate/open/configure/start/stop/close/reconnect, cancellation token, fixed error/capability reporting and loss metrics. No physical adapter/driver, scheduler, GUI or actual network transmission. Trait TX hook defaults unsupported and is never invoked by lifecycle; future explicit TX worker must own authorization. Adapter polling is bounded/cooperative on background worker, never GUI. Risk: stale epochs, queue overflow and reconnect leaks; tests assert exact deterministic transitions, fresh cancellation/epoch, byte/count bounds and measured drops. Poll/backend errors invalidate running state and release ownership through stop/close; configuration failures cannot start RX/TX. Fixed metadata and bounded raw bytes, redacted debug, checked counters/ranges. Keep status/error observable outside data queue.

| AC | Scenario/level | Windows local command | Expected |
| --- | --- | --- | --- |
| AC-01 | Deterministic mock transitions/capabilities/errors/cancel/reconnect and observation bounds | cargo test -p exmon-capture --locked; cargo test -p exmon-domain --locked | Exact lifecycle and fresh epoch; invalid transitions/data fail; no implicit TX |
| AC-02 | Slow consumer count/byte saturation and bounded drain | cargo test -p exmon-capture --locked | Drop-new policy increments application drops; queue never exceeds configured count/bytes; health remains visible |

Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets --locked -- -D warnings, node scripts/check-architecture.mjs and git diff --check. No existing physical transport regression exists. Preserve ignored target/.idea; no platform privilege changes or hardware claims.