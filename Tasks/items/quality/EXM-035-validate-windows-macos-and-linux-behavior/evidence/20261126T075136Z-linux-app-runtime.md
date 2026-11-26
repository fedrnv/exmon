# EXM-035 remaining Linux application execution

Source: 58f5eb938998e6ee9d5dffd4f5f430be2c7da3b6, including verified correction EXM-101 at 9b53499.
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.

The current Linux application test targets were cross-built with Rust 1.98.0,
committed Cargo.lock and the previously verified Zig 0.15.2/LLVM tools. Command:

```text
cargo +1.98.0 test -p exmon-app --target x86_64-unknown-linux-musl --locked --no-run --message-format=json
node target/exm035-linux/run-resume.mjs
```

Target compiler/linker flags retain the musl triple, rust-lld linker flavor
and PIC Zig ubsan_rt/compiler_rt archives documented in the prior Linux evidence.
No runtime instrumentation or test deadline was disabled. llvm-objcopy
--strip-debug reduced only debug sections for the read-only vvfat test share.

All 59 remaining application tests passed in ten executable suites. The guest
printed each binary SHA-256; all ten matched the manifest, followed by
EXMON_ALL_PASS and VM_EXIT 0. Each executable used --test-threads=1.
The runner had a 600-second external watchdog. Actual guest runtime was about
106 seconds including boot. The QEMU child powered off cleanly.

The environment remains QEMU 11.1.0 q35 TCG, two vCPUs, 1 GiB RAM,
Alpine 3.23.8 / Linux 6.18.60-0-lts, read-only ISO and vvfat share, no NIC or
physical passthrough. Only guest loopback was enabled.

The other four application test executables (63 tests) passed on this source
in the EXM-101 evidence. Together these results cover all 122 current app tests.
The unchanged domain/protocol/capture tests (142) and storage tests (48) retain
their earlier source-bound evidence. This does not establish graphical,
packaged, physical-device or Apple Silicon compatibility.

## Executed artifacts

| Target | Guest binary | SHA-256 |
| --- | --- | --- |
| shell | test00 | bb0de77c7d6ea3e3b9b178550aac2889b0107b2e887a09616cea42da9763046d |
| incoming | test01 | ca325ae05bcd731377d24b473af54d690fef0b0811514ac52e1dacd675306d34 |
| charts | test02 | 909ad278cd8450c322f6f2737f9701714423e46484c404a52cbc57cac1eca043 |
| diagnostics | test03 | 72674305497f11cf59c05d7c60392979fcd34358cb13e54c6e930c3dc83763d4 |
| sessions | test04 | e4943e1ad1ec3dfcab13dd6926f8cc4384bd71951ee1fb0b52bd24cd7a0cde4e |
| parser_campaign | test05 | 21560462c2129ac678c630af12bac44e6363af6b113a908bda40ea203fd6416c |
| hid_worker | test06 | 1f10f8a75e13d76ca72d3c1388c9efed363ee8aa6ef2b1beb33a90814325c378 |
| inspection | test07 | 107535a46b362e8a9acf5ae88c5227d6809b59bf965f08c9503053976a27af5e |
| network_filters | test08 | 4aa23e36e7f982b7adab021c861bdfc48c4e22faf1dc86886b536640e35d8d44 |
| settings | test09 | 251a29d4e86ea9f8ea38c6b24295c75f053bb3c9e0f8da959d8a6edb011841dd |

## Local evidence hashes

- `resume-build.jsonl`: `83803f0bc59df93200256cee217159b5e9d2757bf883b1eca0b770cd7a3d509f`
- `resume-binaries.json`: `30825aa79eb72a1787bf35e07d855a461fd13c8b9f807d21c3d44bb8bbe34fb5`
- `resume-serial.log`: `bceec1901d1e50bb4004072d0c531d9fcf25a144c7ebf414d18d8e704b996e0d`
- `resume-runtime.log`: `8c62152b57b7f33a5c97231ef8ac55fc387c9d5f6b69da4e5d203201271e8853`
- `run-resume.mjs`: `46b57d49c87aae4e618690e99e2639d0caf7666469313304fb9541cc777af4d7`

The bounded synthetic transcripts/toolcache remain ignored under target/exm035-linux.
No generated executable, disk image, credential or build output is committed.
