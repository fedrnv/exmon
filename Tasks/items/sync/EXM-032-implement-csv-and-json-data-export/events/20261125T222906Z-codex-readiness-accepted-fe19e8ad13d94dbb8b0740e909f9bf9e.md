---
schema: fdtasks-event/v3
task_id: EXM-032
created_at: 2026-11-25T22:29:06Z
actor: Codex exmon export
base_revision: b88634d43032f869760c1e576deba975032c900a
task_sha256: 16181f4f70228a2612b7d3c0b9e56ad8b3afa0e60a4234ad7cf3f4deeef4d536
event_id: EVT-fe19e8ad13d94dbb8b0740e909f9bf9e
event_type: readiness-accepted
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
atomicity:
  outcomes: [Selectable bounded export of committed session data for external analysis]
  packets: [Shared streaming CSV and JSON projection with its cancellable Sessions action]
  independent_acceptance: false
  boundary_reason: Both encodings and the desktop action share the same selection, committed-record ordering and publication guarantees
---

EXM-021 and EXM-031 are published Done. EXM-EPIC-001 legacy membership and the owner Windows/virtual exception remain applicable. Revalidated root/Tasks instructions, FDTasks adoption, README/DEVELOPMENT, ADRs 0001–0003 and session/privacy contracts at the published base. Manual Git workflow; native activation remains false.

Use the existing stable recovered-session transaction. Stream time-bounded selection by record category, exact transport/protocol type and decoded field ID as applicable. Output versioned raw observations, decoded field values and recorded chart-selected engineering series in deterministic session order, with explicit ns clocks, units, source identity, and original loss/closure status. Preserve null/unavailable values. Older recording data remains readable. Chart export uses saved selections and what the session actually recorded, without guessing missing values or deriving historical GUI state.

One explicit local export action for a recovered session, with raw-content consent by selection and visible privacy guidance. Keep paging/serialization/storage off the UI thread. Bounded page/row/writer, cancellation between pages and rows and before publication; staged local file, no overwrite, clear error on unsupported atomic publication or I/O failure. No capture, settings apply, TX, external upload, or dependencies added. Allowed paths: storage export/session projection and focused tests; app session worker/producer and direct tests; Sessions UI and native/interaction tests; docs/sessions.md; new EXM-032 events/evidence/first request. No transport, protocol, allocator, CI or accepted-history edits.

AC-01 -> cargo test -p exmon-storage --test export --test sessions --locked. Synthetic independent CSV/JSON expected records, time/type/field filters, clocks, safe text including formula prefixes, endian-independent decoded values, chart updates, loss and older record compatibility. Exact checks, no numerical tolerance for fixture values.
AC-02 -> same suites plus cargo test -p exmon-app --test sessions --locked and cargo test -p exmon-gui --test sessions --locked -- --test-threads=1. Deterministic multi-page traversal, cancellation including partial progress, existing destination and failure cleanup, real Sessions interaction and inspected synthetic small/large dark/light DPI images. Relevant Windows regressions, workspace format/Clippy, architecture and diff review. Record exact source/tool/commands/results and limitations; no stand or packaged release qualification.\n