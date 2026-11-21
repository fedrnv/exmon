---
schema: fdtasks-spec/v3
id: EXM-087
title: "Attribute Linux network traffic to processes"
type: Task
area: server
component: linux-process-traffic-attribution
epic_id: EXM-EPIC-D001
size: medium
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-085","EXM-008","EXM-009"]
created_at: 2026-11-21T22:36:31Z
created_by: "Fedor and Codex"
atomicity:
  outcomes: ["Attribute Linux network traffic to processes"]
  packets: ["Deliver and verify linux-process-traffic-attribution as one contract."]
  independent_acceptance: false
  boundary_reason: "One observable result with its direct consumers and negative checks."
---

# EXM-087 — Attribute Linux network traffic to processes

## Goal and owner authorization
Attribute Linux network traffic to processes. Requested by the owner on 2026-11-21: inspect one application's traffic, packet structure/signature, and its transformation before and after COT encryption. This publication authorizes planning only; it does not claim implementation or verification.

## Inputs and source baseline
ExMon main at d38cc06dcec07c7d41014da1396b883f376d0e7d; FDTasks 5.7 policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Read root AGENTS.md, DEVELOPMENT.md, Tasks/AGENTS.md, Tasks/FDTasks_5_7_ADOPTION.md and accepted ADR-0001/0002/0003. Revalidate source, prerequisites and policy at future dispatch. Reuse existing transport, reconstruction and session contracts instead of duplicating EXM-020/021/022/030/031/041/042.
Dependencies above must be complete before claim; prerequisite implementation determines concrete module paths, which must be bound in dispatch readiness.

## Scope
Implement Linux process/socket attribution behind a capability-reported adapter using supported socket ownership/kernel facilities. Cover TCP/UDP, IPv4/IPv6, loopback, process namespaces and lifecycle; retain namespace identity and report container/permission blind spots. Any optional eBPF/event backend has an explicit privilege/capability path and no automatic installation or elevation.

## Allowed paths, boundaries and impact
Cargo manifests and Rust source/tests/fixtures implementing linux-process-traffic-attribution, limited to the capture/protocol/application boundary; this task's events/evidence. No unrelated refactoring, hosted workflows, drivers, deployment or third-party source edits.
Risk: high — process attribution, untrusted captures or potentially sensitive application bytes.
Keep capture, parsing and storage off the GUI thread, bound queues/retention, expose loss separately, and never transmit as a side effect of inspection or replay. Do not install drivers, elevate privileges, intercept TLS or collect cryptographic keys. Treat absence of plaintext/ownership/correlation evidence as unknown. A signature here is an observed protocol fingerprint, not a cryptographic signature.
No changes to COT are made by this planning publication; real COT hook implementation and its repository-specific authority are an explicit EXM-094 prerequisite.

## Failure and recovery
Cancellation/disconnection stops collection safely without altering application/COT traffic. Preserve accepted captures and distinguish loss, unsupported capability, malformed input and missing permission. Resume only after source/session identity revalidation; never silently join a reused PID, endpoint or trace identifier. Missing native host or upstream COT support blocks only its affected acceptance and cannot be reported as a pass.

## Ordered implementation
Publish a claim only after dependency and current-rule validation. Implement the cohesive result using temporary microsteps; run the checks below locally, inspect the diff and UI evidence when applicable, clean owned temporary data and publish exact immutable completion evidence. No hosted acceptance tests and no product implementation are part of creating this Planned record.

## Readiness, impact and acceptance matrix
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8. Direct boundaries: linux-process-traffic-attribution, accepted prerequisite contracts and their callers.
The following are explicit manual local verification scenarios, not claims that a test binary already exists. At dispatch bind actual commands/toolchain/source and fixture seeds in readiness; missing infrastructure is a blocker.

| AC ID | Observable criterion | Test level / exact local steps | Fixture/environment | Expected result |
| --- | --- | --- | --- | --- |
| AC-01 | Linux process filtering isolates controlled traffic | focused integration/negative/UI as applicable: Run two local TCP/UDP senders and filter one process; repeat IPv4/IPv6 and loopback | Native Linux host and controlled peers | Correct verified ownership and direction |
| AC-02 | Namespace, privilege and lifecycle ambiguity stays explicit | focused integration/negative/UI as applicable: Repeat in a separate network namespace where available, deny access and reuse PID/ports | Linux namespace and denial cases | No accidental host/container conflation; unavailable cases recorded blocked |

## Completion evidence and cleanup
Record exact ExMon/COT revisions where applicable, policy hash, environment, commands/manual steps, fixture identities, pass/fail/skip/block outcomes, capability limits, inspected UI evidence and removal of owned synthetic captures. A future first verified closure gets one normal immutable version request. This accepted specification remains immutable; later changes use events.
