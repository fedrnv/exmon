---
schema: fdtasks-epic/v4
id: EXM-EPIC-D001
title: "Application-scoped traffic and packet structure inspection"
initial_status: Planned
initial_dependencies: ["EXM-EPIC-001","EXM-EPIC-002"]
created_at: 2026-11-21T22:36:31Z
created_by: "Fedor and Codex"
---

# EXM-EPIC-D001 — Application-scoped traffic and packet structure inspection

## Outcome
Select an application on Windows, Linux or macOS, inspect its verified network flows, reconstruct packet/message fields and review, edit and reuse evidence-backed signatures. Clearly distinguish packets, reassembled messages, opaque encrypted bytes and uncertain attribution.

## Owner request and boundary
Owner requested these tasks on 2026-11-21. This is accepted future planning only, not activation, a development claim or permission to start unrelated implementation. Existing accepted tasks and numeric Epic identities remain unchanged. Normal one-active-development-Epic scheduling applies.

## Accepted membership
- EXM-085: Define process identity and traffic attribution contract
- EXM-086: Attribute Windows network traffic to processes
- EXM-087: Attribute Linux network traffic to processes
- EXM-088: Attribute macOS network traffic to processes
- EXM-089: Select an application and inspect its live traffic
- EXM-090: Inspect inferred packet structure and reusable signatures
- EXM-091: Prove application traffic and signature inspection
- EXM-092: Independently review application traffic inspection

## Dependencies and integration
Reuse the initial release's capture/UI/session infrastructure and Epic 002's protocol reconstruction/export; no duplicate decoder or reconstruction engine. Native platform capability evidence is mandatory; lack of host or privilege must be explicit.

## Gate and review
FDTasks 5.7 policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Exactly six functionality items, integration gate EXM-091 and independent Level-0 review EXM-092.
Gate depends on all six implementation items; review depends only on the gate.
Run the targeted changed-area/direct-interaction gate locally on a frozen candidate. New findings/candidate changes reopen gate/review by append-only events. Epic closure requires every member Done, current passing gate, independent Accepted review and no blocking findings. Missing native/upstream evidence stays blocked.

## Practical acceptance journey
Run two applications, select one, inspect its RX/TX and connections, open a message, click inferred fields and highlighted byte ranges, accept/edit a masked signature and apply it to later positive and negative traffic. Repeat attribution lifecycle/permission cases on each native platform.
