---
schema: fdtasks-spec/v3
id: EXM-102
title: "Make recording cleanup portable under strict lint"
type: Bug
area: quality
component: performance-harness
epic_id: EXM-EPIC-001
size: medium
initial_priority: P0
initial_status: Planned
initial_dependencies: ["EXM-033"]
created_at: 2026-11-26T10:29:26Z
created_by: Codex exmon platform qualification
atomicity:
  outcomes: [Benchmark recording cleanup preserves OS semantics and passes reference strict checks]
  packets: [Platform cleanup dispatch with local compile and recorder smoke verification]
  independent_acceptance: false
  boundary_reason: The Windows bounded retry and Unix single removal are one helper contract and one compile boundary
---

# EXM-102 portable benchmark recording cleanup

## Objective and local evidence

EXM-035 locally reproduced strict Linux-target Clippy failure on source
8b8fb24aed3654538f65ba095ac700744d676c96. performance_soak.rs273-282
unconditionally declares a deadline and loop, but only the Windows sharing-
violation arm uses/repeats them. Unix compilation reports unused_variables
and clippy::never_loop. The permitted local Windows owning turn invoked
Rust1.98.0 Clippy for x86_64-unknown-linux-gnu; no hosted result is acceptance.
See evidence/20261126T102926Z-local-failure.md.

## Scope and boundaries

Make the helper dispatch correctly by OS. Preserve Windows sharing-violation
error32 retry,500ms total budget and5ms waits, all other error propagation,
recorder finalization/metrics and the immediate single removal on non-Windows.
No lint allowance, renamed dummy variable, artificial retry, timeout relaxation,
product feature, API/schema/dependency change, CI stage or historical edit.

Allowed source path: crates/gui/examples/performance_soak.rs. Add only this
item's events/evidence and first verified-closure version request. Existing
scripts/run-performance-soak.ps1 is a verification entrypoint; do not change
its limits or behavior. Current035 ownership and local target obligations remain.

## Acceptance and trusted checks

| ID | Requirement | Trusted local check | Expected |
| --- | --- | --- | --- |
| AC-01 | Non-Windows cleanup uses one removal attempt with original results/errors and no unused deadline or never-loop | cargo +1.98.0 clippy -p exmon-gui --example performance_soak --target x86_64-unknown-linux-gnu --locked -- -D warnings | PASS; no suppression |
| AC-02 | Windows bounded sharing-error cleanup and recording shutdown remain intact | source comparison of unchanged Windows retry contract; scripts/run-performance-soak.ps1 -Mode smoke -OutputDirectory NEW_IGNORED_DIRECTORY -Toolchain 1.98.0 | all3 workloads and existing resource/cleanup checks PASS |
| AC-03 | Direct Windows consumer and invariants remain clean | cargo +1.98.0 clippy -p exmon-gui --example performance_soak --locked -- -D warnings; cargo +1.98.0 fmt --all -- --check; node scripts/check-architecture.mjs; reviewed diff | PASS |

Reuse035's qualified Rust/Zig GNU target tooling for local compile checks.
Record exact source/policy/toolchain, commands, negative baseline, smoke output
and hashes, source review, limitations and owned-resource cleanup. Do not claim
Linux/macOS runtime from cross-compilation. No hosted tests or hosted acceptance
are permitted by FDTasks5.7. The old actual2h EXM-033 baseline is retained at
its original source; this scoped correction does not invent a new2h verdict.

This is the smallest natural independently verifiable helper boundary. No
product/architecture decision or additional API is required; eligible for the
bounded implementation-model class. Planning this item grants no execution
ownership: the next fresh turn must refresh and publish its exclusive claim
before changing source.
