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
