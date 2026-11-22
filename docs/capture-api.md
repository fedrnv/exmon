# Capture adapter API

`exmon_capture::adapter::Adapter` is a Send trait for trusted background adapters.
`Session<A>` owns its adapter and bounded RX queue. Construct/configure it on the
worker, never on the GUI thread. The caller owns scheduling and transfers small
drained batches to other workers. No thread, driver, timer or physical interface
opens merely by constructing a Session.

Discovery returns at most 64 descriptors with opaque id, transport kind and
separate RX/TX availability (available, unavailable with bounded reason, or
unsupported). Discovery is not authorization to open. Open is an explicit action
subject to the product safety policy; adapters must release partial resources
before returning an open error. Configure supplies validated snapshot length;
backend-specific configuration remains in the platform adapter. Start requires
open/stopped, successful configuration and RX availability.

Each successful start creates a checked epoch, resets sequence and creates new
metrics and cancellation state. Reconnect closes and opens explicitly; it never
configures, starts or transmits automatically. Old cancellation tokens stay
cancelled and cannot affect the new run. Errors stop the backend and invalidate
configuration; disconnect/failed states cannot restart through Stop. Failed
close retains Failed and cannot be treated as a safely closed interface. Drop
attempts stop/close without panicking; callers must use explicit close to observe
cleanup errors. Backend stop/close must be idempotent and finite.

`pump()` performs one finite cooperative poll. Platform adapters must implement a
bounded timeout or cancellation-aware read; they check the shared Cancellation
token during reads. Cancelling requests interruption; worker Stop completion is
the acknowledgement after backend cleanup. A backend that ignores this contract
cannot claim clean cancellation merely because the trait compiles. This API has
no scheduler and invokes no TX hook. `Adapter::transmit` defaults unsupported and
is reserved for a separately verified explicit TX worker with final authorization
and Stop/submission serialization. Do not invoke it directly from a GUI/timer.

Observations have relative monotonic nanoseconds, direction, immutable raw bytes,
optional original length and bounded typed transport metadata. Ethernet has link
type; TCP/UDP have endpoints (and TCP connection identity); serial has config
revision; USB has endpoint/completion; CAN has identifier/extended/FD flags.
Backend-specific link parsing, CAN DLC/error/remote details and richer transfer
status may extend this metadata when their adapters implement them. The current
API does not claim those platform adapters. Observation construction checks a
65536-byte maximum, original lengths, USB address bits and CAN id/data bounds
before copying. Debug omits payload and device/network context. Original length
greater than captured length remains visible truncation, never padding.

Session envelopes add interface id, epoch and checked per-run sequence. Sequence
advances even when queue insertion drops a record, allowing consumers to detect
gaps and invalidate partial framing. Clock/source epochs must never be compared
as wall time. The session does not synthesize a timestamp from the current clock.

Limits require queue count 1..4096, queue payload bytes 1..16 MiB, batch count
1..64 and batch payload bytes 1..65536. Queue metadata overhead is additionally
bounded by item count. Adapter poll must enforce these budgets before allocation;
Session rejects a violating returned batch and enters Failed. No Rust wrapper
can retroactively prevent an allocation already made by an untrusted backend.
Do not use this trait as a sandbox for third-party code.

Insertion drops new observations when either queue budget is exhausted and
increments ApplicationDrops. Clearing undrained entries at close/error/restart
also accounts for discarded entries. RX counters describe all valid polled
observations, including queue drops. `take_backend_loss()` returns no report,
known count or unknown-count report separately; the coalesced report is consumed
even when the queue is full. Display decimation remains a separate consumer
metric. No log I/O occurs in these paths.

`drain(maximum)` transfers at most batch_items and releases the payload-byte
budget. Queue backing allocation remains bounded by queue count; drained output
is bounded by batch count and the queue byte budget. Stop preserves buffered RX
for optional inspection; a subsequent start clears it with loss accounting and
uses a new epoch. State, last_error and health snapshots are available outside
the data queue, so full queues cannot hide disconnect or application loss.

`cargo test -p exmon-capture --locked` uses a deterministic in-memory mock with
no driver, wall-clock sleeps or physical TX. Tests cover lifecycle, cancellation,
explicit reconnect, stale token isolation, invalid transitions/configuration,
permission denial, disconnected/failed state, misbehaving batch sizes, snapshot
limits, failed cleanup, count/byte pressure and exact measured drop totals.
`cargo test -p exmon-domain --locked` checks observation validation/redaction and
shared metrics. Actual platform stop-under-load qualification belongs to each
platform adapter task.
