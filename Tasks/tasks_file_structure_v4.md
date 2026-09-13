# FDTasks File Structure v4

Version: 4.1

## Work hierarchy

- An **Epic** is a non-executable delivery and integration container.
- An executable **Task** or **Bug** is medium-sized: one independently verifiable outcome a Planner-class model can decompose without inventing product or architecture decisions.
- A **Microtask** is temporary runtime work produced from one medium item.
- Every executable item belongs to exactly one Epic.
- Oversized accepted items are superseded by an ordered series of medium items; accepted records are never rewritten.

A medium item freezes objective, inputs, dependencies, scope, exclusions, security and compatibility boundaries, failure/recovery behavior, ordered stages, focused tests, observable acceptance criteria, cleanup and completion evidence.

## Verification levels

1. Microtask: focused checks and tests for its bounded change.
2. Medium Task/Bug: affected-component and deterministically selected dependent regression tests, review, cleanup and closure commit.
3. Epic closure: complete canonical gate, Level-0 review, cleanup and evidence.
4. Release: release build, signing, install/update/rollback acceptance and publication.

Failed Epic verification creates correction items in the same Epic and repeats the complete gate.

## Canonical version and allocation

The canonical version is four integers: `MAJOR.RELEASE.EPIC.ITEM`.

- `MAJOR`: owner-authorized fundamental product/compatibility transition; lower fields reset.
- `RELEASE`: published release; EPIC and ITEM reset.
- `EPIC`: verified Epic closure; ITEM resets.
- `ITEM`: every completed medium Task, Bug or correction. It is one repository-wide counter within the current epoch, not an Epic-local counter.

The agent never edits the assigned version or history. In its closure commit it adds exactly one immutable JSON request under `Tasks/version-requests/`. The automatic `FDTasks version allocator` workflow is the sole allocator. It serializes requests on `main`, assigns the next version, writes an immutable event, regenerates `Tasks/VERSION_HISTORY.md`, updates `Tasks/version.json`, commits and pushes the result. Concurrent agents may close different tasks or Epics; unique request files and serialized compare-and-swap retries prevent a shared-file race.

This path-triggered allocator is the only automatic GitHub Actions exception. Builds, tests, canonical gates, releases and deployment remain manual and are executed as part of the applicable project build/closure process.

## Request contract

A request uses schema `fdtasks-version-request/v1` and contains:

```json
{
  "schema": "fdtasks-version-request/v1",
  "request_id": "globally-unique-id",
  "type": "task_closed",
  "epic_id": "EPIC-ID",
  "work_item_id": "TASK-ID",
  "summary": "Completed outcome",
  "verification": ["exact command or evidence"]
}
```

Allowed types are `task_closed`, `bug_closed`, `correction_closed`, `epic_closed`, `release_published`, and `major_changed`. `major_changed` additionally requires an integer `major` and explicit owner authorization evidence. Invalid or duplicate requests fail closed.

Ecosystems requiring SemVer represent the tuple as `MAJOR.RELEASE.EPIC+item.ITEM`; Windows file versions may use all four integers. Updater comparison uses the canonical integer tuple.

## Compatibility

v3 records remain immutable historical input. Before a v3 item is dispatched under v4, validate medium readiness and Epic membership. Repository-specific main-only, no-PR, security, continuous-execution, cleanup and stop-with-commit-and-push rules remain in force.
