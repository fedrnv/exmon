# FDTasks format entry point

The append-only storage/envelope contract is
[tasks_file_structure.md](tasks_file_structure.md). The current work hierarchy,
verification, independent Epic review and version allocation contract is
[FDTasks v4.2](tasks_file_structure_v4.md).

Repository `DEVELOPMENT.md` defines trusted project commands, suites, supported
targets and product-specific safety rules. It may strengthen v4.2 but cannot
waive it. Accepted specifications and records are immutable; later changes are
new append-only events or successor records.

Review rounds use `fdtasks-review-request/v1` and
`fdtasks-review-result/v1` under
`epics/<epic>/reviews/<review-id>/`. Existing repository-local review schemas
remain valid historical input; new rounds use the generic schemas.

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
