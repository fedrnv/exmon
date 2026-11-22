---
schema: fdtasks-event/v3
event_id: EVT-a6e9cb9aeb4049e19330eccb0cd5fabc
task_id: EXM-017
event_type: blocked
created_at: 2026-11-22T18:50:22Z
actor: Codex exmon CAN development
owner: Codex exmon CAN development
base_revision: 3a195b7becf698651a2fa3d9522e15c04108c1cc
verification: Tasks/items/server/EXM-017-implement-can-capture-and-transmission/evidence/20261122T185022Z-can-progress.json
---

Safe classic CAN implementation, bounded in-process virtual bus tests and Linux SocketCAN/test cross-compilation delivered. Actual Linux vcan runtime is unavailable: read-only WSL query reports subsystem not installed. The exact opt-in native acceptance command and vcan-only guard are documented in docs/can.md. EXM-017 remains In Progress until that native virtual integration has run successfully; no Done or version request and no physical support claim.

Evidence records exact candidate/source hashes, commands, partial acceptance, failures corrected, scope, cleanup and owner action. Resume on externally prepared Linux/iproute2/vcan. Current global objective continues with EXM-025 in a fresh turn; EXM-008/014/016 owners and acceptance blockers unchanged. No hardware opened/transmitted, no driver/module/link configuration, elevation or permission changes.
