# Platform qualification

EXM-035 remains in progress. The reference source is the published virtual
prototype at 95068a1 with the outgoing readiness correction at 9b53499,
followed by task/version records. The Windows development
qualification covers the default synthetic desktop and loopback boundaries.
The full supported-platform acceptance requires runtime evidence on the target
systems below.

| Scenario | Windows x64 VPS | Linux x64 | macOS Apple Silicon |
| --- | --- | --- | --- |
| Default synthetic shell, capture, editing, charts, Stop and shutdown | Verified by EXM-039 native smoke and production GUI tests | Pending current desktop runtime | Pending target access |
| Configuration schema, examples, roundtrip, disabled schedules | Verified shared JSON tests and export for Linux handoff | Current configuration/settings tests and exact Windows file import/export passed | Same source contract, target runtime pending |
| TCP/UDP socket traffic | Verified local IPv4/IPv6 loopback | Current Linux QEMU local socket tests passed | Pending target runtime |
| Serial and CAN virtual fixtures | Verified deterministic adapters | Current Linux virtual fixture tests and native vcan RX/TX passed | Pending target runtime |
| USB/HID mock selection and explicit Send | Verified production worker and GUI | Historical emulated HID/USB evidence | Pending target runtime |
| Native HID feature build and mock UI | Current Windows check recorded under EXM-035 | Current feature build and ordinary adapter tests passed; mock GUI pending | Pending target access |
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
the guest. Storage's 48 tests passed on the earlier unchanged source. All 122
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
