# Task Management

This directory uses `tasks_file_structure.md`.

- One permanent directory per task under `items/<area>/`.
- `TASK.md` is immutable after acceptance.
- Every later status, comment, amendment, verification, or completion is a new immutable file under the task's `events/`.
- Evidence belongs under the task's `evidence/`.
- Task directories are never moved or renamed.
- Aggregate task lists, roadmaps, Kanban boards, activity, and changelogs are generated and are not committed.


## FDTasks v4.2

New work follows [Tasks File Structure v4](tasks_file_structure_v4.md).
Executable Tasks, Bugs and Corrections are medium-sized children of Epics and
include an AC-to-test readiness matrix. Legacy v3 records remain immutable and
are amended only through append-only events.

Every open development Epic must have a dedicated full-gate task and a separate independent
review task before closure. Corrections reopen both. First verified closures
create immutable version requests; the automatic allocator alone writes version
events, `version.json` and [VERSION_HISTORY.md](VERSION_HISTORY.md).

## FDTasks v4.2 maintenance exception: EPIC-999

Owner-authorized policy from `fedrnv/FDTasks@3ff6a1162f76cbcbc8a7a90ca1527f1b4753cda4`:

- Bugs belonging to development in an open Epic are added to that Epic.
- Bugs related to a closed Epic, bugs with unclear Epic association, and isolated
  minor improvements go to `EXM-EPIC-999`. Link the original Epic when known;
  do not reopen the closed Epic because of a later bug.
- EPIC-999 stays open permanently and is excluded from the one-active-development-
  Epic limit. It has no mandatory overall gate/review and no Epic closure request.
- Ordinary maintenance tasks require testing changed functionality and directly
  affected behaviour before task closure, with exact evidence. Full project tests
  and independent review require an explicit verification task in EPIC-999; one
  task may request both. Its scope/dependencies cover the selected work only.
- Unrelated new maintenance items do not reopen earlier verification tasks.
  First verified item closure follows normal ITEM allocation; EPIC never increments
  for EPIC-999. Existing development Epics retain full gates and independent review.

This explicit exception supersedes blanket per-item full-test/review requirements
and test-deferral rules only for EPIC-999. Preserve project-specific test commands,
domain/security checks for affected behaviour, resource supervision, immutable
history and build/release authorization. Runtime descriptions are not evidence of
support: report any incompatible tooling; never invent successful verification.
