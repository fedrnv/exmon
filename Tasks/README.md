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

Every open Epic must have a dedicated full-gate task and a separate independent
review task before closure. Corrections reopen both. First verified closures
create immutable version requests; the automatic allocator alone writes version
events, `version.json` and [VERSION_HISTORY.md](VERSION_HISTORY.md).
