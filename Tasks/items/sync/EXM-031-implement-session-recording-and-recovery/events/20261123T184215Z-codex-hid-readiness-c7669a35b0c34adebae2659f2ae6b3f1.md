---
schema: fdtasks-event/v3
event_id: EVT-9ded3e0ef5534badbc92f80bf2e9a56d
task_id: EXM-031
event_type: readiness-accepted
created_at: 2026-11-23T18:42:15Z
actor: Codex exmon sessions
owner: Codex exmon sessions
base_revision: f47aac5cd96f45e2d0227c972daa998bb93c3f40
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
task_sha256: 718712793277e77833fe8d9a01f23f1f6eb50191dcee1b63ce5f31183962d00d
atomicity:
  outcomes: [Bounded transactional local session recording and deterministic committed-data recovery]
  packets: [Versioned session store with its background producers and explicit desktop recording/recovery controls]
  independent_acceptance: false
  boundary_reason: Same031 recording/recovery contract must include the already implemented dedicated HID observation producer without relying on GUI drains
---

Preserve the initial031 readiness/AC matrix and resource/observer amendments. Integration review found the separate HID reports tool owns a worker/report queue outside the common mock-capture pipeline. Metadata roundtrip support alone is not evidence that this producer records. Complete the same accepted session contract by adding a narrow optional recorder sink in app/hid_worker, attached through GUI HidPanel/Shell; deliver observed RX bytes before HID GUI queue decimation. No HID source/adapter/discovery/open/send/permission or physical-device behavior changed. Session-local HID interface keys reserve the high bit plus bounded collection index, with epoch/generation and actual received sequence; common shell interface IDs stay in the low namespace. Recording must not discover/open/send HID by itself.

Extend allowed paths to app/hid_worker and its direct mock tests plus GUI hid recorder wiring/tests and header navigation. Sessions must be reachable while viewing an open HID tool; move its single navigation action from conditional status bar to the always-present header, retain recording/error visibility. No new domain/config schema/dependency or physical authority. Add real mock-HID worker and actual GUI record/open/receive/stop/recover assertions, zero implicit writes/opens, pre-display-queue recording and distinct source keys. Refresh affected HID/sessions/shell routing checks, inspect updated size/theme/DPI/native images. Same single031 closure target, no new task identity or partial Done. Physical HID qualification remains deferred to035/036.
