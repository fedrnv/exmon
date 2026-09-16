# Allocator-owned immutable FDTasks version events

Only the automatic allocator assigns `MAJOR.RELEASE.EPIC.ITEM`, writes immutable
transition events, updates `../version.json` and regenerates
`../VERSION_HISTORY.md`. Existing adoption and transition events are permanent.

Each allocated event binds sequence, version, request ID/type, Epic/work-item,
summary, verification references, request path and request content hash where the
repository allocator version supports it. Gaps, invalid transitions, duplicate
requests or repeated closure identities fail closed.
