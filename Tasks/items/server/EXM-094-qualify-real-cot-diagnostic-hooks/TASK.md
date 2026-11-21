---
schema: fdtasks-spec/v3
id: EXM-094
title: "Qualify real COT hooks before and after encryption"
type: Task
area: server
component: qualify-real-cot-diagnostic-hooks
epic_id: EXM-EPIC-D002
size: medium
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-093"]
created_at: 2026-11-21T22:36:31Z
created_by: "Fedor and Codex"
atomicity:
  outcomes: ["Qualify real COT hooks before and after encryption"]
  packets: ["Deliver and verify qualify-real-cot-diagnostic-hooks as one contract."]
  independent_acceptance: false
  boundary_reason: "One observable result with its direct consumers and negative checks."
---

# EXM-094 — Qualify real COT hooks before and after encryption

## Goal and owner authorization
Qualify real COT hooks before and after encryption. Requested by the owner on 2026-11-21: inspect one application's traffic, packet structure/signature, and its transformation before and after COT encryption. This publication authorizes planning only; it does not claim implementation or verification.

## Inputs and source baseline
ExMon main at d38cc06dcec07c7d41014da1396b883f376d0e7d; FDTasks 5.7 policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Read root AGENTS.md, DEVELOPMENT.md, Tasks/AGENTS.md, Tasks/FDTasks_5_7_ADOPTION.md and accepted ADR-0001/0002/0003. Revalidate source, prerequisites and policy at future dispatch. Reuse existing transport, reconstruction and session contracts instead of duplicating EXM-020/021/022/030/031/041/042.
Dependencies above must be complete before claim; prerequisite implementation determines concrete module paths, which must be bound in dispatch readiness.

## Scope
Own the ExMon-side COT producer conformance harness and upstream integration handoff. Require an actual instrumented COT build with TX/RX hooks before transforms, after encryption, after obfuscation/fragmentation and at the socket boundary, preserving diagnostic-only lineage IDs across batching and fragments. The COT implementation must expose opt-in local diagnostics, bounded nonblocking queues and explicit drops; it must not export keys or change wire bytes for tracing. Track the required COT implementation as an external prerequisite with exact repository commit/task and supported schema before closure. Work is recorded in ExMon; no COT source change is authorized by this planning commit. If upstream hooks are absent, record a blocker and keep this task and the final real-integration gate open; a mock is not producer qualification.

## Allowed paths, boundaries and impact
Cargo manifests and Rust source/tests/fixtures implementing qualify-real-cot-diagnostic-hooks, limited to the capture/protocol/application boundary; this task's events/evidence. No unrelated refactoring, hosted workflows, drivers, deployment or third-party source edits.
Risk: high — process attribution, untrusted captures or potentially sensitive application bytes.
Keep capture, parsing and storage off the GUI thread, bound queues/retention, expose loss separately, and never transmit as a side effect of inspection or replay. Do not install drivers, elevate privileges, intercept TLS or collect cryptographic keys. Treat absence of plaintext/ownership/correlation evidence as unknown. A signature here is an observed protocol fingerprint, not a cryptographic signature.
No changes to COT are made by this planning publication; real COT hook implementation and its repository-specific authority are an explicit EXM-094 prerequisite.

## Failure and recovery
Cancellation/disconnection stops collection safely without altering application/COT traffic. Preserve accepted captures and distinguish loss, unsupported capability, malformed input and missing permission. Resume only after source/session identity revalidation; never silently join a reused PID, endpoint or trace identifier. Missing native host or upstream COT support blocks only its affected acceptance and cannot be reported as a pass.

## Ordered implementation
Publish a claim only after dependency and current-rule validation. Implement the cohesive result using temporary microsteps; run the checks below locally, inspect the diff and UI evidence when applicable, clean owned temporary data and publish exact immutable completion evidence. No hosted acceptance tests and no product implementation are part of creating this Planned record.

## Readiness, impact and acceptance matrix
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8. Direct boundaries: qualify-real-cot-diagnostic-hooks, accepted prerequisite contracts and their callers.
The following are explicit manual local verification scenarios, not claims that a test binary already exists. At dispatch bind actual commands/toolchain/source and fixture seeds in readiness; missing infrastructure is a blocker.

| AC ID | Observable criterion | Test level / exact local steps | Fixture/environment | Expected result |
| --- | --- | --- | --- | --- |
| AC-01 | Real COT emits verifiable before/after bytes and provenance | focused integration/negative/UI as applicable: Build an upstream instrumented COT candidate under its repository authority; send known binary payload both directions through encryption/obfuscation and compare hook bytes to socket boundary | Exact COT commit; local client/server; deterministic payload | All supported stages and lineage verified; non-applicable transforms declared |
| AC-02 | Disabled diagnostics preserve behavior and enabled diagnostics cannot block transport | focused integration/negative/UI as applicable: Compare tracing off/on delivery; overflow/disconnect diagnostic consumer; inspect serialized events for keys | Real COT localhost pair; synthetic nonsecret payload | Same delivered payload and protocol behavior; bounded overhead/queues, explicit drops, no keys; exact COT reference recorded |

## Completion evidence and cleanup
Record exact ExMon/COT revisions where applicable, policy hash, environment, commands/manual steps, fixture identities, pass/fail/skip/block outcomes, capability limits, inspected UI evidence and removal of owned synthetic captures. A future first verified closure gets one normal immutable version request. This accepted specification remains immutable; later changes use events.
