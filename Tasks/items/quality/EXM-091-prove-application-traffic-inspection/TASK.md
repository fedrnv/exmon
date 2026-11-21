---
schema: fdtasks-spec/v3
id: EXM-091
title: "Prove application traffic and signature inspection"
type: Task
area: quality
component: prove-application-traffic-inspection
epic_id: EXM-EPIC-D001
size: medium
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-085","EXM-086","EXM-087","EXM-088","EXM-089","EXM-090"]
created_at: 2026-11-21T22:36:31Z
created_by: "Fedor and Codex"
atomicity:
  outcomes: ["Prove application traffic and signature inspection"]
  packets: ["Deliver and verify prove-application-traffic-inspection as one contract."]
  independent_acceptance: false
  boundary_reason: "One observable result with its direct consumers and negative checks."
---

# EXM-091 — Prove application traffic and signature inspection

## Goal and owner authorization
Prove application traffic and signature inspection. Requested by the owner on 2026-11-21: inspect one application's traffic, packet structure/signature, and its transformation before and after COT encryption. This publication authorizes planning only; it does not claim implementation or verification.

## Inputs and source baseline
ExMon main at d38cc06dcec07c7d41014da1396b883f376d0e7d; FDTasks 5.7 policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Read root AGENTS.md, DEVELOPMENT.md, Tasks/AGENTS.md, Tasks/FDTasks_5_7_ADOPTION.md and accepted ADR-0001/0002/0003. Revalidate source, prerequisites and policy at future dispatch. Reuse existing transport, reconstruction and session contracts instead of duplicating EXM-020/021/022/030/031/041/042.
Dependencies above must be complete before claim; prerequisite implementation determines concrete module paths, which must be bound in dispatch readiness.

## Scope
Run one targeted local integration gate on a frozen ExMon source covering only process attribution, process-scoped capture and packet/signature inspection plus their direct capture/reconstruction/session boundaries. Validate Windows, Linux and macOS capabilities on native hosts. Include two processes, IPv4/IPv6 TCP/UDP, loopback, PID/port reuse, unknown ownership and malformed/encrypted packets. Reuse prior exact-input evidence where valid; unavailable native checks are blocked, not passed.

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
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8. Direct boundaries: prove-application-traffic-inspection, accepted prerequisite contracts and their callers.
The following are explicit manual local verification scenarios, not claims that a test binary already exists. At dispatch bind actual commands/toolchain/source and fixture seeds in readiness; missing infrastructure is a blocker.

| AC ID | Observable criterion | Test level / exact local steps | Fixture/environment | Expected result |
| --- | --- | --- | --- | --- |
| AC-01 | Current integrated outcome passes on the frozen candidate | local gate/review: Perform every native happy/negative scenario in the scope; bind local results and UI screenshots to source and policy hashes | Native Windows/Linux/macOS controlled senders | All applicable cases pass; unsupported capabilities and blocked environments explicit; no unrelated broad gate |
| AC-02 | Gate evidence is reproducible and bounded | local gate/review: Record commands/manual steps, toolchains, fixtures, resource limits, outcomes and cleanup; stop on functional failure | One frozen candidate; at most 10 minutes per fixture, 1 GiB test capture limit | Hash-bound local evidence; failed/skipped checks never presented as pass |

## Completion evidence and cleanup
Record exact ExMon/COT revisions where applicable, policy hash, environment, commands/manual steps, fixture identities, pass/fail/skip/block outcomes, capability limits, inspected UI evidence and removal of owned synthetic captures. A future first verified closure gets one normal immutable version request. This accepted specification remains immutable; later changes use events.
