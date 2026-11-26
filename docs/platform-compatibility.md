# Platform qualification

EXM-035 remains in progress. The reference source is the published virtual
prototype at 95068a1 with the outgoing readiness correction at 9b53499,
followed by task/version records. The Windows development
qualification covers the default synthetic desktop and loopback boundaries.
The full supported-platform acceptance requires runtime evidence on the target
systems below.

| Scenario | Windows x64 VPS | Linux x64 | macOS Apple Silicon |
| --- | --- | --- | --- |
| Default synthetic shell, capture, editing, charts, Stop and shutdown | Verified by EXM-039 native smoke and production GUI tests | Partial local release screenshots and manual Stop/close verified; full local GUI gate pending | Pending local target access |
| Configuration schema, examples, roundtrip, disabled schedules | Verified shared JSON tests and export for Linux handoff | Current configuration/settings tests and exact Windows file import/export passed | Same source contract, target runtime pending |
| TCP/UDP socket traffic | Verified local IPv4/IPv6 loopback | Current Linux QEMU local socket tests passed | Pending target runtime |
| Serial and CAN virtual fixtures | Verified deterministic adapters | Current Linux virtual fixture tests and native vcan RX/TX passed | Pending target runtime |
| USB/HID mock selection and explicit Send | Verified production worker and GUI | Historical emulated HID/USB evidence | Pending target runtime |
| Native HID feature build and mock UI | Current Windows check recorded under EXM-035 | Local feature build, synthetic HID screenshot and all three default HID GUI tests passed | Pending local target access |
| Raw Ethernet native capture | Historical Windows Npcap virtual adapter evidence | Historical Linux veth/libpcap evidence | Pending BPF/libpcap qualification |
| Native CAN | Unsupported vendor backend | Current native SocketCAN RX/TX/filter test passed on guest-only vcan | Unsupported vendor backend |
| Passive USB capture | Unsupported USBPcap backend | Requires enabled usbmon and selected bus access | Unsupported backend |
| Install/update/uninstall and packaged permissions | Pending EXM-037 | Pending EXM-037 | Pending EXM-037 |
| Physical timing/electrical/report fidelity | Pending declared EXM-036 devices | Pending declared EXM-036 devices | Pending declared EXM-036 devices |

The historical native adapter results have their exact earlier source, library,
OS and fixture identities in the immutable task evidence for EXM-008, EXM-013
through EXM-017. They provide adapter evidence for those revisions. They do not
establish current native desktop or packaged compatibility.

Current Linux core verification uses Rust 1.98.0 x86_64-unknown-linux-musl
executables in Alpine 3.23.8, kernel 6.18.60-0-lts, under QEMU TCG. All 142
domain, protocol and default capture tests passed across 21 binaries. Each
guest binary hash was matched to the cross-built artifact. The VM used two
vCPUs, 1 GiB RAM, a read-only ISO and a read-only directory of test binaries,
with no network adapter or physical passthrough. Loopback was enabled inside
the guest. Storage's 47 Linux tests passed on the earlier unchanged source,
with one opt-in helper test ignored. All 122
current application tests passed after EXM-101: 63 in its correction evidence
and the remaining 59 in ten binaries under the resumed EXM-035 run. The latter
run matched every binary hash, emitted `EXMON_ALL_PASS` and powered off cleanly.
This establishes Linux core/application execution. Desktop, packaged permission
and hardware qualification remain pending in the table.

A synthetic profile exported through the Windows settings controller was
imported and exported through the same public controller in an actual Ubuntu
24.04.5 LTS x64 guest. The exported files were byte-identical, SHA-256
`8f8b9c51a2a0ae2ca05c9d350ef24ba6fb126beba2e00c980ac1537edc6950db`.
The profile covered numeric field endianness/scaling/units, a moving-average
chart, a TX template and a disabled schedule. The harness started no capture or
transmission worker. Apple Silicon handoff remains pending.

The Ubuntu guest also executed 49 ordinary capture tests with native CAN, HID,
USB and usbmon features enabled. Five opt-in fixture tests were ignored in that
run. A separate native SocketCAN RX/TX/filter test then passed on a declared
guest-only `vcan` link after loading the version-matched Ubuntu `can-dev` and
`vcan` modules. The link and added modules were removed afterward. These results
provide current native-feature and virtual-CAN evidence; the other ignored
fixtures and physical-device fidelity remain separate qualifications.

The current debug Linux GNU GUI builds and opens under an ordinary user in the
Ubuntu guest. Xvfb 21.1.12 provides X11; Vulkan exposes only the CPU adapter
`llvmpipe (LLVM 20.1.2, 128 bits)` from Mesa 25.2.8. Minimal-server startup also
needed `libxcursor1` and `libxi6`, alongside the X11/XKB and Vulkan libraries.
The manual synthetic workflow rendered incoming observations, reached IDLE
after Stop and exited normally on `WM_DELETE_WINDOW`. Chart controls and the
settings form rendered; plotted-series, settings editing and theme switching
were not established by that manual check.

