# EXM-039 prototype verification

Published base f76db525fcb4b9cc274812259b5fe94f3eafed7f, tested source tree a74b621f83ee30a054364d0d583fcb8e3ab02ead.
Policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Windows Server 2025 x64 VPS, Rust/Cargo1.98.0, Node24.22.1.

AC-01 passed. docs/virtual-prototype.md records the toolchain, launch and full
manual demonstration. The actual native Windows smoke ran through recording,
recovery and export, rendered the production shell and exited within its8s
deadline. No physical driver is needed by this default synthetic route.

AC-02 passed. The new complete_virtual_session_preserves_data_and_explicit_transmission
GUI test crosses Start recording, capture, classification, received draft cloning,
field edit, explicit mock TX binding, exact byte submission, periodic Start/Stop,
live moving averages, synthetic PCAP import and recovered committed session state
through one production Shell. Source bytes remain unchanged, edits cause no send,
and capture/TX remain disabled after recovery. Capture suites exercised actual
Windows TCP/UDP loopback and virtual serial/CAN/simulated USB/HID boundaries.

AC-03 passed through the existing production GUI suites for interface and filter
selection, naming, field decode/edit, HEX/ASCII, settings/framing preview,
averaging/axes, schedule safety, recording/export and unsupported native HID.
PCAP import uses the bounded packet inspector. It is inert offline inspection.
Physical adapters and macOS/Linux packaged behavior remain035/036/037 evidence.

AC-04 passed with339 unique unit/integration tests across six layers. The
workspace run passed app121/capture48/domain6/GUI28/protocol88 before reaching
a Windows line-ending fixture failure in storage configuration. The final
storage/protocol/domain command passed storage48/protocol88/domain6 after
correcting only that test extractor. Existing app/capture/GUI functional inputs
were unchanged. Two default-ignored tests are explicit manual campaign and
abrupt-child helpers; their accepted explicit parent/campaign evidence remains
recorded under034 and031 respectively.

Commands and outcomes follow.

- cargo +1.98.0 test -p exmon-gui --test prototype --locked -- --test-threads=1 --nocapture passed.
- cargo +1.98.0 test --workspace --locked -- --test-threads=1 initially exposed an observational history-helper race. After correction, app, capture, GUI, domain and protocol passed. Storage configuration then exposed CRLF extraction in its documented example test.
- cargo +1.98.0 test -p exmon-app --test transmission --locked -- --test-threads=1 passed20 tests after returning the completed Attempt captured by the successful poll, within the original deadline.
- cargo +1.98.0 test -p exmon-storage --test configuration --locked passed13 after normalizing CRLF in the test-only extractor.
- cargo +1.98.0 test -p exmon-storage -p exmon-protocol -p exmon-domain --locked -- --test-threads=1 passed142 tests.
- cargo +1.98.0 clippy --workspace --all-targets --locked -- -D warnings passed.
- cargo +1.98.0 fmt --all -- --check passed.
- node scripts/check-architecture.mjs passed.
- cargo +1.98.0 run -p exmon-gui --example native_sessions_smoke --locked passed.
- Staged diff, immutable-history and source-scope review passed.

No production algorithm or resource limit changed. Existing two-hour033
performance evidence covers unchanged production worker inputs. This item adds
verification, documentation and two test portability fixes. The final recovered
prototype and native screenshots were inspected at1360x900. Current existing
visual tests exercised their supported sizes, themes and DPI. No UI change.

Hashes before local artifact cleanup:

- complete-session.png 7fde73a50c447db3d56dc8239550981ce545a1a907edfd6e73d5f3aaf422a9c9
- exm032-native.png e8f120846792edd43343f5ebc53828c717ff15972840dcb04bf2cfda4777a242
- exm039-final.log a8be3ffcb72e9690381aa944bc156390e01c3beb2c1d7777b44e300bcfd26bdd
- exm039-remaining.log 25f5692a97139ddb615b9239dc93c018cece72d707663c925de2eb3b3952bdd5

Owned synthetic files, logs and screenshots were cleaned, build cache preserved.
