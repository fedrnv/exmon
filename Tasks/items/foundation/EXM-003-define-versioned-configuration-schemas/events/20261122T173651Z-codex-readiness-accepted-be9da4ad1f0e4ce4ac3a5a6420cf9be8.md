---
schema: fdtasks-event/v3
event_id: EVT-be9da4ad1f0e4ce4ac3a5a6420cf9be8
task_id: EXM-003
event_type: readiness-accepted
created_at: 2026-11-22T17:36:51Z
actor: Codex exmon configuration development
base_revision: fe6b786f47c37f121507e1e1332f8b1346d8c1fd
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
task_sha256: a51dcea4cfddc7adcb9dab752b23081218b7735bcd54713617db6d164224e68e
atomicity:
  outcomes: [Validated portable version-one configuration contract]
  packets: [Implement Serde configuration types and bounded validated JSON boundary]
  independent_acceptance: false
  boundary_reason: Schema types and validated loader implement one configuration contract
---

EXM-001 published Done at aca7a30; EXM-002 published Done at e5b79fe supplies executable workspace. Current remote fe6b786f47c37f121507e1e1332f8b1346d8c1fd contains version allocation only since those closures. EXM-003 Planned without owner, medium in EXM-EPIC-001 by accepted migration. No other active development Epic. Applicable sources: root AGENTS.md, DEVELOPMENT.md, Tasks/AGENTS.md, FDTasks 5.7 adoption and ADR-0001/2/3. No deeper instructions.

Scope: crates/domain/src/config.rs and export, crates/storage configuration module/tests/fixtures and error dependency, Cargo.lock if required, docs/configuration.md, this item's new evidence/events and first version request. One portable schema for interfaces, filters, framing, classification, fields, charts, templates and schedules; machine bindings separate from reusable protocols. Exclude executing adapters, field codecs, scheduling, UI and disk I/O. Initial version 1; missing/future versions fail explicitly, no implicit migration. Default TX disabled; enabled persisted schedules rejected rather than armed. Bound input and collection/string/range sizes, report fixed bounded errors without reflecting device/user input. Unknown fields rejected to avoid silent semantic loss. Risk: malformed lengths/references and accidental arming; negative boundary tests and no executable TX side effects mitigate. Recovery: preserve original document on error; callers can edit or explicitly migrate later, never silently downgrade.

| AC | Scenario/level | Windows local command | Expected |
| --- | --- | --- | --- |
| AC-01 | Schema fixtures roundtrip all transport and framing variants; portable protocols versus local bindings; defaults | cargo test -p exmon-storage --locked | Deterministic stable bytes, equivalent structures, no bindings leaked into portable document |
| AC-02 | Unsupported/missing version, unknown/duplicate keys, invalid references, oversized input, unsafe schedule, numeric and field bounds | cargo test -p exmon-storage --locked; cargo test -p exmon-domain --locked | Bounded actionable errors, no panic or unsafe accepted document |

Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets --locked -- -D warnings, node scripts/check-architecture.mjs and git diff --check. Review source diff and preserve ignored target/.idea; no hardware/UI verification applies. Tests are local only.