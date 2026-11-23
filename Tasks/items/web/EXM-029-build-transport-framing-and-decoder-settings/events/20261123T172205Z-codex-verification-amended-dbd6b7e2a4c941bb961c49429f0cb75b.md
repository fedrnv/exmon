---
schema: fdtasks-event/v3
event_id: EVT-f2948ac422c144f7b11b9f6785129feb
task_id: EXM-029
event_type: amended
created_at: 2026-11-23T17:22:05Z
actor: Codex exmon settings
owner: Codex exmon settings
base_revision: 3bcfce6d079c0b3399fdbe6c4302d4a9c665db55
---

Verification-boundary amendment: include only crates/app/tests/hid_worker.rs fixture snapshot helper as a direct selected regression correction. A repeated required app boundary run failed at reopen line124 with Busy. The unchanged HID API snapshot is intentionally optional/nonblocking; the fixture used unwrap_or_default(), so lock contention could fabricate open=false and acknowledge Close before the worker actually closed. Source inspection confirms inactive worker does not update snapshots after confirmed close. Preserve the original observed failure in ignored target/exm029-app-observed-hid-race.log, SHA256 9cf983c3088a82d6edf50f32805d8b4227287ee3f5e1e0bc12f70e41be3bc67e; it is not erased or accepted through an unchanged retry.

Replace only that test observation helper with a bounded2s wait for Some(actual snapshot), retaining all existing identities/permissions/epochs/write-count assertions and deadlines. No open/send command retry, no production HID/driver/adapter change or weaker assertion. This strengthens the existing direct consumer verification used by029 worker/configuration lifecycle acceptance; it adds no separate product outcome. Final acceptance requires the corrected fixture and all declared affected checks, exact failure/correction evidence, current source hashes and normal publication. GoalACTIVE and029 remains In Progress; no new task identity or first-closure request here.
