# Diagnostics and metrics

`exmon_domain::metrics::Metrics` is a fixed-size collector owned by one interface
and session epoch. Capture, protocol and storage workers can share it through an
`Arc` without depending on the application or GUI. Create a new collector on
reconnect/new epoch; do not reset live counters underneath readers.

`add`, `backend_loss`, `set_queue_depth` and `set_connected` perform only atomic
operations. They allocate nothing, hold no mutex, invoke no callbacks and never
log or perform I/O. Saturating compare-and-swap updates are lock-free on the
supported desktop hosts, not a wait-free or hard real-time latency guarantee.
Totals never wrap; `saturated()` exposes loss of precision at `u64::MAX`.

The independent totals are RX/TX packets and bytes, known backend loss, backend
loss reports with unknown count, application drops, display decimation, decode
failures, transmission errors, recording loss and validation rejections. An
unknown backend count increments only the unknown-report count, never pretends
that zero packets were lost. Loss stages may overlap; do not sum them into a
physical packet-loss total. TX counters measure accepted/submitted traffic; they
do not prove wire delivery. Update byte/packet counters at one documented stage
per adapter to prevent double accounting.

Queue depth is an atomically sampled gauge, not a queue or synchronization
primitive. Its designated queue owner reports the actual current value. A
collector represents one queue; use distinct collectors for distinct pipeline
queues rather than overwriting unrelated depths. It does not reserve capacity.

`snapshot()` returns an owned, fixed-size `HealthSnapshot`. The GUI reads it
without accessing device handles or waiting for the logging sink. Each field is
atomic but the combined snapshot is approximate during concurrent updates. Health
starts Disconnected. Connected with no failures is Healthy; backend/application
loss, decoding/TX/recording/validation errors or saturation makes it Degraded.
Display decimation alone does not imply capture failure. Degradation remains
visible for the epoch; disconnection takes precedence over it.

`throughput_since` computes RX/TX bytes and packets per second using monotonic
sample times. Snapshots retain an opaque source token; cross-epoch comparisons,
equal/reversed times, decreased counters or saturation yield `None`. Do not
substitute zero for unavailable rates. Samples must be taken periodically by
the consumer; no timer or sampling thread is created by the metrics library.

`exmon_app::diagnostics::log_health` emits named tracing fields for all counters,
queue depth, connection, health and saturation. Call it only on a background
diagnostics worker at a bounded cadence chosen by the application. A tracing
sink can block, so logging never belongs on capture or GUI paths. Capture workers
update metrics even if the consumer stalls. This task supplies the producer and
consumer APIs; adapters and GUI integration are implemented by their own tasks.

Payload and device context are always formatted as `[REDACTED]`; lengths, hashes
and identifier fragments are not emitted either. The `redacted` wrapper's Debug
and Display implementations never call the wrapped formatter and provide no
reveal option. Raw device error strings must not be interpolated into tracing
messages; use fixed diagnostic codes and stage counters. The numeric snapshot
contains no raw payload or device identity. This protects these APIs; arbitrary
future logging calls must use the same redaction boundary explicitly.

Run `cargo test -p exmon-domain --locked` and `cargo test -p exmon-app --locked`.
Tests check separate counters, saturation, concurrent totals, deterministic
monotonic rate arithmetic, epoch isolation, captured structured output and
producer progress while an actual logging writer is deliberately blocked.
