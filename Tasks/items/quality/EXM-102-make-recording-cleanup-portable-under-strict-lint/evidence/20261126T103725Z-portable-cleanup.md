# EXM-102 local verified closure

Source: published claim/base `287c3daedc0f98fa0634503551fd37c01a00e1ab` plus the sole source diff in
`crates/gui/examples/performance_soak.rs` (canonical LF SHA-256 `5974ee8777725aed2b8add8ce5992584c3188cd9efe4751e21cb861ce0c661c0`).
Policy tasks_file_structure_v5_7.md SHA-256
b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Rust1.98.0 (88d9e12ae), LLVM22.1.8, local Windows Server2025 build26100,
8 logical CPUs, fixed local NTFS. GNU cross target uses qualified local Zig0.15.2
wrappers/cache from035 (x86_64-linux-gnu.2.35). Cargo jobs2. No hosted tests.

## Acceptance and commands

- AC-01 PASS, exit0: `cargo +1.98.0 clippy -p exmon-gui --example performance_soak --target x86_64-unknown-linux-gnu --locked -j 2 -- -D warnings`.
  Set CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER and CC_x86_64_unknown_linux_gnu to
  C:/GitHub/ExMon/target/exm035-linux/zig-gnu.cmd, CFLAGS_x86_64_unknown_linux_gnu
  to -target x86_64-linux-gnu.2.35, AR_x86_64_unknown_linux_gnu to
  C:/GitHub/ExMon/target/exm035-linux/zig/zig-x86_64-windows-0.15.2/zig.exe ar.
  Existing immutable local negative baseline records exit101 for unused deadline
  and never_loop before this fix; no lint allowance was added.
- AC-02 PASS, exit0: `powershell -NoProfile -File scripts/run-performance-soak.ps1 -Mode smoke -OutputDirectory C:/GitHub/ExMon/target/exm102-smoke-20261126T103700Z -Toolchain 1.98.0`.
  The existing supervisor built the example and ran the exact slow-storage fixture
  `sessions::tests::full_data_queue_stop_and_stale_token_are_independent_of_store_progress`
  (1 passed). All3 actual Windows synthetic workload phases passed; recording
  stop42/40/49ms.16 memory samples,17.921s, peak private391176192 bytes,
  growth35258368 bytes, below unchanged1GiB/256MiB limits. Child exit0 and
  summary.passed=true. The published claim revision in summary is paired with
  the modified-source hash above; no clean release source is implied.
- AC-03 PASS, exit0 each: `cargo +1.98.0 clippy -p exmon-gui --example performance_soak --locked -j 2 -- -D warnings`;
  `cargo +1.98.0 fmt --all -- --check`; `node scripts/check-architecture.mjs`;
  `git diff --check`. Architecture: six workspace layers, resolved dependency
  graph and three negative graph fixtures passed.

## Source review and limits

Reviewed the complete diff: only the recording_finish helper changed (13 additions,
7 deletions). Non-Windows directly returns one remove_file result. Windows retains
exact same deadline Instant::now()+500ms, error32 guard with Instant::now()<deadline,
5ms sleep and direct return of every other result. Recorder metrics/finalization,
threads, resource limits, supervisor and dependencies are unchanged. No artificial
retry, dummy variable, allowance or weakened test. Conditional compilation is the
smallest natural boundary; no mirror-only unit test added.

All commands ran locally in the owning102 development turn. PowerShell5.1 logs
wrap normal Cargo stderr as NativeCommandError text; recorded process exit0 and
Cargo Finished lines establish success, not that wrapper label. Cross checking
is compile/lint evidence only: no Linux/macOS runtime or new2h soak verdict.
Original033 two-hour evidence remains tied to its original source. Physical
interfaces were not opened for this synthetic example.

Owned-resource cleanup: child exited; no recording database or SQLite sidecars
remain in the smoke directory (only summary, phases, memory and empty child-errors).
No VM or background fixture was started. Ignored target cache/raw logs/executable
remain local; none are committed. Git diff contains only the permitted helper
plus new102 immutable records and first closure request. No existing records edited.

## Evidence SHA-256

Committed snapshots are UTF-8/LF transcriptions with trailing whitespace removed of the local raw artifacts.

- `20261126T103725Z-architecture.txt`: `bfa394b6ca7acdbecc48cd78341f625fe872d0655d5be2feda549bf69c34ae6e`
- `20261126T103725Z-diff-check.txt`: `5eadbab2f46f9ed332757c6024e379f45a919981f16c60c88f3e021ef68a73b3`
- `20261126T103725Z-fmt.txt`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
- `20261126T103725Z-gnu-clippy.txt`: `2b8fc0c9ae9b9b2631e659b567a3291f25ecef0fad1d2f9db5e27c3570a4929b`
- `20261126T103725Z-memory.csv`: `18d9fd2d7a017229c27132971a3c22a3ed154d206b913c8a598724b3e76ebab1`
- `20261126T103725Z-phases.txt`: `1cb99c75678bdff17c1b1622f879fd7028213d9af470bd906f6e1c09183d1a9d`
- `20261126T103725Z-smoke-supervisor.txt`: `d22b754d2c69b9b08e4e599ed9dfa524ad295c9f7531a8e159cc28ae83e61d42`
- `20261126T103725Z-summary.json`: `da7c98278e34b8beba3a06de17aff3aa3da686499e2b2c39173fec3171bb2e61`
- `20261126T103725Z-windows-clippy.txt`: `41221264bc4b516dbed3813f9aa8323103ac78736ab9d1e1112b7f260f64c841`

Original raw artifact hashes (retained under ignored target directories):

- `20261126T103725Z-architecture.log` original local raw bytes: `4f389e2d9cdf167261ce323cd3ff4bfc6fb9335b6d69cac66fc14320ab8eaece`
- `20261126T103725Z-diff-check.log` original local raw bytes: `4a5f3fb23acd8580c73b1eb1ce68ef7b71a47c68baf9a9b7311c42386a85653c`
- `20261126T103725Z-fmt.log` original local raw bytes: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
- `20261126T103725Z-gnu-clippy.log` original local raw bytes: `ae648af60455ccef7d7f17e37f28ebd7c5e74eb4ebbba2437f76929245ea8d08`
- `20261126T103725Z-memory.csv` original local raw bytes: `94dfeae5ffa57a5237ce373618c4d00bd925b20d32edb29fe4476f579a97ae51`
- `20261126T103725Z-phases.log` original local raw bytes: `1cb99c75678bdff17c1b1622f879fd7028213d9af470bd906f6e1c09183d1a9d`
- `20261126T103725Z-smoke-supervisor.log` original local raw bytes: `bcbf3dbec3d97c3d6acb5e4f75b5c0a784961fc0430029d8ddbfcae12f589988`
- `20261126T103725Z-summary.json` original local raw bytes: `198fb224b307127ce02d7432b9d12cb99a4e3d0108f4d9e60c4afa0fa26df7b9`
- `20261126T103725Z-windows-clippy.log` original local raw bytes: `3bb645b28054bdfa7561008091903a07d0d90b008deceb3a60fc4a54614915a5`
