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
environment. These live checks have not been run in this Windows environment:
Npcap runtime is absent, and macOS/Linux hosts are unavailable.

Offline fixture tests use a synthetic complete Ethernet header with local
addresses and an experimental EtherType. They verify exact bytes, truncation,
capture-time preservation, option propagation, cancellation under 400 synthetic
records, rejection of non-Ethernet links and explicit permission-denied results.
They test the production generic adapter, not just a reference decoder, but
cannot prove the native driver behavior. EXM-008 remains unclosed until the
required platform evidence is available.
