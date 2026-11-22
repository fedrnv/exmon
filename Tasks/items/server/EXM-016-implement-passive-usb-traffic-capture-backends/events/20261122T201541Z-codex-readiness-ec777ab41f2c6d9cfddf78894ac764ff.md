---
schema: fdtasks-event/v3
event_id: EVT-ec777ab41f2c6d9cfddf78894ac764ff
task_id: EXM-016
event_type: readiness-accepted
created_at: 2026-11-22T20:15:41Z
actor: Codex exmon passive USB development
owner: Codex exmon passive USB development
base_revision: bac4c171d5eb0bf235ddbd8d7e5e8bcb33181a92
published_base: bac4c171d5eb0bf235ddbd8d7e5e8bcb33181a92
task_sha256: 0090d71f7da65c22ee43e5972eafa9331255a6588a28c1052a391f5199af9f89
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
atomicity:
  outcomes: [Qualify passive Linux usbmon capture metadata and payload on emulated USB traffic]
  packets: [Native runtime acceptance with isolated synthetic USB fixture]
  independent_acceptance: false
  boundary_reason: Capture start-stop and payload fidelity require the same real kernel source and controlled traffic fixture
---

Resume existing published owner, status In Progress. Refreshed origin/main equals bac4c17; EXM-017 verified Done and previous turn made progress. Root/Tasks rules, README/DEVELOPMENT, complete FDTasks 5.7/adoption, accepted ADR-0001/2/3 and all EXM-016 events/evidence revalidated; EXM-006/007 Done, sole task this turn within EXM-EPIC-001. No deeper rules or competing owner.

Use the already qualified portable QEMU/Alpine standard TCG environment under ignored target. At most two vCPUs, 512 MiB RAM and 2 GiB writable guest storage; synthetic read-only USB disk fixture only. No physical USB passthrough, host driver installation, elevation, private capture or repository/home sharing. Guest-only usbmon/debugfs provisioning and deliberate synthetic guest I/O are manual fixture preparation outside application capture. Native source must remain read-only and must never mount/load/configure USB or fall back to active access.

Allowed tracked scope: crates/capture usbmon source/native source/tests and isolated manual example if needed; docs/passive-usb.md; new EXM-016 immutable records. Preserve accepted metadata/bounds and unsupported platforms. Add explicit ignored native acceptance guarded for the emulated environment; exact expected synthetic bytes, metadata, source time, selected bus, startup/stop/restart/cancel and missing setup/access behavior. Native source test must not generate physical traffic. Finite poll/deadline, narrow fixture identity and resource supervision required. Source fixes only for demonstrated accepted behavior defects.

AC-01: cross-link Linux musl test with Rust LLD; run actual production usbmon on guest-only QEMU USB mass-storage traffic with known payload. Verify host transfer metadata, bytes and lifecycle, plus parser/reader/Session regressions. AC-02: actual Linux missing setup/denied access where feasible plus Windows Unsupported fixtures; no fallback. Run focused capture/domain tests, workspace format/lint, Linux native lint and architecture check; direct affected tests for any source changes. Record exact source/image/binary identity, scenarios, failures and unsupported limits, review and teardown. Close only with complete native evidence; physical electrical/device validation stays separate. Save durable next-task checkpoint and immediately push every commit.
