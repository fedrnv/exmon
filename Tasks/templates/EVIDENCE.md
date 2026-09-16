---
schema: fdtasks-evidence/v1
evidence_id: EVD-<UUID-or-ULID>
task_id: <PROJECT>-<ID>
event_id: EVT-<UUID-or-ULID>
created_at: YYYY-MM-DDTHH:MM:SSZ
created_by: "actor"
---

# Evidence

- Source SHA / clean state / policy SHA / tested diff base: <values>.
- OS/toolchain/dependency lock or hash/hardware when relevant: <values>.
- AC IDs and test node IDs/manual scenarios: <mapping>.
- Exact commands, exit codes, pass/fail/skip/timeout counts and duration: <values>.
- Fixture identity/hash/seed and numerical/performance tolerances: <values>.
- Logs/artifacts and inspected visual result/reviewer when applicable: <links>.
- Failures, exclusions, missing checks and blockers: <explicit>.
- Diff review identity/result and cleanup receipt: <values>.
- Final verdict and relation of any metadata-only closure commit: <values>.

Do not include credentials, private customer data or unverifiable success claims.
