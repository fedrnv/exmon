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
environment. Windows Npcap runtime is absent on this host, and a macOS verification
host is unavailable. Linux virtual native acceptance is recorded below.

Offline fixture tests use a synthetic complete Ethernet header with local
addresses and an experimental EtherType. They verify exact bytes, truncation,
capture-time preservation, option propagation, cancellation under 400 synthetic
records, rejection of non-Ethernet links and explicit permission-denied results.
They test the production generic adapter, not just a reference decoder, but
cannot prove the native driver behavior. EXM-008 remains unclosed until the
required platform evidence is available.

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
application and no privilege grant is attempted. Windows/macOS smoke remains
unverified; this Linux evidence does not close EXM-008 or qualify a release.
