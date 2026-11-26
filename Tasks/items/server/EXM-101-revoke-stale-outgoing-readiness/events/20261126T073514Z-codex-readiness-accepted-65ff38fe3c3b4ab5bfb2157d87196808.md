---
schema: fdtasks-event/v3
event_id: EVT-65ff38fe3c3b4ab5bfb2157d87196808
task_id: EXM-101
event_type: readiness-accepted
created_at: 2026-11-26T07:35:14Z
actor: Codex exmon outgoing correction
base_revision: bb9dd6ee1aa8af20362f640bbdc1496e08e8a136
task_sha256: 2ddc30cd87d0626866df01f7c63c72acc0005a48512555640d31da10da075c10
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
atomicity:
  outcomes: [Outgoing readiness and terminal outcomes agree after interface revocation]
  packets: [Coherent TX authority observation with deterministic regression]
  independent_acceptance: false
  boundary_reason: Readiness, terminal publication and later Send admission share the same authority boundary
---

The published101 spec and EXM-027 Done dependency were read.035 owner is checkpointed pending this correction. Source confirms TX rejection is recorded before interface-loss revocation; Editor::ready uses cached state.tx. Verify both observation paths without weakening assertions. AC matrix and allowed paths from TASK remain binding. Add a controlled test that holds editor publication while underlying TX revokes, then verifies ready() rejects the cached ticket. Preserve true Busy behavior and all exact counters. Focused Windows suites, GUI outgoing consumers and actual isolated Linux outgoing/transmission/scheduling run are required before closure. Reuse035 toolcache and current Linux fixture under its existing resource bounds.
