---
schema: fdtasks-event/v3
event_id: EVT-9c867a0e2f1c7a012afc1bc5dfcfe3ef
task_id: EXM-014
event_type: blocked
created_at: 2026-11-22T20:40:58Z
actor: Codex exmon HID development
owner: Codex exmon HID development
base_revision: 4ce663bd36adab4c1734765acda2b419c7eaf73c
verification: Tasks/items/server/EXM-014-implement-usb-hid-support/evidence/20261122T203745Z-hid-linux-progress.json
---

Actual Linux HIDAPI access to the isolated QEMU USB keyboard passed: exact unnumbered input/metadata, explicit output independently verified by usbmon SET_REPORT bytes and matching successful callback, actual unprivileged permission denial, cancellation/close and emulated removal with revoked output. Focused capture/domain transport regressions, workspace format/lint, native Windows/Linux lint and architecture/diff checks passed. Exact source/binary/image/output and manual harness, review, limits and cleanup are recorded. No physical device opened or sent to; emulation never substitutes for approved physical acceptance.

EXM-014 remains In Progress without Done/version request: approved physical HID collection, descriptor, known reports and selected read/output authorization remain unavailable. Owner supplies those for physical qualification. EXM-008 still requires authorized Windows/Npcap and macOS verification environments; Linux evidence is published. Providing those environments and closing EXM-008 first unblocks EXM-009/011.

Full remaining task projection:13Done,85Planned,2In Progress. Only EXM-043/085 are task-dependency-ready but belong to future Epics with unmet accepted Epic prerequisites. No remaining ready Epic001 task after final native virtual checks. No dependency/scope exception invented. Global objective remains active; this is concrete progress and first end-of-turn global external-blocker audit. Next fresh turn revalidates authoritative state/external inputs; after3 consecutive same genuine global-blocker turns, stop the goal as blocked rather than polling caches or manufacturing work. Durable checkpoint includes exact evidence hashes, next action and resource/authority status.
