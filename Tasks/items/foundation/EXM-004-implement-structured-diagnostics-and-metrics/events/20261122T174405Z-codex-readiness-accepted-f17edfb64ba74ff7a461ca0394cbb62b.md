---
schema: fdtasks-event/v3
event_id: EVT-ef1704ce-d677-4ed8-8507-6a0e89a15389
task_id: EXM-004
event_type: readiness-accepted
created_at: 2026-11-22T17:44:05Z
actor: Codex exmon diagnostics development
base_revision: 4ba0d3b663a7b918ab6d74fd9057d53a00919ce7
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
task_sha256: 0ab77d6698c092442f9a5410dba4f134e2a327380782decd5eb056776317f1ca
atomicity:
  outcomes: [Nonblocking capture metrics with redacted background diagnostics and GUI-readable snapshots]
  packets: [Implement shared atomic metrics and their safe diagnostic consumer]
  independent_acceptance: false
  boundary_reason: Producers and consumer implement one observable health contract
---

Dependency EXM-002 published Done at e5b79fe; current source 4ba0d3b663a7b918ab6d74fd9057d53a00919ce7 includes EXM-003 published Done. EXM-004 Planned and unowned; medium in EXM-EPIC-001 under accepted migration. No other development Epic active. Rules: root AGENTS.md/DEVELOPMENT.md, Tasks/AGENTS.md, FDTasks 5.7 adoption and ADR-0001/2/3; no deeper rules.

Allowed: crates/domain/src/metrics.rs and export, crates/app/src/diagnostics.rs and export/tests, docs/diagnostics.md, new task evidence/events and version request. No capture adapter, UI rendering or scheduler implementation. Metrics live below app to permit capture/protocol/storage producers without reverse dependencies. Fixed atomic counters and gauges, saturating totals, no locks/allocations/logging/I/O in update paths; fixed-size snapshots and rates consumed by GUI/background workers. Logging takes snapshots and redacts borrowed payload/device context by default with no opt-out. Unknown backend loss count remains distinct. Risk: log sinks can block; emit structured logs only from background consumer, never counter updates. Atomic snapshots are approximate across fields, not transactions; no cross-epoch rates. Fixed state bounds memory, overflow saturates visibly. Cleanup: reuse ignored target, preserve .idea, no sensitive fixtures.

| AC | Scenario/level | Local command on Windows | Expected |
| --- | --- | --- | --- |
| AC-01 | Atomic producer source inspection, concurrent updates, blocked sink isolation, snapshot/rate boundaries | cargo test -p exmon-domain --locked; cargo test -p exmon-app --locked | Exact joined totals, no wrap, independent producer progress despite blocked logger, valid per-source rates |
| AC-02 | Separate stage counters, gauges, fault/health and captured tracing output with synthetic secret context | cargo test -p exmon-domain --locked; cargo test -p exmon-app --locked | Numeric structured fields present; payload and device strings absent from Debug/Display/logging |

Run cargo fmt --all -- --check, cargo clippy --workspace --all-targets --locked -- -D warnings, node scripts/check-architecture.mjs and git diff --check. No transport tests exist; unchanged configuration tests are not automatically selected. No hardware/UI claims. Author diff review and exact evidence before Done.

Fresh validation after rejected unpublished claim 31f85a1 (preserved in refs/codex/rejected-claims/EXM-004-31f85a1). Remote changed only allocated version records; task, owner, dependencies, rules and source unchanged. No implementation started. This is a fresh claim against the current published base, not replay of the rejected claim.
