# Task Management

This directory uses `tasks_file_structure.md`.

- One permanent directory per task under `items/<area>/`.
- `TASK.md` is immutable after acceptance.
- Every later status, comment, amendment, verification, or completion is a new immutable file under the task's `events/`.
- Evidence belongs under the task's `evidence/`.
- Task directories are never moved or renamed.
- Aggregate task lists, roadmaps, Kanban boards, activity, and changelogs are generated and are not committed.


## FDTasks v4

New work follows [Tasks File Structure v4](tasks_file_structure_v4.md).
Executable Tasks and Bugs are medium-sized children of Epics. Legacy v3 records
remain immutable; they must pass medium-readiness and Epic-membership checks
before dispatch or be superseded and split. Version transitions are immutable
under `version-events/`; the generated traceability table is
[VERSION_HISTORY.md](VERSION_HISTORY.md).