All eight existing native screenshot examples failed their unchanged
eight-second completion checks in this debug TCG/software-renderer environment.
A visible diagnostic also reached its unchanged deadline while capture made
progress. An optimized build passed the native shell, HID, incoming and packet
inspection screenshot scenarios. Outgoing, charts, sessions and settings did
not finish their screenshot scenarios within the unchanged deadline. Across
the attempted GUI regression cases, 15 passed, 11 failed and two visual cases
were incomplete at the external process watchdog. The completed failures
include GPU readback deadlines and existing FPS assertions. These results leave
the full GUI gate pending. EXM-102 corrected the Linux benchmark cleanup lint
failure at `1965fc8`, preserving Windows retry limits. A subsequent local
Rust 1.98.0 GNU-target `clippy --workspace --all-targets --locked -- -D warnings`
passed at `82f065c`. This is a cross-compile lint result, not a GUI runtime pass.

The software-renderer comparison covered Vulkan/OpenGL and QEMU's default CPU
plus `-cpu max` (llvmpipe 128/256 bits). No test assertion, eight-second native
deadline or ten-second GPU wait limit was relaxed. A native runner is the next
qualification target. Windows Hypervisor Platform was installed under the
existing owner installation authority with `-NoRestart`; Windows reports a
required restart, and the immediate WHPX re-probe still fails to initialize.
The host has not been restarted. Local acceleration availability must be
rechecked after a coordinated restart or on a separate local target.

## Local verification boundary

[FDTasks 5.7](../Tasks/tasks_file_structure_v5_7.md) prohibits hosted product
tests, including manually dispatched workflows. Task and Epic acceptance tests
must run in the owning local development/verification turn. The agent-created
manual hosted qualification route at `aa88a9d` was a policy error. Its workflow
has been disabled and removed; its run observations are retained as diagnostics
and are excluded from task/Epic acceptance. The earlier accepted records remain
unchanged and a new amendment records this correction.

The retained [local supervisor](../.github/scripts/platform_qualification.py)
rejects hosted-CI environments. It can be used only from an owning local
Linux/macOS turn with explicit `EXMON_EXPECTED_MACHINE` and `RUNNER_TEMP` values.
It records exact source, lockfile, policy, OS/architecture, graphics and individual
command outcomes. The installed startup probe uses supervised termination as
fixture cleanup; normal window-close behavior is checked separately. Packaged
installers remain EXM-037. No current hosted workflow invokes this supervisor.

The committed Windows-exported JSON fixture is preserved without line-ending
conversion and checked against its original SHA-256 before import/re-export.
`cargo run -p exmon-app --example portable_profile_smoke --locked -- import
INPUT OUTPUT` exercises the public settings controller without starting I/O.
Local Windows import/export and existing-file refusal passed. At `82f065c`, the
committed verifier also ran in the local Ubuntu 24.04.5 x64 guest: import,
validate, apply and export preserved the original fixture bytes; existing-file
refusal returned exit 1 and left those bytes unchanged. Temporary profile
fixtures were removed afterward. The stripped GNU verifier SHA-256 was
`4f1951d24ccfff095970329416e747c32a71a44c46e68d5e614b56711da3d435`.
The full local Linux GUI and Apple Silicon qualification remain required. Hosted
diagnostics establish no task/Epic acceptance or platform sign-off.

The current Windows host has no detected Npcap service or installed Npcap DLLs
at the trusted System32/Npcap location. The default synthetic and socket routes
remain usable. Native Ethernet requires a trusted Npcap installation and its
configured access rights. Linux capture requires libpcap and appropriate
capture permission. macOS requires permitted BPF access. Consult
[the access policy](security-and-transmission.md) before native qualification.

## Remaining target procedure

Provide the accepted source revision and the committed Cargo.lock on each
target. Record OS version, architecture, graphics backend and driver/library
versions. Build the default workspace and enabled native backends, then run
the applicable integration and GUI tests locally.

```text
cargo +1.98.0 build --workspace --locked
cargo +1.98.0 test --workspace --locked -- --test-threads=1
cargo +1.98.0 fmt --all -- --check
cargo +1.98.0 clippy --workspace --all-targets --locked -- -D warnings
node scripts/check-architecture.mjs
cargo +1.98.0 run -p exmon-gui --example native_sessions_smoke --locked
```

Run the [prototype demonstration](virtual-prototype.md) in a native window.
Record scaling, themes, charts, import/export paths, error feedback, permissions
and clean shutdown. Configuration currently uses explicit text paths for
import/export. The accepted file-dialog scenario must be qualified against the
actual delivered UI.

Export a synthetic portable project JSON on Windows and load those exact bytes
on Linux and Apple Silicon. Compare validated fields, endianness, units, framing
and disabled schedule values. Export it again, compare semantic JSON and
record each file hash. Machine binding selectors remain target-local.

Native tests require a declared virtual interface or explicitly authorized
physical stand. Verify denied access, reconnect, malformed input and Stop on
the actual selected backend. Store only synthetic data and redacted device
models. Record unsupported capabilities and unavailable checks explicitly.

Final sign-off requires a reviewer to bind the completed matrix to the tested
source and every applicable target result. EXM-036 retains the physical-device
matrix, EXM-037 owns installable artifacts, and EXM-038 owns release acceptance.
