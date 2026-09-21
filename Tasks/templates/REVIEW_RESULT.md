---
schema: fdtasks-review-result/v1
review_id: "<matching request ID>"
epic_id: "<PROJECT>-EPIC-<ID>"
review_task_id: "<PROJECT>-<review-task-ID>"
previous_review_id: null
request: "REQUEST.md"
source_sha: "<same full SHA as request>"
policy_sha: "<same policy SHA as request>"
created_at: "<UTC ISO8601>"
reviewer: "<identity/session>"
independence: "<different person or fresh session; disclose limitations>"
verdict: "<Accepted | Changes requested | Blocked>"
---

For an explicitly requested maintenance review, use `epic_id: <PROJECT>-EPIC-999`
and the explicit verification task as `review_task_id`. Acceptance may close that
task but never EPIC-999. Ordinary maintenance items do not require this record.

# Independent Level-0 result

## Coverage and evidence checked

<Reviewed diff/components, AC/evidence links, independent checks and limitations.>

## Findings and disposition

| Finding ID | Severity | Location/reproduction/evidence | Required action | Blocking? | Resolution/correction link |
| --- | --- | --- | --- | --- | --- |
| <ID> | <severity> | <facts> | <action> | <yes/no + reason> | <link> |

## Acceptance decision

<Justify verdict. Accepted applies only to the exact source/policy SHA.>

## Task disposition

Append an event to `review_task_id` linking this immutable result. Accepted with
all mandatory criteria permits Done. Other verdicts leave it open and require
correction items. Never overwrite an old review round.
