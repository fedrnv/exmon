# Tasks/AGENTS.md

These instructions apply below `Tasks/` and supplement the root `AGENTS.md`.

- Read `tasks_file_structure.md`, the task's `TASK.md`, all events, evidence, and linked ADRs before changing task state.
- Never edit, rename, move, or delete an accepted `TASK.md`, event, evidence file, ADR, or task directory.
- Record each logical change as one new uniquely named event file.
- Event filenames use UTC, actor, event type, and a collision-resistant random suffix.
- Preserve concurrent events and record conflicts instead of guessing a winner.
- Record exact verification or explain why it was unavailable.
- Do not commit aggregate indexes, changelogs, roadmaps, task tables, or Kanban files.
- Never project Done until every acceptance criterion is verified.
