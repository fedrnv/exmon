# EXM-001 verification and continuation checkpoint

Source baseline: 323bdd56fc1edaee73e84085e8abc413aa14aaed. Candidate docs/domain-model.md SHA-256: c0700aab5e4189969255e86344b60e799e122081d341b47d9d04270fd4dfab01.
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Reviewer/executor: Codex exmon domain development (author inspection, not independent Epic review).
Environment: Windows PowerShell, local repository on main; synthetic byte examples only.

AC-01 PASS, manual inspection: all nine entity kinds have identity/lifecycle definitions. Ethernet, TCP, UDP, serial, USB (including CDC), CAN use the same observation envelope and provenance. TCP spans reconstruct 41 42 0A then 43 0A; UDP field 2 yields 42; USB preserves report id; CAN standard identifier and DLC bounds are explicit. GUI consumes capabilities and shared values, without backend handles. ADR-0001/2/3 boundary preserved.

AC-02 PASS, manual inspection: backend loss, ingress drop, display decimation, retention eviction and recording loss have distinct outcomes; full ingress queue cannot hide loss notification. Invalid original lengths rejected, oversized allocation rejected, offset overflow checked, partial framing state bounded. Interface loss disables schedules and stale requests; already submitted bytes cannot be recalled. Big-endian 10 20 equals 4128; offset 2 length 1 exceeds two bytes and yields error. Average of 10 and 14 equals 12 with explicit warm-up/gap handling.

Commands: git diff --cached --check PASS (exit 0); git diff --cached --stat inspected: one 143-line domain document. Full document read and reviewed against immutable task and readiness. No executable changes, so Rust format/lint/unit/integration, native UI and hardware tests are not applicable and were not run. No implementation/hardware support claimed. No generated task artifacts. Unrelated .idea directory appeared during work and is preserved/untracked.

Checkpoint: EXM-001 verified Done after publication of the accompanying closure. Next exact action in a fresh IDE/agent turn: fetch origin/main; validate EXM-001 published Done and EXM-002 dependency readiness; read EXM-002 all events/evidence; publish FDTasks 5.7 readiness and ownership before creating the six-layer Rust workspace. No hardware needed. Authority: owner-authorized development on main. No credentials requested or exposed.

Compatibility: the existing version publisher CLI requires GitHub Actions and is not a qualified local 5.7 publisher. No environment guard bypass or manual version assignment performed. First-closure request is published for the existing allocator under the explicit adoption compatibility boundary; local native allocation remains unqualified.