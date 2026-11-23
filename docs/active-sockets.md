# Active TCP and UDP sockets

`exmon_capture::sockets::Sockets` implements the worker-owned `Adapter` contract
using ordinary OS sockets, without raw-capture privileges. It supports TCP client,
TCP server, bound UDP and connected UDP modes with numeric IPv4 or IPv6 local and
remote endpoints. IPv6 sockets are IPv6-only; use a separate IPv4 adapter when
both families are needed. Local port zero requests an ephemeral port; inspect
`snapshot().local` after Start. Remote ports must be nonzero and addresses must
be specified and have the same family as the local endpoint.

Construction, enumeration, Open and Configure do no socket I/O. Explicit Start
binds/listens/connects. TCP connect has a validated 1–1000 ms timeout (default
250 ms), checks cancellation before and after that bounded call, and drops the
new socket on failure. Run it on the capture worker, never the GUI thread.
Cancellation cannot undo a submitted kernel handshake. Accept, receive and send
are nonblocking; this adapter creates no threads, async tasks, timer or runtime.
GUI endpoint controls and worker command routing are separate EXM-029 work.

Poll returns received application bytes, not Ethernet/IP/TCP wire packets. TCP
observations carry local/remote endpoints and a connection ID; UDP observations
carry the actual sender and preserve datagram boundaries, including an empty
datagram. Timestamps use the adapter's monotonic clock since the current Start,
with no invented device timestamp. TCP chunks are arbitrary stream fragments.
UDP receives into a full 65536-byte scratch buffer before applying the configured
snapshot; its original length is exact even when the retained bytes are shorter.
Snapshot truncation has a separate counter/event. Kernel UDP loss is unknown,
reported through the common backend-loss interface; it is not reported as zero.
The Windows oversized-datagram error is explicit and counts one lost datagram.
An asynchronous UDP refusal/reset is an observable nonfatal error; unrelated
datagrams can still be received. Fatal adapter errors release owned sockets.

`snapshot()`, `peers()` and `drain_events()` expose phases, endpoints, half-close,
connection errors and counters. The bounded event ring discards its oldest event
when full and increments `events_dropped`; snapshot/peers remain authoritative
after an event gap. Debug formatting redacts endpoints and carries no payload.
Counters are cumulative over the adapter lifetime, and events identify the Start
epoch. A TCP peer EOF closes only its read direction so a response can still be
sent. Explicit `shutdown_write` closes the other direction; once both are closed,
the peer is removed. Explicit `close_peer` removes it immediately. A server keeps
listening after an individual peer EOF/reset, subject to its active-peer cap;
half-closed peers occupy a slot until closed. No implicit reconnection occurs.

Payload transmission requires a separate explicit action:

- `send_tcp(current_epoch, connection_id, bytes, cancel)` submits one write and
  returns the actual accepted prefix length. On `Busy`, no bytes were accepted.
  Only the caller may retry the remaining suffix while its user action or enabled
  schedule is still valid. Successful submission does not prove peer delivery.
- `send_udp(current_epoch, destination, bytes, cancel)` sends one entire datagram,
  including an explicitly requested empty datagram. Bound mode needs a valid
  destination; connected mode uses its selected peer and rejects an override.

Generic targetless `Adapter::transmit` is Unsupported. Startup, Poll, reconnect
and cleanup send no application payload. There is no internal TX queue, periodic
schedule or broadcast-to-all-peers operation. Stop revokes the running phase
before dropping every owned socket. A new Start uses a fresh epoch and TCP
connection IDs, so stale submissions fail. A cancelled Poll returns Cancelled;
its worker must call Stop/Close, as the common Session failure path does. Drop
also releases resources. A TCP connection can leave OS TIME_WAIT state after its
handle has been dropped; that is distinct from a leaked descriptor and does not
guarantee immediate rebinding of the same connected endpoint tuple.

Hard bounds per adapter: 32 peers, 256 event slots, 65536 snapshot/batch/send bytes,
64 observations and at most 64 OS accept/read attempts per Poll (at most eight
accepts). Fair TCP polling rotates across peers. UDP empty datagrams still consume
an item/attempt. Settings default to 16 peers and 64 event slots. Requested socket
buffers are 1024–1048576 bytes (default 65536); actual allocation/rounding is under
OS control. IPv4 UDP send payloads are limited to 65507 bytes, IPv6 to 65527, and
OS/path limits may be lower. Peer IDs/epochs use checked counters, metrics use
saturating counters. Returned batches and caller-retained history need their own
bounded queues; these limits are not a process RSS guarantee.

EXM-012 verification uses actual Windows IPv4/IPv6 loopback sockets, ephemeral
ports, exact binary payload comparisons, partial-write/backpressure handling,
half-close/reset isolation, repeated shutdown/reopen and port reuse, event/peer
limits and common Session integration. It requires no physical interface, driver,
firewall exception or external server. This is Windows VPS qualification; later
EXM-035/036 platform and physical-stand validation and Epic gate/review remain
separate acceptance obligations.
