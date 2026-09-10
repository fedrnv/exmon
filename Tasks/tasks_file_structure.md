# Tasks File Structure

Version: 3.0

```text
Tasks/
  README.md
  AGENTS.md
  tasks_file_structure.md
  decisions/
  templates/
  items/
    foundation/
    server/
    sync/
    web/
    operations/
    quality/
    governance/
      <TASK-ID>-<slug>/
        TASK.md
        events/
        evidence/
```

`TASK.md` is the immutable accepted specification. Later information is appended as immutable event files named `<YYYYMMDDTHHMMSSZ>-<actor>-<event-type>-<random>.md`. Evidence is stored with its task. Empty event and evidence directories are created only when required because Git does not retain empty directories.

Current status, priority, owner, roadmap, Kanban, activity, and changelog are derived views and are not manually committed. Task directories retain their original area and name permanently. Areas are stable storage locations: foundation, server, sync, web, operations, quality, and governance.
