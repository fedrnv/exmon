---
schema: fdtasks-spec/v3
id: EXM-093
title: "Define COT transformation-stage diagnostic contract"
type: Task
area: foundation
component: cot-diagnostic-observation-contract
epic_id: EXM-EPIC-D002
size: medium
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-085"]
created_at: 2026-11-21T22:36:31Z
created_by: "Fedor and Codex"
atomicity:
  outcomes: ["Define COT transformation-stage diagnostic contract"]
  packets: ["Deliver and verify cot-diagnostic-observation-contract as one contract."]
  independent_acceptance: false
  boundary_reason: "One observable result with its direct consumers and negative checks."
---

# EXM-093 — Define COT transformation-stage diagnostic contract

## Goal and owner authorization
Define COT transformation-stage diagnostic contract. Requested by the owner on 2026-11-21: inspect one application's traffic, packet structure/signature, and its transformation before and after COT encryption. This publication authorizes planning only; it does not claim implementation or verification.

## Inputs and source baseline
ExMon main at d38cc06dcec07c7d41014da1396b883f376d0e7d; FDTasks 5.7 policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Read root AGENTS.md, DEVELOPMENT.md, Tasks/AGENTS.md, Tasks/FDTasks_5_7_ADOPTION.md and accepted ADR-0001/0002/0003. Revalidate source, prerequisites and policy at future dispatch. Reuse existing transport, reconstruction and session contracts instead of duplicating EXM-020/021/022/030/031/041/042.
Dependencies above must be complete before claim; prerequisite implementation determines concrete module paths, which must be bound in dispatch readiness.

## Scope
Specify a versioned opt-in diagnostic API usable by ExMon and COT. Events include session/host/process identity, flow/message/fragment identifiers, direction, stage, monotonic timestamp plus clock-domain identity, sequence, byte length, optional bounded bytes, capability flags and drop counts. Stages cover COT ingress (before its transforms), after compression/packing, after encryption, after obfuscation/fragmentation and socket egress; RX has the inverse stages. A COT implementation without a transform explicitly reports not-applicable. Preserve input-to-output span links for batching/splitting; bytes passed into COT may already be TLS ciphertext. Metadata carries no keys and never alters wire payloads.

## Allowed paths, boundaries and impact
Cargo manifests and Rust source/tests/fixtures implementing cot-diagnostic-observation-contract, limited to the domain/schema boundary; this task's events/evidence. No unrelated refactoring, hosted workflows, drivers, deployment or third-party source edits.
Risk: high — process attribution, untrusted captures or potentially sensitive application bytes.
Keep capture, parsing and storage off the GUI thread, bound queues/retention, expose loss separately, and never transmit as a side effect of inspection or replay. Do not install drivers, elevate privileges, intercept TLS or collect cryptographic keys. Treat absence of plaintext/ownership/correlation evidence as unknown. A signature here is an observed protocol fingerprint, not a cryptographic signature.
No changes to COT are made by this planning publication; real COT hook implementation and its repository-specific authority are an explicit EXM-094 prerequisite.

## Failure and recovery
Cancellation/disconnection stops collection safely without altering application/COT traffic. Preserve accepted captures and distinguish loss, unsupported capability, malformed input and missing permission. Resume only after source/session identity revalidation; never silently join a reused PID, endpoint or trace identifier. Missing native host or upstream COT support blocks only its affected acceptance and cannot be reported as a pass.

## Ordered implementation
Publish a claim only after dependency and current-rule validation. Implement the cohesive result using temporary microsteps; run the checks below locally, inspect the diff and UI evidence when applicable, clean owned temporary data and publish exact immutable completion evidence. No hosted acceptance tests and no product implementation are part of creating this Planned record.

## Readiness, impact and acceptance matrix
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8. Direct boundaries: cot-diagnostic-observation-contract, accepted prerequisite contracts and their callers.
The following are explicit manual local verification scenarios, not claims that a test binary already exists. At dispatch bind actual commands/toolchain/source and fixture seeds in readiness; missing infrastructure is a blocker.

| AC ID | Observable criterion | Test level / exact local steps | Fixture/environment | Expected result |
| --- | --- | --- | --- | --- |
| AC-01 | Contract represents full forward/reverse transforms and many-to-many byte lineage | focused integration/negative/UI as applicable: Validate good event fixtures for TCP and UDP, split/batch/reorder and inverse RX path | Versioned synthetic contract fixtures | Stages and identity resolve without assuming one message equals one packet |
| AC-02 | Invalid, old and missing-capability producers fail predictably | focused integration/negative/UI as applicable: Validate unsupported versions, excessive bytes, absent stages, clock domains and drops | Negative contract fixtures | Bounded rejection or documented compatibility; unavailable stages explicit |

## Completion evidence and cleanup
Record exact ExMon/COT revisions where applicable, policy hash, environment, commands/manual steps, fixture identities, pass/fail/skip/block outcomes, capability limits, inspected UI evidence and removal of owned synthetic captures. A future first verified closure gets one normal immutable version request. This accepted specification remains immutable; later changes use events.
