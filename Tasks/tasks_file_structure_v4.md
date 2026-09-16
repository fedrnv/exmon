# FDTasks File Structure v4

Version: 4.2

## Authority and compatibility

This document is the normative cross-repository FDTasks lifecycle. The immutable
v3 envelope and append-only storage format remain valid historical input.
Repository `DEVELOPMENT.md` files may add product-specific suites, environments
and safety boundaries, but may not weaken this lifecycle. Conflicts are resolved
in favour of this document unless an explicit owner-authorized successor says
otherwise.

## Work hierarchy

- An **Epic** is a non-executable delivery and integration container.
- An executable **Task**, **Bug** or **Correction** is medium-sized: one
  independently verifiable outcome that a Planner-class model can decompose
  without inventing product, architecture or acceptance decisions.
- A **Microtask** is temporary runtime work produced from one medium item.
- Every executable item has exactly one `epic_id` and `size: medium`.
- Oversized accepted items are superseded through append-only events and replaced
  by an ordered series of medium items. Accepted records are never rewritten.
- At most one Epic is active unless the owner explicitly authorizes another
  scheduling policy.

## Required medium-item contract

A ready medium item freezes its objective, source baseline, inputs, dependencies,
scope, exclusions, security and compatibility boundaries, failure/recovery
behaviour, ordered stages, risk and impact assessment, observable acceptance
criteria, verification and cleanup.

Acceptance criteria use stable IDs (`AC-01`, `AC-02`, ...). A readiness matrix maps
each criterion to the scenario, test level, exact trusted command or manual steps,
fixture/environment, expected result and tolerance. Existing immutable tasks that
lack this information receive an append-only readiness event before dispatch.

## Verification levels

1. **Microtask:** run the focused checks assigned to that bounded change before
   advancing.
2. **Medium item:** run focused checks, affected-component tests,
   deterministically selected dependent regressions and applicable invariant
   smoke suites; review the actual diff, clean owned disposable artifacts and
   record exact evidence.
3. **Epic gate:** after all implementation/correction items are Done, run one
   complete canonical gate on a frozen candidate through a dedicated medium gate
   task.
4. **Epic review:** after a successful current gate, perform an independent
   Level-0 review through a separate medium review task and immutable review
   request/result artifacts.
5. **Release:** run release build, signing, install/update/rollback acceptance and
   publication checks under separate authorization.

Test selection follows behavioural impact, not filenames alone. Shared APIs,
schemas, dependencies, workflows, persistence, security boundaries or unknown
impact require the conservative catalog set and may require an early full gate.
Functional failures are never retried unchanged. A project may permit one
host-controlled infrastructure retry after cleanup; PASS after FAIL is FLAKY and
does not by itself accept the candidate.

Evidence identifies the source SHA, policy SHA, commands/manual scenarios,
environment, fixtures/seeds, outcomes including fail/skip/timeout, limitations,
reviewer and cleanup. Textual model claims and unavailable checks are not passing
evidence. UI changes require interaction tests and inspected visual evidence on
the supported sizes, DPI/themes and packaged/native targets.

## Mandatory Epic gate and review tasks

Every newly created Epic includes exactly one full-test gate task and one separate
independent review task. The gate depends on the implementation/correction set;
the review depends on the gate. The gate never depends on the review. Membership
is resolved from the Epic specification plus all append-only membership events.

For an already accepted open Epic that lacks this topology, add the two medium
items and membership/dependency events before closure; do not rewrite its original
records. A historically closed Epic is not rewritten retroactively.

The review task owns immutable rounds under
`Tasks/epics/<epic>/reviews/<review-id>/REQUEST.md` and `RESULT.md`. The reviewer
must be a different person or a fresh independent model session and must disclose
the independence boundary. A request is not approval. Only an `Accepted` result
for the current tested source can close the review task.

Findings become medium correction items in the same Epic. Adding a correction or
changing the candidate immediately reopens a Done gate and any Done review task by
new events. After corrections, repeat the complete gate and create a new review
round. Keep all prior rounds. Epic closure requires every member Done, the latest
gate passed, the latest review Accepted, all blocking findings resolved and a
separate Epic closure event/evidence.

## Canonical version and allocation

The canonical version is four non-negative integers:
`MAJOR.RELEASE.EPIC.ITEM`.

- `MAJOR`: explicit owner-authorized fundamental transition; lower fields reset.
- `RELEASE`: published release; EPIC and ITEM reset.
- `EPIC`: first verified closure of an Epic; ITEM resets.
- `ITEM`: first verified closure of each medium Task, Bug or Correction.

The closing agent never assigns a version or edits allocated state/history. It
adds exactly one immutable `fdtasks-version-request/v1` JSON file on first closure.
Reopening and reclosing the same gate/review/item adds events and evidence but no
second request. The allocator permanently deduplicates both `request_id` and the
closed work-item/Epic identity across version epochs.

The path-scoped automatic `FDTasks version allocator` workflow is the sole
allocator. It serializes on `main`, replays and validates the immutable ledger,
assigns the next tuple, writes one immutable event, updates `Tasks/version.json`,
regenerates `Tasks/VERSION_HISTORY.md`, commits and fast-forward pushes. This is
the only automatic GitHub Actions exception. Build, test, canonical, release and
deployment workflows remain manual.

## Request contract

```json
{
  "schema": "fdtasks-version-request/v1",
  "request_id": "globally-unique-id",
  "type": "task_closed",
  "epic_id": "EPIC-ID",
  "work_item_id": "TASK-ID",
  "summary": "Completed outcome",
  "verification": ["Tasks/items/.../evidence/... or exact durable reference"]
}
```

Allowed types are `task_closed`, `bug_closed`, `correction_closed`,
`epic_closed`, `release_published` and `major_changed`. `major_changed` also
requires the new integer `major` and explicit owner authorization evidence.
Invalid, duplicate, unknown-member or evidence-free requests fail closed.

SemVer surfaces represent the tuple as `MAJOR.RELEASE.EPIC+item.ITEM`; Windows
file versions may use all four integers. Update comparison uses the integer tuple.

## Immutable records

Accepted TASK.md, EPIC.md, ADRs, events, evidence, review request/results and
version requests/events are never edited, moved, renamed or deleted. Current
status is derived from ordered events, not `initial_status`. Mutable aggregate
task lists, Kanban and version history are generated projections, not hand-edited
authority.


Canonical cross-repository source: `fedrnv/FDTasks@4bfb77800d78ba690a7362e6177bde15e17d64d5`.
