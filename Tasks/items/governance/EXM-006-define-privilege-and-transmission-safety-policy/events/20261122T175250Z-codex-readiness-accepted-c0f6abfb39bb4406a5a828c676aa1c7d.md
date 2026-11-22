---
schema: fdtasks-event/v3
event_id: EVT-eaa01f3f-2f4c-466f-a1bf-c1efffe79f3d
task_id: EXM-006
event_type: readiness-accepted
created_at: 2026-11-22T17:52:50Z
actor: Codex exmon safety policy development
base_revision: 5bb8a0c556b1ab1aecc0d5a8568119d33ac581d7
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
task_sha256: d298370daf5e9f270a129bc29da98e05c492f2644cfafaed2efecf3efb209851
atomicity:
  outcomes: [Platform privilege and active-transmission product safety contract]
  packets: [Document platform boundaries and verify default-disabled configuration behavior]
  independent_acceptance: false
  boundary_reason: Permission and transmission rules define one product safety boundary
---

EXM-001 dependency published Done at aca7a30; current main contains EXM-002 through EXM-005 Done. EXM-006 Planned/unowned, medium in EXM-EPIC-001 by accepted migration; no other development Epic active. Rules: root AGENTS.md/DEVELOPMENT.md, Tasks/AGENTS.md, current 5.7 adoption, ADR-0001/2/3. No deeper rules. Product security policy is the accepted EXM-006 deliverable, not a change to task lifecycle/developer rules.

Allowed: docs/security-and-transmission.md, README.md link, this task's new evidence/events and first version request. Specify Windows/macOS/Linux capture, serial, USB and CAN privileges, trusted driver boundaries, privacy and safe explicit TX. No driver install, elevation, privilege change, physical transmission, new runtime scheduler or adapter. Inputs include docs/domain-model.md, docs/configuration.md and existing startup-safe config implementation. Research official platform/backend documentation; cite supported constraints and distinguish product decisions from platform facts. Recovery: permission denial disables affected capability with actionable bounded reason; no automatic elevation/fallback. Risk: stale platform assumptions and implicit rearm; source review and existing config safety tests mitigate.

| AC | Scenario/level | Exact check | Expected |
| --- | --- | --- | --- |
| AC-01 | Manual contract/source review | Read platform matrix against linked Npcap/libpcap/libusb/kernel/Microsoft primary docs | Explicit boundaries for each transport/platform; unsupported combinations stated |
| AC-02 | Default/reloaded configuration and unsafe schedule regression; source audit | cargo test -p exmon-storage --test configuration --locked; rg --files crates; inspect app and capture entry points | Defaults disabled; persisted enabled rejected; no startup send path; explicit future worker authorization contract |

Run cargo test -p exmon-app --test diagnostics --locked for privacy contract referenced by policy; git diff --check. Markdown-only edit: Rust formatting/lint evidence from unchanged EXM-005 remains valid; no new Rust code. No physical/native UI validation claimed. Preserve target/.idea and accepted history.

Fresh readiness/claim after rejected unpublished ff2e552, preserved under refs/codex/rejected-claims/EXM-006-ff2e552. Remote update 5bb8a0c changed version records only; task remains Planned/unowned, source/rules/dependencies unchanged. No implementation began before this fresh publication.
