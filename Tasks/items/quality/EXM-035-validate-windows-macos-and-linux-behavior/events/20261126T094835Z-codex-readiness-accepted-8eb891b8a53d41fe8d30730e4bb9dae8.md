---
schema: fdtasks-event/v3
event_id: EVT-8eb891b8a53d41fe8d30730e4bb9dae8
task_id: EXM-035
event_type: readiness-accepted
created_at: 2026-11-26T09:48:35Z
actor: Codex exmon platform qualification
owner: Codex exmon platform qualification
base_revision: 4e68e9652e5bb587ca8d8f068a180cb30515adda
task_sha256: 73f4f0b66b8143d4ac6c4c4b5498b218d1a7af119fc7a3815f100211b56db834
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
atomicity:
  outcomes: [Evidence-bound cross-platform compatibility matrix for the accepted prototype]
  packets: [Manual native runner verification and portable-profile handoff]
  independent_acceptance: false
  boundary_reason: Runner execution and profile transfer directly verify the existing cross-platform acceptance criteria
---

Continue the same published035 owner. Authoritative main/task state refreshed;
no other developer claim or runtime-source change was found. The accepted TASK
explicitly permits manual-only GitHub Actions or documented local commands.
Connected GitHub reports this public repository with admin permission; the
existing repository authentication also verified Actions enabled via a read-only
API request. The connector lacks workflow dispatch, so use the existing
repository-authenticated manual API path, without exposing/persisting tokens or
acquiring accounts/secrets. Standard public Linux/Apple Silicon runners are
available according to the current official GitHub runner reference; no paid
runner, purchase, release, deployment or signing is authorized/inferred.

Extend the allowed verification paths to:
- .github/workflows/manual-platform-qualification.yml (workflow_dispatch only)
- .github/scripts/platform_qualification.py (bounded command/evidence supervisor)
- crates/app/examples/portable_profile_smoke.rs (public-controller fixture verifier)
- .gitattributes (only exact-byte portable fixture preservation)
- this task's append-only events/evidence and docs/platform-compatibility.md.

Pin action code, use read-only job permissions, trusted main only, fixed standard
ubuntu-24.04/macOS Apple Silicon labels and Rust1.98.0. Bound jobs/processes,
Cargo jobs, storage and retained evidence. Exercise synthetic/default fixtures;
do not capture/send on host control interfaces or open physical devices. No
push/PR/schedule trigger is admitted. The allocator remains the sole automatic
Actions exception and native FDTasks activation remains false.

Add the already Windows-exported synthetic profile as immutable evidence, retain
its exact bytes, and import/re-export it on each actual target through the same
public settings controller. A small example makes the previously ignored
verification harness reproducible; it adds no product API/transport behavior.
Keep all existing test assertions/deadlines. Record original failures and all
native/virtual/hosted distinctions; do not claim runtime success from builds.
The known Linux performance_soak warning must remain visible for its separate
correction; do not suppress strict lint failure. Host HypervisorPlatform is now
installed under the prior explicit owner installation authority with NoRestart;
it reports RestartNeeded and WHPX still fails to initialize. No host restart
has occurred. Its follow-up does not block safe hosted qualification work.

Local verification: format/lint/check and actual Windows profile import/export
for the example, static manual-only workflow/supervisor validation, then one
explicit manual run with source/run/OS/architecture identities and exact logs.
Physical EXM-036 and final Epic gate/review obligations remain unchanged. Goal
active; this is the same035 task identity, not a second item admission.
