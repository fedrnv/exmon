# Platform qualification

EXM-035 remains in progress. The reference source is the published virtual
prototype at 95068a1, followed by task/version records. The Windows development
qualification covers the default synthetic desktop and loopback boundaries.
The full supported-platform acceptance requires runtime evidence on the target
systems below.

| Scenario | Windows x64 VPS | Linux x64 | macOS Apple Silicon |
| --- | --- | --- | --- |
| Default synthetic shell, capture, editing, charts, Stop and shutdown | Verified by EXM-039 native smoke and production GUI tests | Pending current desktop runtime | Pending target access |
| Configuration schema, examples, roundtrip, disabled schedules | Verified shared JSON tests | Same source contract, target runtime pending | Same source contract, target runtime pending |
| TCP/UDP socket traffic | Verified local IPv4/IPv6 loopback | Historical native virtual adapter evidence | Pending target runtime |
| Serial and CAN virtual fixtures | Verified deterministic adapters | Historical isolated serial/SocketCAN fixture evidence | Pending target runtime |
| USB/HID mock selection and explicit Send | Verified production worker and GUI | Historical emulated HID/USB evidence | Pending target runtime |
| Native HID feature build and mock UI | Current Windows check recorded under EXM-035 | Pending current feature/runtime checks | Pending target access |
| Raw Ethernet native capture | Historical Windows Npcap virtual adapter evidence | Historical Linux veth/libpcap evidence | Pending BPF/libpcap qualification |
| Native CAN | Unsupported vendor backend | Requires enabled SocketCAN and provisioned link | Unsupported vendor backend |
| Passive USB capture | Unsupported USBPcap backend | Requires enabled usbmon and selected bus access | Unsupported backend |
| Install/update/uninstall and packaged permissions | Pending EXM-037 | Pending EXM-037 | Pending EXM-037 |
| Physical timing/electrical/report fidelity | Pending declared EXM-036 devices | Pending declared EXM-036 devices | Pending declared EXM-036 devices |

The historical native adapter results have their exact earlier source, library,
OS and fixture identities in the immutable task evidence for EXM-008, EXM-013
through EXM-017. They provide adapter evidence for those revisions. They do not
establish current native desktop or packaged compatibility.

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
