# EXM-032 export verification

- Published claim eef5955dfbdaa7878c14b13f2d4c287db8534b17 and canonical readiness 2c902e2.
- Tested product tree 7594db27eeb6c980b04e712b0939158f6daddd91. Canonical TASK SHA-256 9c8e456deab2808daccf003d3f940f41f862ae354b54b39b47fafae9083a1c7c.
- FDTasks policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8. The readiness follow-up corrected the initial Windows CRLF checkout hash to the canonical immutable Git bytes without changing ownership or product inputs.
- Windows Server 2025 Datacenter build 26100.33650, x86_64-pc-windows-msvc, rustc 1.98.0 (88d9e12ae), Cargo 1.98.0 (797e8a9bc), Clippy 0.1.98, Node 24.22.1. The installed rolling stable rustc 1.100.0 built the initial GUI check and passed the initial storage regression, with baseline deprecation warnings for Atomic::fetch_update. Strict acceptance used the same Rust 1.98.0 as EXM-031.

| Local command | Result |
| --- | --- |
| `cargo +1.98.0 test -p exmon-storage --test export --test sessions --locked` | 17 passed. The existing ignored subprocess fixture is explicitly invoked by its passing abrupt-exit parent. |
| `cargo +1.98.0 test -p exmon-app --lib --test sessions --test incoming --test charts --test shell --locked` | 38 passed, including queued cancellation, stale recovery identity rejection, exact little-endian field recording and synthetic capture independence. |
| `cargo +1.98.0 test -p exmon-capture --locked` | 48 passed using mocks, virtual endpoints and local socket loopback. |
| `cargo +1.98.0 test -p exmon-gui --test sessions --test incoming --test charts --test shell --test inspection --test hid --locked -- --test-threads=1 --nocapture` | 17 passed. Sessions interaction ran 47 frames with maximum 21.3214 ms, below the existing 200 ms bound. |
| `cargo +1.98.0 run -p exmon-gui --example native_sessions_smoke --locked` | Passed the hidden native synthetic record, recover, export, render and close scenario within its 8 s deadline. |
| `cargo +1.98.0 fmt --all -- --check` | Passed. |
| `cargo +1.98.0 clippy --workspace --all-targets --locked -- -D warnings` | Passed. |
| `node scripts/check-architecture.mjs` | Passed six workspace layers, resolved graph and three negative fixtures. |
| `git diff --cached --check` and staged source review | Passed. |

AC-01 used independently expected synthetic CSV and JSON, including exact ns clocks,
source IDs above 2^53, raw bytes and typed literals. The tests cover inclusive time
bounds, transport, protocol and field selection, CSV quoting and formula neutralization,
legacy records without decoded literals, chart selection changes, gap handling and
preserved recording-loss metadata. Chart output contains recorded engineering points
with configured series identity and units. Live GUI averages are outside the recorded
series contract.

AC-02 exercised 2,000 observations over 32 recovery pages, pre-start and midstream
cancellation, atomic refusal of an existing or concurrently created destination,
late corrupt-record failure and temporary file cleanup. The GUI exercised explicit
raw CSV export, no-overwrite and no capture/TX effects. Export scans a single SQLite
snapshot with at most 64 records and 1 MiB serialized input per page, one output row
and one buffered writer. Output has a 1 GiB ceiling and local publication requires
same-directory hard-link support. No dependencies, product activation, hosted tests,
physical I/O, macOS or packaged release qualification.

Visual inspection covered Sessions export controls at 900x600 and 1360x900, 1x
and 1.5x DPI, in both themes. The scrollable small view shows all export controls
and moves part of the record preview below the fold. The native hidden-window
smoke verified synthetic recording, recovery and JSON publication. The screenshots
and generated synthetic files were local verification artifacts. Image SHA-256:

- sessions-1360x900-1.5-dark.png 64eb18b674cc9c86a0797ef6ff6b492cc8c06ee1f16e5d1df81fd4c304246a33
- sessions-1360x900-1.5-light.png d1ed086f6a80890645b9feff049554647a5f33c81652d88e406b42b28f4df038
- sessions-1360x900-1-dark.png 6d8f95212ea2e923e847cdd2e984e2713086ee2115b8ed23ca13e8378f2125a1
- sessions-1360x900-1-light.png 44b70bcdaaa82cc2c6e31970372a20a87c9bdbfe55fd42bd2f4f608724d7456a
- sessions-900x600-1.5-dark.png d52acfe7a9239a374da1bd48083c238924f6d457321da7477c7224533a19e2c9
- sessions-900x600-1.5-light.png 59f6b52b9431506a1d55770b84dfa3c9b2cec8afa562a5c797f507d620a0f2f7
- sessions-900x600-1-dark.png c4037f9eb86034ddb478e3cc1ef6db1a3f9a090cd3957257cb2dc19ce1903504
- sessions-900x600-1-light.png 12feb6f7c65c62350b30b91da951c4cd5e505f04e4b59a1224880951380f0f9a
- exm032-native.png c258004c2e71fe54164cab87da63356c0efe0d8e899c97e69ff2425573f3c363

Owned synthetic output, visuals and transient verification logs were cleaned after
inspection. The existing Cargo cache and unrelated workspace state were preserved.
