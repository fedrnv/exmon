# EXM-035 local continuation after102

Source `82f065c508c8d17fdc0aa7947581d08689c983b5` (runtime product code unchanged by102 except benchmark cleanup).
Policy SHA-256 b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Owning local035 turn, stable published owner Codex exmon platform qualification.
Previous goal turn made progress:102 was verified Done and published1965fc8;
authoritative allocator9d4f5e2 assigned0.1.0.38. No hosted acceptance is used.

## Actual checks

All commands ran on the local Windows Server2025 build26100 host, Rust1.98.0,
Cargo.lock unchanged, jobs2, verified Zig0.15.2 GNU wrapper/cache. GNU variables:
CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER and CC_x86_64_unknown_linux_gnu =
C:/GitHub/ExMon/target/exm035-linux/zig-gnu.cmd; CFLAGS_x86_64_unknown_linux_gnu =
-target x86_64-linux-gnu.2.35; AR_x86_64_unknown_linux_gnu =
C:/GitHub/ExMon/target/exm035-linux/zig/zig-x86_64-windows-0.15.2/zig.exe ar.

- `cargo +1.98.0 clippy --workspace --all-targets --target x86_64-unknown-linux-gnu --locked -j 2 -- -D warnings`: exit0,24.32s. The original Unix cleanup lint blocker is resolved across all workspace targets.
- `cargo +1.98.0 check -p exmon-capture --features native-can,native-hid,native-usb,native-usbmon --target x86_64-unknown-linux-gnu --locked -j 2`: exit0,21.78s.
- `cargo +1.98.0 build -p exmon-app --example portable_profile_smoke --target x86_64-unknown-linux-gnu --locked -j 2`: exit0,57.53s.
- Reference toolchain llvm-objcopy --strip-debug produced the executed verifier
  SHA-2564f1951d24ccfff095970329416e747c32a71a44c46e68d5e614b56711da3d435.
  Only debug sections removed; no deadline/assertion/instrumentation changed.

The retained Ubuntu24.04.5 guest, Linux6.8.0-147-generic x86_64, booted under
QEMU11.1 q35 TCG -cpu max,4vCPUs/4GiB, existing synthetic overlay and loopback-only
SSH forwarding. No physical passthrough or product network interface was used.
Trusted guest/tools were reused from the earlier evidence. Startup took about
170s. Two initial SSH probes timed out during boot; no product command ran in
those probes. The next SSH and copy completed; no VM was restarted on timeout.

`node target/exm035-linux/guest.mjs --script target/exm035-linux/guest-committed-profile.sh`
exited0. Guest SHA matched the verifier and immutable Windows fixture. The actual
committed public settings controller example imported, validated, applied and
exported that file. Both SHA-256 values were
8f8b9c51a2a0ae2ca05c9d350ef24ba6fb126beba2e00c980ac1537edc6950db and cmp passed.
A second export to the existing path exited1 with Destination exists; cmp still
passed. Transcript emitted COMMITTED_PROFILE_IDENTICAL_AND_NO_OVERWRITE_PASS
refusal_exit=1 and OWNED_PROFILE_FIXTURES_REMOVED. This extends AC-02's real
Windows/Linux portable-file evidence, not Apple Silicon acceptance. No capture
or transmission worker was started.

Guest temporary copies, exported file and refusal log were deleted by the scoped
script. sudo poweroff completed and the owned VM session reported VM_EXIT0;
Win32 process18876 no longer existed. No owned QEMU remains. Bounded synthetic
cache/raw transcripts stay ignored; no binary/image/credential committed.
Reviewed documentation diff only updates the stale lint blocker and adds the
actual committed-verifier result. git diff --check passed.

## Completion and dependency audit

Actual Epic001 membership is001..039 plus048/049 and corrections101/102, through
accepted append-only membership events. All except035,036,037,038,048,049 project
Done.035 remains In Progress with its published owner.036 is Planned and needs
the declared physical devices/peers; none is supplied.037 is Planned and
functionally depends on035.038 depends on035/037.048 depends on035/036/037/038;
049 depends on048. No dependent item is free to claim. No safe independent
Epic001 implementation remains after the available local checks recorded here.
No full gate, independent review, Epic closure or035 version is issued.

The remaining external condition is access to the required local target/stand:
- Apple Silicon macOS is not connected or declared. Actual local GUI/profile,
  native BPF permissions/fidelity/Stop and matrix sign-off remain unverified.
- Linux full local GUI gate has existing11 failures and2 incomplete cases under
  TCG/llvmpipe, despite debug/release, Vulkan/OpenGL and default/maxCPU comparisons.
  Do not retry unchanged failed scenarios or relax limits. A sufficiently fast
  local Linux target is needed. Current Windows HypervisorPlatform is Enabled,
  installed NoRestart with RestartNeeded=True; host last boot2026-11-02T08:01:08Z,
  before that installation. No host restart occurred; prior WHPX probes failed.
  A coordinated owner restart may permit a changed acceleration probe, but does
  not promise success or replace the missing Apple target.
- Physical RS-485/USB/CAN declared stand is still unavailable for036.

Shortest owner action: supply the local Apple Silicon/Linux target connection
or coordinate this VPS restart for Linux acceleration, and provide the declared
physical stand when ready. A concise target/access question is pending; elapsed
time is not permission. After target access, resume this035 owner and run only
remaining affected local qualification, then proceed037/038/048/049 once actual
dependencies and authority permit.036 uses its own fresh turn when stand ready.

This is the first consolidated external-blocker checkpoint after102. The goal
remains active; blocked status requires its separate repeated-condition audit.
All hosted diagnostic observations stay excluded by035 amendment4d5ea307.

## Local raw artifact hashes

- `local-full-gnu-clippy-after102.log`: `fec39811905a5a9d483d4c6a9b64177817bb0bd7a8071197074a260f5abb6b86`
- `local-native-check-after102.log`: `bdbfd5fda49bdcb941d14c645a0f39eaa95b579330f779462ae4d13e3d887906`
- `committed-profile-build.log`: `57828fb5322481fbdf0e0d1401fa36dc0158d8f518fadebe598260673407a45d`
- `committed-profile-runtime.log`: `53d847e2e17db4f897715d30e511fd64548eebf2104863c722eb652315da151c`
- `guest-committed-profile.sh`: `65b921fe276f85bd992a6d6f143f869a5e57b4c3c9aba6a072728a3c398ebe9a`
- `desktop-serial-profile-committed.log`: `1605b8d59d3ff7bc4a2730bbfdbee400eb663afbf744593dfc9a902df78ce3ef`
- `committed-portable-linux`: `4f1951d24ccfff095970329416e747c32a71a44c46e68d5e614b56711da3d435`
