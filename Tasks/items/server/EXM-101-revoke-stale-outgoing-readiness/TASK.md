---
schema: fdtasks-spec/v3
id: EXM-101
title: "Revoke stale outgoing readiness after interface loss"
type: Bug
area: server
component: outgoing
epic_id: EXM-EPIC-001
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-027"]
created_at: 2026-11-26T07:34:15Z
created_by: Codex exmon platform qualification
atomicity:
  outcomes: [Outgoing readiness and terminal outcomes agree after interface revocation]
  packets: [Coherent TX authority observation with deterministic regression]
  independent_acceptance: false
  boundary_reason: Readiness, terminal publication and later Send admission share the same authority boundary
---

# EXM-101 outgoing readiness after interface loss

## Objective and evidence

EXM-035 Linux QEMU testing at 6c1e59a0754e636ae3ff0e20c9cdae714bb42b42 exposed
actual_partial_unknown_busy_and_interface_loss_are_not_successful_tx_counts.
After the audited adapter reports Disconnected, template.last becomes terminal
and backend calls remain zero, but can_send(id) still returns true.
Failure is at app/tests/outgoing.rs line436. Editor::ready reads cached state.tx,
and the editor loop takes a TX snapshot before reading terminal history. Inspect
and establish the actual ordering before changing the implementation.

## Scope and boundaries

Fix the shared outgoing readiness/terminal observation contract with bounded
synchronization. Preserve explicit Send authority, safe Stop, unchanged Busy
retry behavior, exact revision checks and partial/unknown outcome accounting.
Preserve the existing failure assertion. No physical transmission or new device
authority. Do not add sleeps, retries or larger timing tolerances to hide the race.

Allowed paths are crates/app/src/outgoing.rs, narrowly required transmission
observation API, app outgoing/transmission tests, and direct GUI outgoing tests
only if needed. New EXM-101 events/evidence/version request. No other runtime
feature, dependency, schema, CI or accepted-history edit.

## Acceptance and verification

| ID | Requirement | Trusted check | Expected |
| --- | --- | --- | --- |
| AC-01 | Terminal interface-loss observation implies revoked ready state and no backend submission | deterministic regression with controlled ordering, app outgoing/transmission suites | false readiness, stale Send rejected, zero writes |
| AC-02 | Partial, unknown and Busy results preserve exact counts and allowed explicit retry | cargo +1.98.0 test -p exmon-app --test outgoing --test transmission --test scheduling --locked | PASS |
| AC-03 | The failure is corrected on actual Linux runtime and Windows direct consumers | rebuild affected Linux musl binaries and run in isolated QEMU; Windows GUI outgoing checks | PASS without relaxing assertions |

Run focused tests, workspace format/Clippy and architecture checks. Reuse035
trusted bounded Linux toolcache and exact guest environment. Record source,
commands, output hashes, failures, review and cleanup. Medium natural boundary,
no product decisions required. Route to a model capable of concurrency analysis.
