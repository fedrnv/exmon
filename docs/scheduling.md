# Periodic transmission

`exmon_app::scheduler` supplies deadline arithmetic and activation handles.
`TxWorker::enable_schedule` is an explicit user command, using a current worker
ticket plus schedule/template IDs and revisions. Configuration loading, binding,
template registration, reconnecting, cloning definitions and application startup
do not call it. The first deadline is one interval after worker activation.
Keep the returned handle while the user wants the schedule enabled. Stop revokes
its atomic grant and cancellation token directly, independently of the four-item
command queue. Dropping the last handle also stops it. A replacement activation
has a fresh grant; an old handle cannot stop the replacement.

Portable schema-1 schedule definitions retain `id`, `template_id`, `interval_ms`
and disabled `enabled`. Optional additions are `count` (positive submission
attempt limit), `drift` (`fixed_rate` default or `fixed_delay`), `missed` (`skip`
default or `stop`) and `max_lateness_ms` (defaults to one interval). Default
additions are omitted when saved. Intervals are 1..86400000 ms; lateness windows
are 0..86400000 ms. Unknown fields/policies, fractional or negative counts, zero
counts and imported `enabled: true` are rejected. Definitions confer no I/O
authority. UI/controller code must resolve the configured template string to the
same immutable numeric template revision supplied to activation.

Fixed rate stays on the activation deadline grid. Fixed delay measures the next
interval from actual completion. Under load, Skip counts older elapsed deadlines
and selects at most the latest eligible opportunity; it creates no catch-up queue.
An opportunity past its lateness window is skipped too. Stop disables on a missed
period/window. Preparation checks the clock/window again immediately before the
submission fence. Skips and pre-submission rejections do not consume `count`;
actual source calls, including Busy/errors, do. Unchanged live Busy may continue
only under Skip, at a later deadline. Partial/unknown/fatal results disable without
an implicit retry. Finite count includes the final attempted source call, whether
or not delivery succeeds.

The existing single background TX worker owns all adapters and templates. It
alternates manual commands and one cyclic due-schedule selection; at most 64
schedules are retained. A pending operation serializes that port and uses the
existing one-second watchdog. Normalization, codec checks, metadata admission,
current capabilities/identity and template/session/configuration revisions share
the one-shot dispatch path. Scheduled preparation repeats the pure capability
check before taking the binding lease and the final schedule grant. Stop winning
that grant prevents the source call. A call already leased can complete after
Stop, with truthful known or unknown acceptance; native code must obey its finite
and cancellation contracts. Canceling pending work may close the shared port and
invalidate all its schedules. Explicit rebind and enable are then required.

Binding, permission, configuration or template changes revoke all grants. Known
capability loss is polled while schedules are active. Controllers must notify the
worker about external changes; this is not a guarantee of physical unplug
detection latency. Worker failure/shutdown, exhausted identities and backward or
unavailable monotonic time fail closed. No restart/reconnect rearming occurs.
The injectable Clock is trusted bounded nonblocking code, not a GUI callback.

Snapshot schedule status includes state/stop reason, next deadline, submission
attempt and missed counts, last decision/submission/completion times and maximum
lateness. Times are nanoseconds in the injected scheduling clock domain, separate
from existing observation/history timestamps. Attempt history adds schedule ID,
activation and timing, with unknown submission/completion retained as None.
Pre-submission failures retain a rejected record without a fabricated TX
observation. History remains limited to 128 records/1 MiB and reads of four; no
new payload log or unbounded timing trace exists. Retain the last snapshot when a
read reports lock contention. External retained snapshots have caller budgets.

Run `cargo test -p exmon-app --test scheduling --locked -- --nocapture` for virtual
clock/load/cancellation fixtures and actual Windows localhost UDP smoke (50 ms,
five attempts, 250 ms soft lateness ceiling, three-second bound). These host-side
measurements do not qualify electrical delivery or hard real-time timing.
Physical device/driver timing remains EXM-036 and other-platform runtime remains
EXM-035 on the later stands. GUI controls and calendar schedules are separate
tasks.
