# EXM-101 verification

Published claim 418d6753d650a588c88ed06b67414cb30396de2e, tested source tree 884c863da06d96d4f59eb06f6885c6a7b765699c.
Policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Windows Server 2025 x64 and actual Alpine 3.23.8 / Linux 6.18.60 QEMU x64,
Rust 1.98.0 debug builds. Linux toolchain and isolation match 035 retained cache.

AC-01 passed. The deterministic unit test holds editor publication control while
the underlying TX worker closes. It asserts that the cached ticket stays present
but ready() rejects it, with zero mock submissions. It failed on the original
source and passed after correction. Editor readiness now checks the same atomic
worker authority and ticket identity as send/schedule admission. Source-validation
interface-loss revocation precedes terminal history publication. Terminal
Disconnected, PermissionDenied or backend failures revoke authority before
publishing their outcome. Existing late leased completion semantics remain.
The direct integration case checks both unavailable metadata and Disconnected
returned during submission, with exact zero/one backend calls.

AC-02 passed. Windows command
cargo +1.98.0 test -p exmon-app --lib --test outgoing --test transmission
--test scheduling --locked -- --test-threads=1 passed 63 tests
(16 unit, 12 outgoing, 20 transmission, 15 scheduling). Partial and unknown results,
Busy explicit retry, Stop, invalid revisions and schedule counts pass unchanged.

AC-03 passed. The same four test targets cross-built for
x86_64-unknown-linux-musl and executed inside the isolated guest, with 63 passes,
EXMON_ALL_PASS and VM_EXIT0. All four guest SHA-256 values matched the manifest.
Windows cargo +1.98.0 test -p exmon-gui --test outgoing --locked
-- --test-threads=1 passed two production UI tests. Final fmt check, workspace
Clippy with -D warnings, architecture and staged diff checks passed.

One preliminary Linux run exposed a separate observation unwrap in the outgoing
test's wait closure. mock_status uses nonblocking access and may return None.
All affected wait predicates now await Some with their original exact count and
four-second bound. No application retries, sleeps or test deadline increases.
The final corrected candidate passed; the earlier failure remains recorded here.

Linux build command used cargo +1.98.0 test -p exmon-app --lib --test outgoing
--test transmission --test scheduling --target x86_64-unknown-linux-musl
--locked --no-run --message-format=json. Target linker was rust-lld with
-C linker-flavor=ld.lld and existing 035 PIC ubsan_rt/compiler_rt link archives.
SQLite used verified Zig 0.15.2 cc/ar and exact x86_64-linux-musl CFLAGS.
Rust llvm-objcopy --strip-debug prepared read-only guest copies without changing
test code. node target/exm035-linux/run101-final.mjs booted 2 vCPUs / 1 GiB q35 TCG,
no NIC or physical passthrough,600 s watchdog, each binary --test-threads=1.
VM powered off and no QEMU remains. No new dependencies, GUI layout or stored
schema changed. Applicable qualification and Epic gates still remain.

Final local evidence hashes before cleanup:

- Linux serial d1d7eb4976f3f982bd7716e10e2a7dfa0f8c02acdc891f062048bcd7103b9a9e
- Linux binaries manifest 63fb37f30ac391720fcf7a1b30eeeafb91f0f784d4feb054e1c8daa1ddcc247d
- Windows app 241f0b5023d98d8c76c8c5d4b13d04f7e036fcba5c955efca88c16724d07f618
- Windows GUI d6912bbc768981a1df9c19aaa374e2494e1841f1bb8d7e13a7d2ca3f645a2667

The 035 Linux cache and bounded failed/current manifests remain ignored for
qualification resumption. Owned Windows logs and screenshots cleaned.
