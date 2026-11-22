---
schema: fdtasks-event/v3
event_id: EVT-f3e4b75e39a34d56a8b4a1d02f8d32d2
task_id: EXM-001
event_type: readiness-accepted
created_at: 2026-11-22T17:01:50Z
actor: Codex exmon domain development
base_revision: 5cee776510f3f1afabb4710db7d809077cfdf26d
policy_sha256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8
task_sha256: 179d9fa885ae76bd968b533b642b23878acdbd081bf18a9c13009f8e53380be8
atomicity:
  outcomes: [Transport-neutral product domain contract]
  packets: [Document entities and validate transport and failure examples]
  independent_acceptance: false
  boundary_reason: Entities and examples define one shared observation contract
---

EXM-001 remains medium in EXM-EPIC-001 under its accepted migration. No dependencies or existing owner. Published base and task bytes above revalidated against origin/main; other development Epics remain Planned. ADR-0001 through ADR-0003 apply.

Allowed paths: docs/domain-model.md and this item's new events/evidence, plus its first version request. No Rust implementation, persistence schema, UI or adapter implementation. Inputs: immutable TASK, ADRs and root development rules. Risk: ambiguous provenance or retention could lose data; mitigate through explicit checked ranges, loss accounting and examples. Failure/recovery: reject invalid metadata with diagnostics; preserve bytes; never transmit implicitly. Cleanup: no generated artifacts.

| AC | Scenario / level | Local verification | Expected |
| --- | --- | --- | --- |
| AC-01 | Document review of transport-neutral entities and Ethernet/TCP/UDP/serial/USB/CAN examples | Read docs/domain-model.md and trace each example through shared envelope, framing and entity references against ADR-0002 | Every transport expressible without GUI backend branching |
| AC-02 | Document review of bounded retention and invalid input scenarios | Read loss/error table; trace queue saturation, eviction, truncation, bad offsets and interface loss through explicit outcomes | Distinct backend/application/display loss; checked bounds and fail-safe TX |

Verification is manual contract inspection plus git diff --check. Format/lint: Markdown whitespace check. Unit/integration/transport tests not applicable to documentation-only change; no runtime claims. The next workspace task owns executable model realization. No independent Epic review is claimed here.