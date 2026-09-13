# FDTasks v4 version events

Each version transition is one immutable event file. Required fields:

- schema: fdtasks-version-event/v1
- version: MAJOR.RELEASE.EPIC.ITEM
- created_at (UTC), actor and expected_base_revision
- epic_id and work_item_id
- change_type: task, bug, correction, epic, release or major
- bounded summary
- exact verification/evidence references

The agent creates the event, updates the canonical version state and regenerates
../VERSION_HISTORY.md in the same final commit. Duplicate versions or a gap in
the transition rules fail validation.
