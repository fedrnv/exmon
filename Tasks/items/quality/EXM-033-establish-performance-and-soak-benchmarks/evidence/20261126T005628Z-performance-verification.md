# EXM-033 performance verification

Tested published source 957b2acae51673efadaa9ea0bb805ce4aac881c2 with FDTasks 5.7 policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8. The clean reference checkout ran on Windows Server 2025 Datacenter build 26100, x64, eight AMD EPYC-Milan logical CPUs, 17,169,162,240 bytes RAM, local NTFS, Rust 1.98.0 debug build. Executable SHA-256 8b345a4eebeece12af91621d80818d030d51ce895ce2170fe6296483fd1bc913.

| Local check | Result |
| --- | --- |
| `powershell -NoProfile -File scripts/run-performance-soak.ps1 -Mode smoke -OutputDirectory target/exm033-smoke-final` | Passed, three 5 s phases, supervised duration 17.644 s, 16 memory samples. |
| `powershell -NoProfile -File scripts/run-performance-soak.ps1 -Mode release-gate -OutputDirectory target/exm033-release` | Passed, 7201.303 s wall time, three consecutive 40-minute phases, exit 0. Both supervisor and benchmark exited. |
| `cargo +1.98.0 test -p exmon-gui --example performance_soak --locked` | Passed the focused negative gate test for short/missing phase coverage, missing chart results, frame stalls and backend loss. |
| `cargo +1.98.0 test -p exmon-app --lib --locked sessions::tests::full_data_queue_stop_and_stale_token_are_independent_of_store_progress -- --exact` | Passed the deterministic storage backpressure fixture in the release runner. |
| `cargo +1.98.0 fmt --all -- --check`, `cargo +1.98.0 clippy --workspace --all-targets --locked -- -D warnings`, `node scripts/check-architecture.mjs`, `git diff --cached --check` | Passed. Staged implementation reviewed. |

| Phase | Time (s) | Captured | Captured/s | Processed/s | Frames/s | p95 UI (ms) | Max UI / GPU (ms) | Backend loss | App drops | Display decimation | Processing output decimation | Chart samples | Commit count | Recording drops | Bursts | Max Stop (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Small Ethernet packets | 2400.006 | 28,765,664 | 11,985.7 | 5,748.6 | 44.9 | 30 | 166.542 / 536 | 0 | 0 | 14,968,512 | 832 | 4,893,504 | 296,125 | 422,502 | 40 | 81 |
| Mixed and fragmented | 2400.001 | 28,774,688 | 11,989.4 | 5,770.1 | 44.6 | 30 | 99.452 / 626 | 0 | 0 | 15,086,720 | 378,939 | 4,928,494 | 291,834 | 406,957 | 40 | 60 |
| Chart-heavy | 2400.017 | 28,829,248 | 12,012.1 | 5,019.9 | 39.2 | 30 | 152.625 / 601 | 0 | 0 | 16,781,088 | 192 | 5,004,032 | 310,176 | 382,210 | 40 | 79 |

All phases had 80 GPU readbacks, a maximum of 1,024 retained incoming rows and
1,210,368 retained bytes. The mixed fixture submitted 107,096 serial fragments
and observed a complete two-span message with a decoded numeric field. There were
479 memory observations at 15 s cadence. Peak private memory was 395,534,336
bytes and peak working set 108,392,448 bytes. Private memory grew by 7,581,696
bytes over the five-minute warm-up baseline.

AC-01 passed with separate capture backend, application, display, processing and
recorder loss measurements. The deliberately unpaced generator caused recording
drops during the short bounded bursts. The storage stall is a deterministic
blocked-worker fixture. Neither observation claims sustained physical adapter or
slow-disk throughput. AC-02 passed with fail-closed release verdict checks.
The later release task must use the manual reference gate for its own frozen
source. No Windows packaged-driver, physical device or macOS/Linux qualification.

Canonical output SHA-256 values before cleanup:

- `summary.json` 3c262f4ca44c960c6d9e84215c0928ba12e5d94bc8a2432c3cd48e87a8f1d763
- `phases.log` 8923731ba6734661bf151a544cd758deed06e6e15880a125cf7b99b05ac832ad
- `memory.csv` 07fed4a407b9a66899cd86405b112496334fb0bec47ac9bc7c7c318f4c8028f7
- Empty `child-errors.log` e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855

The earlier smoke failures and corrections are recorded in the published
candidate verification event. Owned synthetic files, logs and transient run
checkpoint were cleaned. The existing build cache and unrelated state remain
available for subsequent work.
