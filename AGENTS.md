# AGENTS.md

These rules apply to the entire repository and to human and AI contributors.

## Required reading

Before any work:

1. read `README.md` and `DEVELOPMENT.md`;
2. read the relevant accepted ADRs under `Tasks/decisions/`;
3. select the next unblocked Work Item under `Tasks/items/`;
4. read its immutable `TASK.md`, every event, and all evidence;
5. follow `Tasks/AGENTS.md` for task records.

## Autonomous development mode

- Development is performed continuously by AI agents, one accepted Work Item at a time, in dependency order.
- An agent must not stop after planning, scaffolding, a partial implementation, or a successful commit while another safe and unblocked task can be completed.
- For every task, the loop is: inspect scope and dependencies; implement; add or update tests; run focused tests; run relevant regression tests; review the diff; remove generated files and temporary artifacts; record immutable verification; commit; push to `main`; select the next unblocked task.
- Work directly on `main`. Do not create branches or pull requests unless the project owner explicitly requests them.
- GitHub Actions must remain manual-only until the project owner changes this policy.
- Never force-push or rewrite published history.
- Preserve unrelated user changes.

## Prototype-first sequence

- The first development milestone is a complete functional prototype that can be run and debugged without the physical exchange stand.
- Implement transport traits, deterministic fixtures, mock adapters, loopback adapters, virtual serial/CAN where available, PCAP replay, and simulated USB devices so that the complete UI and processing pipeline can be exercised.
- The prototype must include interface selection, RX/TX panels, automatic grouping, naming, framing, field decoding/editing, HEX/ASCII inspection, charts and moving averages, one-shot transmission, periodic transmission, configuration, session recording, and diagnostics.
- Do not wait for the physical stand to implement functionality that can be developed and verified with mocks, replay files, loopback, or virtual devices.
- After the prototype passes its automated and virtual integration tests, continue with platform adapters, packaging, and all work that does not require the stand.
- Physical RS-485, USB, CAN, timing, electrical, and real-protocol validation is performed on the exchange stand when it becomes available. Record blocked hardware tests explicitly; do not claim them as passed.

## Valid reasons to stop

An agent may stop only when no safe unblocked task remains because completion requires one of the following:

- access to the physical exchange stand or a specific unavailable device;
- administrator permission, driver installation, OS signing identity, or credentials that the agent does not have;
- a product or architecture decision with materially different outcomes that is not settled by the accepted tasks and ADRs;
- an external service or repository permission that is unavailable;
- a safety-relevant action requiring explicit owner authorization.

Before stopping, the agent must:

1. complete and push all work that does not depend on the blocker;
2. add an immutable task event describing the exact blocker;
3. state what was attempted and verified;
4. provide the shortest concrete action required from the project owner;
5. identify the next task that becomes unblocked afterward.

## Architecture and safety rules

- Keep capture, framing, decoding, storage, application, and GUI layers separated.
- Keep transport-specific privileges and unsafe platform APIs behind small reviewed adapters.
- Never run capture, decoding, storage writes, or transmission scheduling on the GUI thread.
- Use bounded queues and memory; expose backend loss, application drops, and display decimation separately.
- Validate all untrusted capture files, protocol definitions, device data, lengths, offsets, and arithmetic.
- Never transmit on a physical interface without an explicit user action or an explicitly enabled schedule.
- Periodic transmission must default to disabled and must stop immediately on user request, interface loss, or invalid configuration.
- Do not silently weaken tests, safety limits, error handling, or accepted scope to make a task pass.
- Code comments and technical documentation are written in English.
- Never commit credentials, sensitive captures, device serial numbers, signing material, personal data, production secrets, or generated build outputs.

## Completion standard

A Work Item is Done only when:

- every acceptance criterion is demonstrably satisfied;
- relevant unit, integration, and regression tests pass;
- format and lint checks pass;
- failures and unsupported platform behavior are explicit;
- exact verification is recorded in a new immutable FDTasks v3 event;
- the reviewed change is committed and pushed to `main`.

A commit or a locally passing happy-path test alone is not completion.
