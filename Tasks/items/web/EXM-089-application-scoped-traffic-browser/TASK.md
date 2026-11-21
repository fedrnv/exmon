---
schema: fdtasks-spec/v3
id: EXM-089
title: "Select an application and inspect its live traffic"
type: Task
area: web
component: application-scoped-traffic-browser
epic_id: EXM-EPIC-D001
size: medium
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-085","EXM-086","EXM-087","EXM-088","EXM-025","EXM-026"]
created_at: 2026-11-21T22:36:31Z
created_by: "Fedor and Codex"
atomicity:
  outcomes: ["Select an application and inspect its live traffic"]
  packets: ["Deliver and verify application-scoped-traffic-browser as one contract."]
  independent_acceptance: false
  boundary_reason: "One observable result with its direct consumers and negative checks."
---

# EXM-089 — Select an application and inspect its live traffic

## Goal and owner authorization
Select an application and inspect its live traffic. Requested by the owner on 2026-11-21: inspect one application's traffic, packet structure/signature, and its transformation before and after COT encryption. This publication authorizes planning only; it does not claim implementation or verification.

## Inputs and source baseline
ExMon main at d38cc06dcec07c7d41014da1396b883f376d0e7d; FDTasks 5.7 policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Read root AGENTS.md, DEVELOPMENT.md, Tasks/AGENTS.md, Tasks/FDTasks_5_7_ADOPTION.md and accepted ADR-0001/0002/0003. Revalidate source, prerequisites and policy at future dispatch. Reuse existing transport, reconstruction and session contracts instead of duplicating EXM-020/021/022/030/031/041/042.
Dependencies above must be complete before claim; prerequisite implementation determines concrete module paths, which must be bound in dispatch readiness.

## Scope
Add process search/selection by name, PID and executable, optional child processes, live connection list and process-scoped RX/TX capture. Show application identity, endpoint, direction, time, size and attribution provenance on each observation. Keep packet versus reassembled application-message boundaries explicit. Retain unknown/unattributed counters and expose permissions/capabilities; filters must not conceal capture loss. Use only the active platform adapter at runtime.

## Allowed paths, boundaries and impact
Cargo manifests and Rust source/tests/fixtures implementing application-scoped-traffic-browser, limited to the GUI/application and directly used model boundary; this task's events/evidence. No unrelated refactoring, hosted workflows, drivers, deployment or third-party source edits.
Risk: high — process attribution, untrusted captures or potentially sensitive application bytes.
Keep capture, parsing and storage off the GUI thread, bound queues/retention, expose loss separately, and never transmit as a side effect of inspection or replay. Do not install drivers, elevate privileges, intercept TLS or collect cryptographic keys. Treat absence of plaintext/ownership/correlation evidence as unknown. A signature here is an observed protocol fingerprint, not a cryptographic signature.
No changes to COT are made by this planning publication; real COT hook implementation and its repository-specific authority are an explicit EXM-094 prerequisite.

## Failure and recovery
Cancellation/disconnection stops collection safely without altering application/COT traffic. Preserve accepted captures and distinguish loss, unsupported capability, malformed input and missing permission. Resume only after source/session identity revalidation; never silently join a reused PID, endpoint or trace identifier. Missing native host or upstream COT support blocks only its affected acceptance and cannot be reported as a pass.

## Ordered implementation
Publish a claim only after dependency and current-rule validation. Implement the cohesive result using temporary microsteps; run the checks below locally, inspect the diff and UI evidence when applicable, clean owned temporary data and publish exact immutable completion evidence. No hosted acceptance tests and no product implementation are part of creating this Planned record.

## Readiness, impact and acceptance matrix
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8. Direct boundaries: application-scoped-traffic-browser, accepted prerequisite contracts and their callers.
The following are explicit manual local verification scenarios, not claims that a test binary already exists. At dispatch bind actual commands/toolchain/source and fixture seeds in readiness; missing infrastructure is a blocker.

| AC ID | Observable criterion | Test level / exact local steps | Fixture/environment | Expected result |
| --- | --- | --- | --- | --- |
| AC-01 | User can isolate one application without confusing COT outer traffic | focused integration/negative/UI as applicable: Start two senders, select one by name/PID, toggle child inclusion, open a connection and inspect RX/TX | Native platform plus deterministic attribution adapter | Selected traffic visible; other traffic excluded; originating app and outer owner distinct |
| AC-02 | UI remains usable and honest during exit, loss and unavailable attribution | focused integration/negative/UI as applicable: Exit selected process, restart with reused PID, inject drops and revoke access | UI at 1280x800 and 1920x1080, light/dark | No silent reassignment; visible unknown/loss/capability status; inspected screenshots |

## Completion evidence and cleanup
Record exact ExMon/COT revisions where applicable, policy hash, environment, commands/manual steps, fixture identities, pass/fail/skip/block outcomes, capability limits, inspected UI evidence and removal of owned synthetic captures. A future first verified closure gets one normal immutable version request. This accepted specification remains immutable; later changes use events.
