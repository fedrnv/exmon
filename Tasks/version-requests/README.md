# Immutable FDTasks version requests

Use `fdtasks-version-request/v1` from `../tasks_file_structure_v4.md`. Each first
verified closure of a medium item or Epic/release adds one uniquely named JSON
request. Never edit or delete an accepted request. Reclosing the same item after
correction/review repetition creates no second request.

The automatic allocator is the sole version writer and permanently deduplicates
both request IDs and closed work-item/Epic identities across version epochs.
Agents never assign versions or edit `version.json`, version events or generated
history.
