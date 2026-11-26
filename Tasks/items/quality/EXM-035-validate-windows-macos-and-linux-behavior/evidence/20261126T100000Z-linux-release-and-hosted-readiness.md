# EXM-035 optimized Linux comparison and manual native-runner preparation

Source: 412bc9f02cf9cf43a86a8012c3ad452beb1a2f4f; runtime product code remains EXM-101 at 9b53499.
Policy SHA-256: b7c6929e7bbf540ec19cf23beeca24d47030c7654ba5501e019ce5938b137dd8.
Reference Rust 1.98.0 and Cargo.lock. Qualified environment/acquisition versions
remain those in the preceding035 evidence. No physical passthrough or host
control-interface capture/transmission occurred.

## Actual Linux comparison

The native_shell_smoke Linux GNU release build completed in21m12s. Seven other
native examples and twelve GUI test executables were then built in the same
release profile. These use the standard release profile; no assertion or test
source changed. Zig0.15.2's linker emitted a deprecated optimization-setting1
warning. It is retained as a tooling limitation, not a clean lint verdict.
The all-examples debug build's independent performance_soak unused-variable
warning still requires a separate source correction.

The prepared Ubuntu24.04.5 / Linux6.8.0-147-generic VM retained4vCPUs/4GiB
and used q35 TCG. Default CPU inventory exposed llvmpipe LLVM20.1.2,128 bits;
-cpu max exposed AVX2 and llvmpipe256 bits. A separate paused QMP probe verified
max CPU features. The first machine-none capability probe required a q35
machine to initialize APIC state; no OS or product ran in that failed probe.
Vulkan and OpenGL outgoing rendering were compared. All original8-second
native example and10-second egui_kittest GPU wait limits remained intact.

Four distinct native examples passed: shell on128-bit Vulkan, and HID,
incoming and packet inspection on256-bit Vulkan. Their original success
assertions, screenshot save and normal Close completed. Charts failed on both
CPU models. Outgoing, sessions and settings failed their screenshot-completion
conditions on256-bit Vulkan. No failed example is counted passed.

The GUI source has28 cases including its chart-clipping unit test. Across the
attempted native Linux release regression cases,15 had explicit success,
11 had explicit failure, and2 visual cases did not complete before the120s
external binary watchdog (inspection and sessions visual matrices). The initial
outgoing Vulkan failure was reproduced with a different OpenGL backend and
then max CPU; these comparisons are not counted as additional unique cases.
An extra inspection-pause diagnostic on max CPU exposed its exact failure at
the existing FPS/max-frame assertion, after the record-progress assertion.
The11 diagnosed failures therefore comprise6 FPS checks and5 GPU readback
wait failures. The2 incomplete cases remain unknown, not passed or reclassified.

The full virtual-session GUI scenario passed in61.97s. All3 default HID GUI
cases passed, including explicit selected-collection sending, recording/recovery
controls and unsupported default native discovery. Other observed passes cover
file import/export/no-overwrite, selection/search/navigation, frozen filters,
invalid input feedback, explicit runtime-limit files and saved split/theme restore.
The per-case outcomes JSON is hashed below; no overall GUI gate passed.

Successful screenshots were visually inspected: decoded sample telemetry,
field values, unbound TX/disabled schedule, synthetic HID RX/TX completion and
the Ethernet/IPv4/UDP inspector tree/selected HEX bytes were readable. Partial
visual-matrix images also show real numeric chart series with dashed MA8 in
light theme at1.5DPI, and recovered session raw/export controls in dark1.5DPI.
These images are partial observations, not a passing complete matrix or FPS
verdict. Layout-saving-unavailable notices belong to explicitly nonpersistent
fixture Shell instances. The production normal-window-close probe is in the
preceding desktop evidence.

