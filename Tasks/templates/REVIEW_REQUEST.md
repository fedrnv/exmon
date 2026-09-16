---
schema: fdtasks-review-request/v1
review_id: "REV-<UUID-or-ULID>"
epic_id: "<PROJECT>-EPIC-<ID>"
review_task_id: "<PROJECT>-<review-task-ID>"
previous_review_id: null
created_at: "<UTC ISO8601>"
created_by: "<actor>"
base_sha: "<full SHA before Epic changes>"
source_sha: "<full tested candidate SHA>"
policy_sha: "<full SHA containing applicable policy>"
---

# Request for independent Level-0 review

## Scope and traceability

<Epic/item/event links, AC-to-test/evidence mapping, diff, changed components,
compatibility contracts and exclusions.>

## Full gate evidence

<Gate report, commands/results, environment/target matrix, data/performance and
inspected UI evidence where applicable.>

## Risks, exceptions and questions

<Known limitations, failed/unavailable checks and prior correction links.>

## Requested review

Review requirements, correctness, data integrity, concurrency/recovery, UI,
compatibility, security, test adequacy and evidence completeness. Preparation of
this request is not approval or Epic closure.
