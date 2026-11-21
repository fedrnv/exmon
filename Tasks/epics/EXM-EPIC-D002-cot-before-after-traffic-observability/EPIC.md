---
schema: fdtasks-epic/v4
id: EXM-EPIC-D002
title: "COT before-and-after traffic observability"
initial_status: Planned
initial_dependencies: ["EXM-EPIC-D001"]
created_at: 2026-11-21T22:36:31Z
created_by: "Fedor and Codex"
---

# EXM-EPIC-D002 — COT before-and-after traffic observability

## Outcome
Follow one selected application's bidirectional traffic through COT input, encryption, obfuscation/fragmentation and actual network packets in synchronized ExMon panels, then record/replay an explicitly consented session. Require real COT hooks and retain honest many-to-many lineage, drop/capability status and ciphertext limits.

## Owner request and boundary
Owner requested these tasks on 2026-11-21. This is accepted future planning only, not activation, a development claim or permission to start unrelated implementation. Existing accepted tasks and numeric Epic identities remain unchanged. Normal one-active-development-Epic scheduling applies.

## Accepted membership
- EXM-093: Define COT transformation-stage diagnostic contract
- EXM-094: Qualify real COT hooks before and after encryption
- EXM-095: Ingest authorized COT diagnostic events into ExMon
- EXM-096: Correlate COT messages with transformed bytes and wire packets
- EXM-097: Compare selected application traffic before COT and on the wire
- EXM-098: Record and replay correlated COT sessions with explicit payload consent
- EXM-099: Prove real COT before-and-after traffic observability
- EXM-100: Independently review COT traffic observability

## Dependencies and integration
ExMon-side adapter/harness work is mockable, but EXM-094 must qualify a real COT producer and record its exact upstream implementation task/commit. Missing upstream instrumentation blocks real integration; no mock-only completion. Direct-mode and COT-mode comparisons use separate named sessions. Plaintext before COT may itself already be TLS ciphertext.

## Gate and review
FDTasks 5.7 policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Exactly six functionality items, integration gate EXM-099 and independent Level-0 review EXM-100.
Gate depends on all six implementation items; review depends only on the gate.
Run the targeted changed-area/direct-interaction gate locally on a frozen candidate. New findings/candidate changes reopen gate/review by append-only events. Epic closure requires every member Done, current passing gate, independent Accepted review and no blocking findings. Missing native/upstream evidence stays blocked.

## Practical acceptance journey
Select an application; inspect normal traffic; enable COT diagnostics and send a known message; click it and follow all corresponding encrypted/obfuscated fragments and real packets; follow a reply in reverse. Observe bytes, structure/signature where available, sizes, overhead and supported timing. Repeat with already-TLS-encrypted input, missing stages, fragmentation and reconnect; save with consent and reopen offline without transmission.