All baseline/max VMs were shut down through sudo poweroff and owned QEMU
processes exited0. The baseline remainder was deliberately stopped for CPU-model
comparison while network_filters was starting; that interrupted attempt is not
counted as a test pass. The subsequent max run exercised that binary fully.
Ignored synthetic tools/cache/transcripts remain available for resumption.

## Windows host acceleration preparation

Windows Server2025 Datacenter10.0.26100 reports virtualization/SLAT capability.
An isolated paused WHPX machine probe returned No accelerator found and failed
to initialize. HypervisorPlatform was Disabled. The explicit owner authority at
EXM-008/events/20261123T091743Z-codex-owner-amended-d9f16d1424bef5f78fabeb8823e23fc1.md
permits installation of everything needed for development on this VPS.
Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform -All
-NoRestart succeeded, State Enabled, RestartNeeded True. No restart occurred.
The immediate paused WHPX re-probe still returned unavailable. Its generic
No space left on device initialization message is not a disk-capacity diagnosis;
host free space is recorded in the hashed host-state JSON. Activation needs a
coordinated restart/re-probe or a separate native target. No successful WHPX
runtime is inferred. Hosted qualification can proceed independently.

## Concrete manual hosted verification

Connected GitHub reports public fedrnv/exmon with admin access. The existing
repository authentication verified Actions enabled via GET/actions/permissions.
No account/token was acquired, printed or persisted; the existing credential
helper is consumed only inside the local authenticated API process. Redirects
are refused. The connector has no new-workflow dispatch method. The next run
uses explicit workflow_dispatch through this existing repository authority.
No push/PR/scheduled test trigger, paid runner, release or deployment is added.

The official current runner reference identifies standard ubuntu-24.04 x64 and
macos-15 arm64 runners. The manual workflow and bounded supervisor prepare
actual native tests, source/OS/architecture/graphics identities, full outcomes,
synthetic native examples and developer install/startup/uninstall probes.
Artifacts exclude binaries, credentials and serials. Existing test deadlines
are unchanged. Omitted debug symbols/incremental output bound hosted disk use;
default debug assertions and optimization remain. Known lint failures remain
failures. Dependent steps are recorded not-run if the workspace build fails.

The public-controller portable_profile_smoke example reproduces the earlier
ignored harness without product API/transport changes. It validated/applied and
exported the immutable Windows fixture on Windows, yielding the same
SHA-2568f8b9c51a2a0ae2ca05c9d350ef24ba6fb126beba2e00c980ac1537edc6950db.
A second export to that existing path was rejected; bytes stayed unchanged.
The fixture's -text attribute preserves its actual Windows-exported bytes.
Focused cargo1.98 clippy -p exmon-app --example portable_profile_smoke --locked
-- -D warnings passed; cargo fmt --all -- --check and git diff --check passed.
PyYAML6.0.3 static validation confirmed workflow_dispatch only, trusted main,
fixed runners, read-only permissions and40-character action pins. Python AST
validation passed. The source workflow/supervisor were reviewed for bounded
owned processes, individual failures and synthetic-only evidence collection.

Hosted runtime results remain pending until explicit dispatch. Native Mac BPF
fidelity/permissions and physical EXM-036 obligations are not inferred from
availability of a runner. EXM-035 remains In Progress; no version request.

## Local evidence hashes

