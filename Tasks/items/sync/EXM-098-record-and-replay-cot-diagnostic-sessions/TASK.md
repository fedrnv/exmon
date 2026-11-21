---
schema: fdtasks-spec/v3
id: EXM-098
title: "Record and replay correlated COT sessions with explicit payload consent"
type: Task
area: sync
component: record-and-replay-cot-diagnostic-sessions
epic_id: EXM-EPIC-D002
size: medium
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-096","EXM-031","EXM-032"]
created_at: 2026-11-21T22:36:31Z
created_by: "Fedor and Codex"
atomicity:
  outcomes: ["Record and replay correlated COT sessions with explicit payload consent"]
  packets: ["Deliver and verify record-and-replay-cot-diagnostic-sessions as one contract."]
  independent_acceptance: false
  boundary_reason: "One observable result with its direct consumers and negative checks."
---

# EXM-098 — Record and replay correlated COT sessions with explicit payload consent

## Goal and owner authorization
Record and replay correlated COT sessions with explicit payload consent. Requested by the owner on 2026-11-21: inspect one application's traffic, packet structure/signature, and its transformation before and after COT encryption. This publication authorizes planning only; it does not claim implementation or verification.

## Inputs and source baseline
ExMon main at d38cc06dcec07c7d41014da1396b883f376d0e7d; FDTasks 5.7 policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Read root AGENTS.md, DEVELOPMENT.md, Tasks/AGENTS.md, Tasks/FDTasks_5_7_ADOPTION.md and accepted ADR-0001/0002/0003. Revalidate source, prerequisites and policy at future dispatch. Reuse existing transport, reconstruction and session contracts instead of duplicating EXM-020/021/022/030/031/041/042.
Dependencies above must be complete before claim; prerequisite implementation determines concrete module paths, which must be bound in dispatch readiness.

## Scope
Extend the existing versioned session format with process identity, stage observations, lineage and capability/loss metadata. Payload collection and persistence are separate explicit opt-ins, off by default; clearly indicate potentially sensitive pre-encryption bytes. Offer metadata-only capture, bounded retention, selection/redaction export and deletion controls. Do not record keys/tokens; restrict local file access; stopping diagnostics stops collection. Replay reproduces correlation and inspection offline and never transmits. Preserve old session loading; reject malformed sessions safely.

## Allowed paths, boundaries and impact
Cargo manifests and Rust source/tests/fixtures implementing record-and-replay-cot-diagnostic-sessions, limited to the session storage/import/export boundary; this task's events/evidence. No unrelated refactoring, hosted workflows, drivers, deployment or third-party source edits.
Risk: high — process attribution, untrusted captures or potentially sensitive application bytes.
Keep capture, parsing and storage off the GUI thread, bound queues/retention, expose loss separately, and never transmit as a side effect of inspection or replay. Do not install drivers, elevate privileges, intercept TLS or collect cryptographic keys. Treat absence of plaintext/ownership/correlation evidence as unknown. A signature here is an observed protocol fingerprint, not a cryptographic signature.
No changes to COT are made by this planning publication; real COT hook implementation and its repository-specific authority are an explicit EXM-094 prerequisite.

## Failure and recovery
Cancellation/disconnection stops collection safely without altering application/COT traffic. Preserve accepted captures and distinguish loss, unsupported capability, malformed input and missing permission. Resume only after source/session identity revalidation; never silently join a reused PID, endpoint or trace identifier. Missing native host or upstream COT support blocks only its affected acceptance and cannot be reported as a pass.

## Ordered implementation
Publish a claim only after dependency and current-rule validation. Implement the cohesive result using temporary microsteps; run the checks below locally, inspect the diff and UI evidence when applicable, clean owned temporary data and publish exact immutable completion evidence. No hosted acceptance tests and no product implementation are part of creating this Planned record.

## Readiness, impact and acceptance matrix
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8. Direct boundaries: record-and-replay-cot-diagnostic-sessions, accepted prerequisite contracts and their callers.
The following are explicit manual local verification scenarios, not claims that a test binary already exists. At dispatch bind actual commands/toolchain/source and fixture seeds in readiness; missing infrastructure is a blocker.

| AC ID | Observable criterion | Test level / exact local steps | Fixture/environment | Expected result |
| --- | --- | --- | --- | --- |
| AC-01 | Recorded session replays the same lineage without transmission | focused integration/negative/UI as applicable: Opt in for synthetic payloads, record both directions, stop and reopen offline; compare stages/links with live trace | Synthetic payload real-COT session and legacy session | Same bytes and lineage where retained, legacy compatibility; zero network transmission during replay |
| AC-02 | Defaults and export respect selected retention/privacy policy | focused integration/negative/UI as applicable: Start without consent, enable metadata-only, redact export, exceed retention, delete and attempt malformed import | Canary nonsecret payload; privacy and corruption fixtures | No payload persisted without consent; redacted bytes absent from export; bounded storage and safe rejection; deletion documented |

## Completion evidence and cleanup
Record exact ExMon/COT revisions where applicable, policy hash, environment, commands/manual steps, fixture identities, pass/fail/skip/block outcomes, capability limits, inspected UI evidence and removal of owned synthetic captures. A future first verified closure gets one normal immutable version request. This accepted specification remains immutable; later changes use events.
