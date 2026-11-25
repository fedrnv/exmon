# Prototype performance gate

The Windows reference workload runs the ordinary capture, incoming processing,
chart and recorder workers, plus the production desktop shell at 1360 × 900.
The fixtures use synthetic data. The renderer uses an offscreen GPU readback
every 30 seconds. Frame latency measures UI construction separately from that
readback. The release gate runs manually on a clean source checkout.

```powershell
powershell -NoProfile -File scripts/run-performance-soak.ps1 -Mode smoke -OutputDirectory target/performance-smoke
powershell -NoProfile -File scripts/run-performance-soak.ps1 -Mode release-gate -OutputDirectory target/performance-release
```

Use a new directory for each run. The reference host uses Windows Server 2025
x64, eight logical AMD EPYC-Milan CPUs, 16 GiB of RAM, fixed local NTFS storage,
and the debug build on Rust 1.98.0. Keep the host available to the runner during
the run. The script records the actual CPU, RAM, OS, source commit and executable
hash. It samples private bytes and working set without requiring elevated access.

The release run spends 40 minutes in each of three consecutive workloads in one
process. The smoke run spends five seconds per workload and checks that the
instrumentation and fixtures work.

- Small packets use the built-in synthetic Ethernet capture and the prototype
  framing and field definitions.
- Mixed traffic adds synthetic CAN messages and two-part serial messages to the
  incoming worker. The gate checks that a serial message carries both source
  spans and a decoded numeric field.
- The chart view renders eight selected series while the chart worker maintains
  four charts, each with up to 4,096 samples per series and a 256-sample average.

Recording runs in 1.25-second bursts, with one burst per minute per workload.
The burst limit is 16 MiB and 32,000 records. The runner records committed counts,
recording drops, burst counts and finalization times before deleting each
synthetic recording. The recorder's deterministic write-worker stall fixture
simulates blocked storage separately. It checks bounded admission, dropped
records, Stop, and recovery without adding a storage delay to production code.

## Pass conditions

The release benchmark returns a nonzero exit code if any phase is incomplete,
capture sustains less than 1,024 observations per second, frame construction
sustains less than 20 frames per second, UI p95 reaches 50 ms, a single UI frame
or GPU readback reaches 1 second, or retained rows or bytes exceed production
limits. It also checks for zero backend loss, at most 10% application drops, at
most 99% display decimation at the deliberately high synthetic source rate, and
positive recording, decoding and rendering progress for the relevant phases.
The output reports capture decimation, processing output decimation, and
recorder loss separately. Recording drops from the unpaced source are retained
in the result. A storage error or failure to finalize fails the run.

The supervisor samples memory every 15 seconds in release mode and enforces
a 1 GiB private-memory ceiling. After five minutes of warm-up, private memory may
grow by at most 256 MiB above the baseline. Pass requires at least 400 samples, a
passing verdict for every workload, and a passing write-worker stall fixture.
The smoke samples memory once per second, uses a two-second warm-up and requires
at least eight samples.

Use `summary.json`, `phases.log` and `memory.csv` together as the manual
release evidence for the source revision under review. An incomplete run or
failed threshold blocks release on that candidate. A smoke result qualifies the
runner only. Physical adapter rates, actual storage-device throughput, packaged
GPU drivers, and macOS/Linux performance require their own environment evidence.
