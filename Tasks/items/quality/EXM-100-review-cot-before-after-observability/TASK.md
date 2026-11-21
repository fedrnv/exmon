---
schema: fdtasks-spec/v3
id: EXM-100
title: "Independently review COT traffic observability"
type: Task
area: quality
component: review-cot-before-after-observability
epic_id: EXM-EPIC-D002
size: medium
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-099"]
created_at: 2026-11-21T22:36:31Z
created_by: "Fedor and Codex"
atomicity:
  outcomes: ["Independently review COT traffic observability"]
  packets: ["Deliver and verify review-cot-before-after-observability as one contract."]
  independent_acceptance: false
  boundary_reason: "One observable result with its direct consumers and negative checks."
---

# EXM-100 — Independently review COT traffic observability

## Goal and owner authorization
Independently review COT traffic observability. Requested by the owner on 2026-11-21: inspect one application's traffic, packet structure/signature, and its transformation before and after COT encryption. This publication authorizes planning only; it does not claim implementation or verification.

## Inputs and source baseline
ExMon main at d38cc06dcec07c7d41014da1396b883f376d0e7d; FDTasks 5.7 policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Read root AGENTS.md, DEVELOPMENT.md, Tasks/AGENTS.md, Tasks/FDTasks_5_7_ADOPTION.md and accepted ADR-0001/0002/0003. Revalidate source, prerequisites and policy at future dispatch. Reuse existing transport, reconstruction and session contracts instead of duplicating EXM-020/021/022/030/031/041/042.
Dependencies above must be complete before claim; prerequisite implementation determines concrete module paths, which must be bound in dispatch readiness.

## Scope
Perform a separate independent Level-0 review of the exact source candidate and current passing targeted gate. Reviewer must be a different person or fresh independent model session and disclose the independence boundary. Inspect only this Epic's changed behavior, direct contracts, uncertainty/capability reporting, privacy, bounds and observed UI. Publish immutable REQUEST.md and RESULT.md under this Epic's reviews directory. Findings become linked medium corrections; changed candidate reopens gate/review through events, preserving previous rounds.

## Allowed paths, boundaries and impact
Tasks/epics/ for this Epic's immutable review records; this task's events/evidence and directly necessary local verification fixtures. Product corrections require separate items.
Risk: medium — acceptance evidence and candidate identity.
Keep capture, parsing and storage off the GUI thread, bound queues/retention, expose loss separately, and never transmit as a side effect of inspection or replay. Do not install drivers, elevate privileges, intercept TLS or collect cryptographic keys. Treat absence of plaintext/ownership/correlation evidence as unknown. A signature here is an observed protocol fingerprint, not a cryptographic signature.
No changes to COT are made by this planning publication; real COT hook implementation and its repository-specific authority are an explicit EXM-094 prerequisite.

## Failure and recovery
Cancellation/disconnection stops collection safely without altering application/COT traffic. Preserve accepted captures and distinguish loss, unsupported capability, malformed input and missing permission. Resume only after source/session identity revalidation; never silently join a reused PID, endpoint or trace identifier. Missing native host or upstream COT support blocks only its affected acceptance and cannot be reported as a pass.

## Ordered implementation
Publish a claim only after dependency and current-rule validation. Implement the cohesive result using temporary microsteps; run the checks below locally, inspect the diff and UI evidence when applicable, clean owned temporary data and publish exact immutable completion evidence. No hosted acceptance tests and no product implementation are part of creating this Planned record.

## Readiness, impact and acceptance matrix
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8. Direct boundaries: review-cot-before-after-observability, accepted prerequisite contracts and their callers.
The following are explicit manual local verification scenarios, not claims that a test binary already exists. At dispatch bind actual commands/toolchain/source and fixture seeds in readiness; missing infrastructure is a blocker.

| AC ID | Observable criterion | Test level / exact local steps | Fixture/environment | Expected result |
| --- | --- | --- | --- | --- |
| AC-01 | Review covers the tested source and has genuine independence | local gate/review: Match source hashes and gate evidence, disclose reviewer independence, inspect changes and recorded UI evidence | Current passing gate and frozen candidate | Accepted result only with all blocking findings resolved; request alone is not acceptance |
| AC-02 | Changed candidate cannot reuse stale acceptance | local gate/review: Check candidate identity before closure; if changed or correction added, append reopen events and require new gate/review round | Review manifest and immutable history | No stale acceptance or overwritten review history |

## Completion evidence and cleanup
Record exact ExMon/COT revisions where applicable, policy hash, environment, commands/manual steps, fixture identities, pass/fail/skip/block outcomes, capability limits, inspected UI evidence and removal of owned synthetic captures. A future first verified closure gets one normal immutable version request. This accepted specification remains immutable; later changes use events.
