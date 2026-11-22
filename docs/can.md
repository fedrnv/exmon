# CAN capture and explicit transmission

`can::Can<S>` is a background-worker adapter. `Frame` validates classic CAN
identifiers, standard/extended flags, remote requests, error records and exact
data/DLC consistency before allocating at most eight bytes. Wire DLC 9..15 is
preserved for eight-byte classic frames. Remote requests carry a requested DLC
and no data; their observation original payload length is zero. Error records
are receive-only. Byte-only `Adapter::transmit` is Unsupported: CAN transmission
requires the typed `send_frame` operation and a current cancellation token.

Discovery, logical open and configure send nothing. Start activates only the
selected channel. `send_frame` requires a running adapter and available TX;
success means accepted by the source/driver, never confirmed physical delivery.
Disconnect, permission failure and backend fault stop the source and require
explicit restart. Busy does not cause a retry. Stop/cancellation prevent later
submissions; already accepted frames cannot be recalled. The application worker
must serialize current user/epoch/schedule authority with submission/revocation.
This transport does not create authorization, schedules or automatic retries.
The common RX `Session` owns its adapter; a future TX worker uses the typed API,
not an implicit conversion from captured bytes or a byte-only send hook.

`Metadata::Can` exposes channel, identifier, extended/FD/remote/error flags and
DLC, with exact payload and direction in the immutable observation. Monotonic
ingest time and optional source time remain separate. Snapshot truncation keeps
the original captured payload size. Metadata and observation Debug redact raw
identifiers/data. `Frame` deliberately has no Debug implementation.

Filters are a bounded OR-list of `(id, mask)` raw CAN identifier words, including
EFF/RTR matching when those bits are in the mask. Empty list receives no ordinary
frames. Error delivery is a separate explicit `receive_errors` option, disabled
by default. Inversion/ERR in ordinary filter ID or mask is rejected. At most 64
filters, channels per discovery and observations per poll are allowed. Polling
also has an item work bound when a source returns rejected or zero-byte records.

`Mode::Fd` is an explicit extension boundary returning Unsupported today. Future
FD support needs a distinct validated payload/DLC mapping, BRS/ESI metadata,
source capability negotiation and FD-native tests. No classic frame is silently
promoted to FD, and no 64-byte input is truncated to pretend it is classic CAN.

## Sources

`virtual_can::VirtualBus` provides isolated in-process broadcast endpoints.
At most 64 active endpoints each retain 1..4096 frames; queue-full drops are
counted separately for each receiver. Filters run before queueing; senders do
not receive their own frames. Synthetic source time increases once per explicit
send, without a wall clock or physical interface. Endpoint close releases its
slot and pending records; reopening starts empty. Bus/endpoint counts and payload
bounds cap memory. Session queue loss remains distinct from source queue loss.

The [SocketCAN kernel documentation](https://docs.kernel.org/networking/can.html)
defines Linux CAN sockets, identifier flags, receive filters and local loopback.
`native_can::NativeCan`, enabled by Linux-only `native-can`, uses the safe
[socketcan 4.0.0 API](https://docs.rs/socketcan/4.0.0/socketcan/).
It discovers ARPHRD_CAN interfaces, binds a selected nonzero interface index,
uses nonblocking I/O, explicit filters and socket receive timestamps, enables
local loopback and disables own-message reception. Other local applications can
still see sent frames. The safe receive API used here does not expose the flags
needed to distinguish local loopback from physical receive, so native direction
is honestly Unknown. Source time is socket arrival time since Unix epoch;
hardware timestamps and backend drop totals are not measured. Loss is reported
unknown once per activation.

Bitrate, CAN mode/listen-only settings, link creation/up, driver/module loading
and access permissions must be prepared externally. No link/configuration or
privilege mutation is performed. Missing link, access denial and absent backend
return explicit errors; `requirements()` provides the setup boundary. Opening
a physical CAN controller can acknowledge traffic: no listen-only, electrical
or timing qualification is claimed. Windows/macOS vendor backends are isolated
future sources; today native discovery/open/send return Unsupported there.
Use the virtual bus for the unattended prototype.

## Verification and resume

`cargo test -p exmon-capture --test can --locked` checks virtual RX/TX, filtered
standard/extended/RTR frames, exact bytes/DLC/timestamps, bounded loss, Session
snapshots, reconnect, cancellation, invalid inputs and explicit unavailable paths.
Linux native code and the opt-in vcan test compile with:

```text
cargo clippy -p exmon-capture --features native-can --target x86_64-unknown-linux-gnu --all-targets --locked -- -D warnings
```

On an externally prepared Linux host with iproute2 and an approved **vcan** link,
set `EXMON_TEST_VCAN` to its name, then run:

```text
cargo test -p exmon-capture --features native-can --test can --locked native_vcan_rx_tx_and_filtering -- --ignored --exact
```

The ignored test verifies iproute2 JSON `info_kind == vcan` before opening sockets
or sending. It refuses physical interfaces, performs no setup and has a one-second
receive deadline. Never remove that check to run this test on a real bus.

Current Windows host has no installed WSL/Linux runtime. Native vcan integration
is UNAVAILABLE, not a passing mock or cross-compile. EXM-017 stays In Progress
until its actual SocketCAN vcan acceptance has run successfully. Physical bus,
controller behavior and timing remain separate stand validation.
