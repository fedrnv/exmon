# Ethernet capture adapter

`Ethernet<S>` implements the common Adapter API using a bounded, injectable
packet source. `NativePcap` is enabled explicitly with `native-pcap` and uses
the safe upstream [pcap API](https://docs.rs/pcap/2.5.0/pcap/struct.Capture.html),
backed by Npcap on Windows or libpcap on Linux/macOS. No local unsafe code,
driver installer, automatic privilege grant or TX implementation is introduced.

Discovery lists at most 64 local interface names. Names remain local UI data,
not operational log fields. Ids select the current discovery list; they are not
portable stable device identities. Re-discovery while selected is rejected.
Construction, discovery and logical open do not activate live capture. Start
activates only the explicitly selected interface after configuration.

Options default to non-promiscuous capture, 10 ms requested buffer timeout and
no BPF filter. Requested timeout must be 1..1000 ms; filter text is limited to
4096 bytes without NUL. Configure supplies snapshot length, 1..65536 bytes.
The native source applies snaplen, promiscuity and timeout before activation,
then compiles/applies BPF before returning data. Activation/filter failure drops
the handle. Nonblocking mode is required: packet-buffer timeout alone does not
guarantee a cancellable read. Poll checks cancellation between packets and stops
at both batch limits; Stop drops the native handle. The worker must choose a
bounded cadence to avoid spinning while no packets arrive.

Only DLT_EN10MB (link type 1) is accepted by this raw Ethernet adapter. Cooked
Linux capture and non-Ethernet loopback links return Unsupported without guessing
headers. A separate suitable adapter can support them later. Backend permission
denial is mapped to a fixed PermissionDenied code when identifiable; other native
failures stay bounded Backend errors without leaking raw error text. Discovery
does not prove access; actual activation may still fail. Unsupported platforms
or missing native dependencies remain unavailable, never trigger installation.

Captured bytes and original wire length are preserved, including explicit
snapshot truncation. Direction remains Unknown because this API does not infer
RX/TX from addresses. Native microsecond capture timestamps are checked and
stored separately as `source_timestamp_ns`; monotonic ingest time relative to
start remains `timestamp_ns`. Source wall time never drives scheduling.
Unsigned timestamp conversion/overflow and impossible original lengths fail.
pcap drop statistics are coalesced separately; a counter decrease/statistics
failure reports unknown loss rather than fabricated zero. Platform statistics
semantics still require actual backend qualification.

## Local verification and manual setup

Default offline checks need no driver:

```text
cargo test -p exmon-capture --locked
cargo test -p exmon-domain --locked
cargo check -p exmon-capture --features native-pcap --all-targets --locked
```

Windows native linking needs the Npcap SDK matching the target architecture.
Set LIBPCAP_LIBDIR to its Lib/x64 directory for an x64 build. The
[official Npcap developer guide](https://npcap.com/guide/npcap-devguide.html)
describes the SDK. SDK download/extraction alone does not install the driver.
Runtime requires an owner-approved trusted Npcap installation and an appropriate
DLL search configuration; do not copy arbitrary DLLs into capture directories.
Linux/macOS require their libpcap development/runtime environment. Packaging must
verify trusted library resolution before distribution.

The manual smoke example requires an explicit argument. Listing does not open
capture; physical capture requires the separately explicit selected id:

```text
cargo run -p exmon-capture --features native-pcap --example ethernet_smoke -- --list
cargo run -p exmon-capture --features native-pcap --example ethernet_smoke -- --capture <id>
```

Run capture only on an authorized test interface. The example reads for one
second, requests cancellation and closes, reporting count only. It does not send,
save or print packets. A nonzero count alone does not verify packet fidelity;
platform acceptance must compare known synthetic traffic and metadata, test BPF
and snapshot behavior, permission denial and stop under load in each supported
environment. Windows Npcap and Linux virtual native acceptance are recorded below;
macOS runtime qualification remains deferred to the separate platform stand.

Offline fixture tests use a synthetic complete Ethernet header with local
addresses and an experimental EtherType. They verify exact bytes, truncation,
capture-time preservation, option propagation, cancellation under 400 synthetic
records, rejection of non-Ethernet links and explicit permission-denied results.
They test the production generic adapter, not just a reference decoder, but
cannot prove the native driver behavior. The owner's 2026-11-23 Windows VPS
phase amendment permits development closure after Windows/Linux verification;
macOS qualification remains an explicit EXM-035 obligation rather than a PASS.

## Windows VPS native acceptance

The owner authorized required Windows development installations and postponed
real-device/macOS validation to separate stands. Npcap 1.90 was installed on
Windows Server 2025 x64 from the official signed installer using its standard
Free Edition wizard. The driver runs with administrator-only access; optional
wireless monitor support and WinPcap compatibility replacement are disabled.
No OEM-only silent mode or driver-signing bypass is used. Installed wpcap.dll
reports libpcap 1.10.7. This is a local development prerequisite, not permission
to redistribute Npcap with this application.

A dedicated Microsoft KM-TEST Loopback Adapter provides real Ethernet framing
without a physical device. Microsoft-signed WDK DevGen/DevCon tools provisioned
the exact root test instance, using Windows' inbox netloop.inf/loop.sys driver.
The management adapter is not changed. For reproduction with trusted WDK tools:

```powershell
devgen.exe /add /bus ROOT /instanceid EXMON_ETHERNET_TEST /hardwareid '*MSLOOP'
devcon.exe update C:\Windows\INF\netloop.inf '*MSLOOP'
$env:LIBPCAP_LIBDIR = '<Npcap SDK>\Lib\x64'
$env:PATH = 'C:\Windows\System32\Npcap;' + $env:PATH
$env:EXMON_TEST_WINDOWS_ETHERNET = 'km-test'
cargo test -p exmon-capture --features native-pcap --test ethernet --locked native_windows:: -- --ignored --nocapture
```

Provision the named fixture once; reuse it instead of creating duplicate adapters.
The test derives the Npcap name only from the exact root instance's GUID and
requires the Microsoft KM-TEST product and kmloop service. It never takes a
caller-supplied interface name. Use trusted installed DLLs; do not place downloaded
or copied DLLs beside the executable. Scope PATH changes to the invoking session.

Independent test-only injection sends bounded alternating EtherTypes over this
virtual adapter. The production RX-only Adapter captures only filtered 0x88b5
frames, preserving exact 64/16-byte snapshots, original length 64, Ethernet link
type, Unknown direction and source timestamp. At least 32 matching frames per
round and native cancellation/close below 500 ms while traffic continues are
required. The generator is bounded to 4000 sends with one-millisecond sleeps,
stops atomically and is joined on cleanup. Run the opt-in process under a
30-second external watchdog when automating fixture execution.

Npcap's built-in NPF_Loopback is separately checked to return Unsupported from
the Ethernet adapter, because its link type is not Ethernet. The implementation
does not invent Ethernet headers to make loopback pass. Native Windows tests,
feature-enabled error unit tests, ordinary regressions and lint passed. No
physical interface capture/transmission or macOS runtime PASS is claimed.

## Linux virtual native acceptance

The production native Adapter passed on Alpine standard 3.23.8/Linux
6.18.60-0-lts in portable QEMU 11.1.0 with software-only TCG, two vCPUs and
512 MiB RAM. Statically linked libpcap 1.10.7 came from Alpine's official
`libpcap-dev-1.10.7-r0.apk`; the guest's trusted `apk verify` accepted that
package before any test. No host driver or physical interface was used.

The ignored native tests require `EXMON_TEST_ETHERNET=qemu-veth`, exact
`EXMON-QEMU`/`EXMON-NET-FIXTURE` guest DMI identities, and iproute2 JSON proving
that both fixed fixture interfaces are `veth` before capture or generation.
Prepare the isolated guest externally, with no TAP, bridge or physical passthrough:

```sh
apk add iproute2 python3
ip link add exmontx type veth peer name exmonrx
ip link set exmontx up
ip link set exmonrx up
```

Use the [manual Alpine standard boot recipe](can.md), adding QEMU
`-smbios type=1,manufacturer=EXMON-QEMU,product=EXMON-NET-FIXTURE`.
User-mode guest networking is needed only for public packages; application
capture selects `exmonrx`, never the guest's Internet-facing adapter.

Build the Linux musl test with Rust LLD as in the CAN recipe, feature
`native-pcap`, test `ethernet` and `--lib` for its error-mapping regression.
Set `LIBPCAP_LIBDIR` to a directory containing the verified Linux `libpcap.a`
and `LIBPCAP_VER=1.10.7`; a Windows SDK library cannot substitute for it.
Share only the binaries and APK on the narrow read-only artifact drive.
After `apk verify`, copy the executable to `/tmp/ethernet-tests`, mode 755:

```sh
su nobody -s /bin/sh -c 'EXMON_TEST_ETHERNET=qemu-veth /tmp/ethernet-tests --ignored --exact native_virtual::native_veth_permission_denied_is_explicit --nocapture'
EXMON_TEST_ETHERNET=qemu-veth /tmp/ethernet-tests --ignored --exact native_virtual::native_veth_fidelity_filter_snapshot_and_stop_under_load --nocapture
/tmp/ethernet-tests --nocapture
```

The finite Python generator sends alternating experimental EtherTypes 0x88b5
and 0x88b6 over `exmontx`. The production BPF filter accepts only 0x88b5; both
64-byte and 16-byte snapshots match the exact expected prefix and retain original
length 64, link type 1, Unknown direction and source timestamp. At least 32
matching records are required per round. Cancellation and native close must finish
within 500 ms while the generator remains live. Generator lifetime is four seconds,
socket timeout one second, receive deadline two seconds and batch limit eight.
The fixture owns and reaps its generator even if an assertion fails.

An actual unprivileged attempt exposed libpcap's message
`Attempt to create packet socket failed - CAP_NET_RAW may be required`, which
previously mapped to Backend. It now maps to PermissionDenied; regression tests
preserve Backend for unrelated failures. No raw backend error is logged by the
application and no privilege grant is attempted. Windows results are recorded
above. macOS smoke remains deferred to EXM-035; these development results do not
qualify macOS or a release.
