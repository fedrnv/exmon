---
schema: fdtasks-event/v3
event_id: EVT-0ef72278f7704d8ead320eb24ee0844c
task_id: EXM-033
event_type: readiness-accepted
created_at: 2026-11-25T22:48:26Z
actor: Codex exmon performance
base_revision: 3fd97ee3b2ad65caef4f5974bf8c63639ea77ec8
task_sha256: af2d6772ad933daa61d6fcf1ad14424a34f04faaff32da9c573a7d67d504654f
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
atomicity:
  outcomes: [One repeatable release performance verdict for the synthetic prototype]
  packets: [Local workload runner with common telemetry thresholds and soak evidence]
  independent_acceptance: false
  boundary_reason: Workload counts, resource bounds, and pass or fail must be derived from the same measured run to support a release decision
---

Published Done dependencies EXM-004/019/020/026/028 verified. EXM-031 session storage is available on the same published base. Reuse the synthetic Windows development boundary and existing worker budgets. Reference Windows x64 VPS on AMD EPYC-Milan, eight logical CPUs, ordinary user access and local NTFS storage. Report hardware and operating environment with each measurement. No physical interfaces, new dependency, hosted quality workflow, release or deployment.

One manual benchmark entry point exercises production capture, framing/classification/decoding, chart work, GUI frame construction, and transactional recording. Use deterministic small and mixed packets, fragmented stream framing, chart-heavy batches and a deliberately slow or blocked storage fixture. Separate observed backend loss, application drops, display decimation and recording drops. Time the workload and report throughput, worst and percentile UI latency where applicable, retained item/byte bounds and process memory over time. Bounded sampled output and generated synthetic data only. Release mode returns failure for missing scope, insufficient duration or thresholds exceeded, with results retained locally and compact closure evidence. Set explicit reference budgets before the acceptance run using established product bounds and observed baseline with disclosed tolerance. Preserve production limits, defaults and application behavior.

Allowed paths: local benchmark example or verification script and focused tests, docs/performance.md, new EXM-033 events/evidence/request. Touch a production interface only if required to observe counters through a read-only existing boundary and document the impact. No CI, existing immutable task records, allocator or product scope changes.

AC-01 -> run the benchmark short validation and an actual two-hour Windows soak with deterministic fixture phases; record program and command hashes, hardware, raw counts, measured rates, latency, memory and verdict. Show distinct loss counters, bounded growth and complete profile coverage. AC-02 -> verify negative verdicts for incomplete coverage and threshold regressions via deterministic focused checks; establish the documented manual release gate input for the later release task. Run focused tests, format, workspace Clippy, architecture and diff review. Record exact outcomes and clean owned artifacts.
