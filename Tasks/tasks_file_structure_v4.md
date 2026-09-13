# FDTasks File Structure v4

Version: 4.0

## Work hierarchy

- An **Epic** is a non-executable delivery and integration container.
- An executable **Task** or **Bug** must be medium-sized: one independently
  verifiable outcome that a Planner-class model can decompose without inventing
  product or architecture decisions.
- A **Microtask** is temporary runtime work produced from one medium item. It is
  not a substitute for the accepted Task/Bug contract.
- Every executable item belongs to exactly one Epic. Existing v3 items receive
  this relationship by an append-only event; accepted TASK.md files stay immutable.
- Oversized items are never dispatched. Append a Superseded event and replace
  them with an ordered series of medium items that preserves every requirement.

A medium item must freeze objective, inputs/context, dependencies, scope,
exclusions, security and compatibility boundaries, failure/recovery behavior,
ordered stages, focused tests, observable acceptance criteria, cleanup and
completion evidence.

## Verification levels

1. Microtask: focused formatting/checks/tests for its bounded change.
2. Medium Task/Bug: affected-component tests plus dependent contract/regression
   tests selected deterministically from changed paths; review; cleanup; commit.
3. Epic closure: complete canonical build, all required workspace/integration/
   security/browser/package/platform checks, Level-0 review, cleanup and evidence.
4. Release: complete release build, signing, install/update/rollback acceptance
   and publication.

Unknown or broad impact selects the complete gate. A passing child task does not
close an Epic. Failed Epic verification creates correction Tasks/Bugs inside the
same Epic and repeats the complete gate.

## Version model

Canonical FDTasks version is four integers: MAJOR.RELEASE.EPIC.ITEM.

- MAJOR changes only by explicit owner decision for a fundamental compatibility,
  architecture or product-stage change; lower fields reset to zero.
- RELEASE increments for each published release; EPIC and ITEM reset to zero.
- EPIC increments only when the Epic and its complete gate pass; ITEM resets.
- ITEM increments after every completed medium Task or Bug.

The development agent reserves and increments ITEM before the task's final
commit and includes version change, completion event and history record in that
same commit. Epic and release transitions use their respective acceptance
commits. Publication to main is serialized with compare-and-swap; conflicts
refresh main and allocate the next valid number. GitHub Actions never allocate
or commit a version. Manual workflows only verify consistency.

Ecosystems requiring SemVer use MAJOR.RELEASE.EPIC+item.ITEM for package
metadata; Windows file versions may use all four integers. Updater comparison
uses the canonical four integer tuple.

## Version history

Source records are immutable files under Tasks/version-events/. Each record
contains version, UTC time, Epic ID, Work Item ID, type (task, bug, correction,
epic, release, major), summary, verification evidence and expected base
revision. Tasks/VERSION_HISTORY.md is a deterministic table generated from those
records and committed in the same closure commit. Validation must rebuild and
compare it; humans and models do not edit rows directly.

## Compatibility and execution

v3 specifications/events/evidence remain immutable and valid historical input.
Before any v3 executable item is dispatched under v4, validate medium readiness
and Epic membership. Add events or supersede/split it; never rewrite it.

All repository-specific main-only, no-PR, manual-workflow, security, continuous
execution, cleanup and stop-with-commit/push rules remain in force.