- `gui-release-build.log`: `53d8e7c97e27e7686e0688598625c9f109ddabe788fb61a3837257e92730b86d`
- `gui-release-remaining-build.log`: `b19b1c56346cfda272d83f3c87a92f9f884a6ef01d9e89e34bf5c57d7fa2c8d7`
- `gui-release-tests-build.jsonl`: `f8359fa4f0bbce2cb6f4fe637c1ea05a66ec6c3d9f726655c86db871c85b56ac`
- `release-smokes.json`: `7d8c6e1925bd148f25ae4181bb4cf10cf1a050101761d21dc36fa5e17b550220`
- `release-gui-binaries.json`: `3a22ac4124d67037fa91f5409b7f225f1cef23a23fa91448a0e237f93666f606`
- `release-shell-runtime.log`: `9ff586cb6164407dcf1fa6b300990557e8674adbb2a7c9dfa4c6c16287f2a0e0`
- `release-remaining-runtime.log`: `8cfde33e958f43ff572b1e5bad39649a5122430f6bab96715b01866f4076d4c7`
- `release-gui-tests-runtime.log`: `4b8af463c40dfaa929726e9528925cc18e9e04bca8dd2b2d56d3d8a31714dc69`
- `release-gui-tests-remainder-runtime.log`: `b1cc4957b55d6aaf63f9d70c257250106550118889143726fd16014d2b4eb896`
- `release-gl-outgoing-runtime.log`: `6e6e7bddf76ff3e57883bee970c38cda4669334ac654db440b12834181a0c305`
- `release-max-outgoing-runtime.log`: `2c141aa9924e9b9ce8c1c696fde70baedd584e51155de2949c7a674ec418caa2`
- `release-max-other-smokes-runtime.log`: `ee6841c52d5b60bf25793282d704a82106998bf9f164fe22b3c018e3967cbbe9`
- `release-max-charts-runtime.log`: `9b93c627952f73e7e09b0075d53f6f022f75a931e5c1c055a7cc3a35742a78df`
- `release-max-gui-remainder-runtime.log`: `d9e67d934eadb64b371fd929c967f43ea88f9c816554eefeda247306e2605c4c`
- `inspection-pause-diagnostic.log`: `8eee86d78a196df847ffcf5089df928e9d810ab2870f51255f771e8d5e149096`
- `release-gui-outcomes.json`: `cf0990fc95cdde7e05967b5d0656da50d8023191d5ba2a5b6a0f626cb9977526`
- `graphics-inventory-runtime.log`: `a27935232f794740edf4a1f2a55ea7275f306de6a7c458e6079b26e9e67d8a09`
- `max-graphics-inventory-runtime.log`: `a1f5185c387cb155fffa781ff40a03edd26bde85c8cf578971e360b1a94e205b`
- `max-cpu-query.json`: `30bdbc12613f8e133e7aabe5fa404ef6265abd990003ac56da0b52551fa2cd07`
- `whpx-probe.log`: `cd46879201acbdf85584c9956f069a2c264736c4bcc26742a6d902a90872efc0`
- `whpx-after-feature.log`: `269064c945e908def2c0a31ec3e968c35727c77938363d092b64eec195b93047`
- `enable-hypervisor-platform.log`: `93d5e3a93b4e1bed801c3d116656062c27e55c153df9d8ef2b42116624cdfae8`
- `host-hypervisor-state.json`: `e5ee88d442efddce030c097cc74bbff7d44c4f432f3cefeb0d26412c0a47877c`
- `portable-no-overwrite.log`: `79951002b066c39fdbf71a432c31fcf5939e4c18e6638f86d68c67eed29e9ebc`
- `portable-reviewed-windows.json`: `8f8b9c51a2a0ae2ca05c9d350ef24ba6fb126beba2e00c980ac1537edc6950db`
- `exm025-native.png`: `50bd349683e1b380da448526a693c47eb3e5f67c450405132149c1bc24fe7fe2`
- `exm014-native.png`: `4a958b059ee19a90199f081e1ed4b7814b4a1f8184890144f772d500846fbd52`
- `exm026-native.png`: `00704f216bf14608a3de74786b013891087788ec30c184ae74787657315749d4`
- `exm030-native.png`: `844cd59fe55cb602fbb52d60dd1c7eb08365ff4e2c7705921f66689d083e527d`
- `charts-900x600-1.5-light.png`: `4eaadde5d0f14a773d8d0238781117c56bcdceb967c03b7c3399688b933f900a`
- `sessions-900x600-1.5-dark.png`: `1aa86aa6a468dc0cf05b71e261819599919df32a02deb880b3ae53f2a5674c9f`
