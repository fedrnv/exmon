# EXM-035 Linux runtime progress

Source 6c1e59a0754e636ae3ff0e20c9cdae714bb42b42, Rust1.98.0, Linux x86_64-musl. Policy SHA-256
b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.

The actual Alpine3.23.8/Linux6.18.60-0-lts guest ran21 domain/protocol/default
capture test executables. All142 tests passed, including local socket loopback,
then EXMON_ALL_PASS and VM_EXIT0. QEMU11.1.0 used q35 TCG,2vCPU,1GiB RAM,
read-only ISO/vvfat and no NIC or hardware passthrough. Each guest SHA matched
the manifest. No physical host or guest interface was used.

Built with cargo +1.98.0 test -p exmon-domain -p exmon-protocol -p exmon-capture
--target x86_64-unknown-linux-musl --locked --no-run --message-format=json.
The target linker was the trusted Rust toolchain rust-lld.exe, target RUSTFLAGS
-C linker-flavor=ld.lld. node target/exm035-linux/run.mjs booted the ISO/kernel,
logged in to the isolated guest, mounted read-only test binaries and ran each
with --test-threads=1 under a300s watchdog.

The next group cross-built23 storage/app test executables. SQLite C compilation
used verified Zig0.15.2 CC with -target x86_64-linux-musl and matching target
CFLAGS, plus Zig ar. Zig UB instrumentation required its PIC ubsan_rt and
compiler_rt archives linked through target RUSTFLAGS. No sanitizer was disabled.
Rust llvm-tools llvm-objcopy --strip-debug produced206,734,953 bytes of guest
binaries within vvfat capacity. The build command was cargo +1.98.0 test
-p exmon-storage -p exmon-app --target x86_64-unknown-linux-musl --locked
--no-run --message-format=json. The local runner used a600s watchdog.

The actual guest stopped after14 suites,81 passed and1 failed, at app outgoing
interface-loss readiness. No physical bytes were submitted. Remaining app
suites were not executed in this run. EXM-101 records the precise finding.
This is partial qualification, with no035 closure or version request.

Acquisition trust followed the previously accepted exact upstream versions.
ISO SHA-256 5d5b4fc2244b25170e915c05141b259fec5350499928eab21c4dd34b24e0dbe4.
QEMU installer SHA-512 5bcf9eed634e8575a37b74f445af41a2fe4106da512d0c30c368301d4c105037fdfab40a5287367a28a957624cddebbc8c07e16c88ab6634f554cdf3d16bf543.
The installer was extracted by existing7-Zip, not executed. Zig ZIP SHA-256
3a0ed1e8799a2f8ce2a6e6290a9ff22e6906f8227865911fb7ddedc3cc14cb0c
matched the current official ziglang.org/download/index.json.
Rust llvm-tools was installed through rustup for the existing pinned toolchain.

Hashes of bounded local evidence retained for correction resumption:

- serial.log fcf65ca2707d874f49b3a39db1562c3e8612b11dcaaa75a6ffd82ee486a0298d
- binaries.json da99b158130c563b0a7a471a68ffa492562c2aa9c244f8412dd7fb06479d6c95
- app-serial-pass.log 2495daf386218a1642158e4c4edb41a8ac683a93d923feabd7170102c6dedc75
- app-binaries.json b73073d5f1063aa9ea97d96d191eac584948bf100c938c2f6ae7ddb9dfe26fac

Initial cross-build recovery corrected the Zig target argument, linked required
UB runtime archives with PIC and included compiler_rt for __extendxftf2.
The first app VM could not mount oversized debug binaries; stripping debug
sections corrected the input size. Actual app runtime then exposed the finding.
No unchanged functional failure was retried. Both VMs powered off and no QEMU
process remains. Toolcache and bounded synthetic evidence remain ignored under
target/exm035-linux for reuse. Linux GUI/packaging/macOS/physical results pending.
