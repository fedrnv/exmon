---
schema: fdtasks-spec/v3
id: EXM-090
title: "Inspect inferred packet structure and reusable signatures"
type: Task
area: web
component: packet-structure-and-signature-inspector
epic_id: EXM-EPIC-D001
size: medium
initial_priority: P1
initial_status: Planned
initial_dependencies: ["EXM-089","EXM-020","EXM-021","EXM-022","EXM-030","EXM-041","EXM-042"]
created_at: 2026-11-21T22:36:31Z
created_by: "Fedor and Codex"
atomicity:
  outcomes: ["Inspect inferred packet structure and reusable signatures"]
  packets: ["Deliver and verify packet-structure-and-signature-inspector as one contract."]
  independent_acceptance: false
  boundary_reason: "One observable result with its direct consumers and negative checks."
---

# EXM-090 — Inspect inferred packet structure and reusable signatures

## Goal and owner authorization
Inspect inferred packet structure and reusable signatures. Requested by the owner on 2026-11-21: inspect one application's traffic, packet structure/signature, and its transformation before and after COT encryption. This publication authorizes planning only; it does not claim implementation or verification.

## Inputs and source baseline
ExMon main at d38cc06dcec07c7d41014da1396b883f376d0e7d; FDTasks 5.7 policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Read root AGENTS.md, DEVELOPMENT.md, Tasks/AGENTS.md, Tasks/FDTasks_5_7_ADOPTION.md and accepted ADR-0001/0002/0003. Revalidate source, prerequisites and policy at future dispatch. Reuse existing transport, reconstruction and session contracts instead of duplicating EXM-020/021/022/030/031/041/042.
Dependencies above must be complete before claim; prerequisite implementation determines concrete module paths, which must be bound in dispatch readiness.

## Scope
Extend existing reconstruction and protocol inspection rather than duplicate it. On selected messages show byte ranges, field names/types/endianness, constants, lengths, payload and supported checksums; clicking a field highlights HEX/ASCII bytes and vice versa. Infer a masked header/constant-byte signature with variable positions, framing/length constraints, sample count, confidence and evidence links; user can accept/edit/name, save and apply it to later traffic. Separate observed protocol fingerprint from cryptographic signature. Ciphertext stays opaque unless an authorized earlier-stage observation supplies structure.

## Allowed paths, boundaries and impact
Cargo manifests and Rust source/tests/fixtures implementing packet-structure-and-signature-inspector, limited to the GUI/application and directly used model boundary; this task's events/evidence. No unrelated refactoring, hosted workflows, drivers, deployment or third-party source edits.
Risk: high — process attribution, untrusted captures or potentially sensitive application bytes.
Keep capture, parsing and storage off the GUI thread, bound queues/retention, expose loss separately, and never transmit as a side effect of inspection or replay. Do not install drivers, elevate privileges, intercept TLS or collect cryptographic keys. Treat absence of plaintext/ownership/correlation evidence as unknown. A signature here is an observed protocol fingerprint, not a cryptographic signature.
No changes to COT are made by this planning publication; real COT hook implementation and its repository-specific authority are an explicit EXM-094 prerequisite.

## Failure and recovery
Cancellation/disconnection stops collection safely without altering application/COT traffic. Preserve accepted captures and distinguish loss, unsupported capability, malformed input and missing permission. Resume only after source/session identity revalidation; never silently join a reused PID, endpoint or trace identifier. Missing native host or upstream COT support blocks only its affected acceptance and cannot be reported as a pass.

## Ordered implementation
Publish a claim only after dependency and current-rule validation. Implement the cohesive result using temporary microsteps; run the checks below locally, inspect the diff and UI evidence when applicable, clean owned temporary data and publish exact immutable completion evidence. No hosted acceptance tests and no product implementation are part of creating this Planned record.

## Readiness, impact and acceptance matrix
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8. Direct boundaries: packet-structure-and-signature-inspector, accepted prerequisite contracts and their callers.
The following are explicit manual local verification scenarios, not claims that a test binary already exists. At dispatch bind actual commands/toolchain/source and fixture seeds in readiness; missing infrastructure is a blocker.

| AC ID | Observable criterion | Test level / exact local steps | Fixture/environment | Expected result |
| --- | --- | --- | --- | --- |
| AC-01 | Known protocol yields inspectable fields and an applicable signature | focused integration/negative/UI as applicable: Analyze a deterministic corpus, click fields/bytes, edit/save signature and classify unseen positive/negative samples | Known protocol with variable counters, lengths and checksum | Correct ranges and masked bytes; valid samples match, near misses rejected |
| AC-02 | Ambiguous, encrypted and malformed data does not produce invented structure | focused integration/negative/UI as applicable: Analyze conflicting/truncated samples and randomized ciphertext; reopen saved definition | Ambiguity, malformed lengths and ciphertext corpus; native UI | Confidence/evidence visible; unresolved fields explicit; no decryption claim; screenshots inspected |

## Completion evidence and cleanup
Record exact ExMon/COT revisions where applicable, policy hash, environment, commands/manual steps, fixture identities, pass/fail/skip/block outcomes, capability limits, inspected UI evidence and removal of owned synthetic captures. A future first verified closure gets one normal immutable version request. This accepted specification remains immutable; later changes use events.
