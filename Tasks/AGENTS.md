# Tasks/AGENTS.md

## Current developer rules — FDTasks 5.7

Read [FDTasks_5_7_ADOPTION.md](FDTasks_5_7_ADOPTION.md) and the complete
[tasks_file_structure_v5_7.md](tasks_file_structure_v5_7.md) before
selecting, claiming, resuming or closing work. This owner-requested developer
policy supersedes older generic developer-rule references below; exact source
identity and local status mapping are in `developer-policy.json`.
Several developers may share one Epic, each task has one published owner.
Refresh GitHub/project state, publish the claim before implementation, and
immediately push every commit and verified closure. A losing claim refreshes
and reselects; it is never replayed over the winner. Preserve accepted history,
product safety and explicit owner exceptions. Native/automated activation is
separate and must retain its actual qualified version; see the adoption boundary.

These instructions apply below `Tasks/` and supplement the root `AGENTS.md`.

- Read `tasks_file_structure.md`, the task's `TASK.md`, all events, evidence, and linked ADRs before changing task state.
- Never edit, rename, move, or delete an accepted `TASK.md`, event, evidence file, ADR, or task directory.
- Record each logical change as one new uniquely named event file.
- Event filenames use UTC, actor, event type, and a collision-resistant random suffix.
- Preserve concurrent events and record conflicts instead of guessing a winner.
- Record exact verification or explain why it was unavailable.
- Do not commit aggregate indexes, changelogs, roadmaps, task tables, or Kanban files.
- Never project Done until every acceptance criterion is verified.


## FDTasks v4.2

Read [tasks_file_structure_v4.md](tasks_file_structure_v4.md). Every executable
Task, Bug or Correction is medium-sized, belongs to exactly one Epic and has
stable acceptance-criterion IDs plus an AC-to-test readiness matrix. Epics are
never sent to a coding Planner/Worker. Oversized accepted work is superseded and
split only through append-only records.

Run focused checks after each microtask and impact-selected focused, affected,
dependent and invariant verification before medium-item closure. Record exact
source and policy revisions, commands/manual scenarios, environment, outcomes,
limitations, applicable visual inspection and cleanup.

Each development Epic has a separate full-gate task and independent Level-0 review task.
Candidate changes or corrections reopen both and require a complete new gate
plus fresh review round. Epic closure requires all members Done, a current
successful gate and an Accepted review for the same tested source.

Create one immutable version request only on first verified closure. Reclosing
the same item creates no second request. Never assign versions or edit generated
version state, events or history.

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
