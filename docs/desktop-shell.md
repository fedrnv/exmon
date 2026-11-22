# Desktop shell

Run the native egui/eframe prototype with:

```text
cargo run -p exmon-gui --locked
```

The top bar selects a transport and one of two explicitly synthetic interfaces.
Start capture begins deterministic RX observations on an application worker.
Stop revokes the request independently of the data queue; another explicit Start
is needed to resume. Selection is disabled while starting/running/stopping.
The effective filter is all synthetic observations. This shell does not open
physical interfaces, send data or arm schedules. Network-labelled synthetic
observations contain byte patterns, not qualified Ethernet/IP protocol fixtures.

Incoming is on the left and Outgoing on the right, separated by a draggable
vertical edge. Incoming retains the latest 1024 observations and renders only
visible rows. Clear view affects display history. Outgoing is a disabled empty
state until the independently accepted template/transmission tasks are integrated.
Theme can be changed during capture. Source loss, application queue drops,
display decimation and history evictions remain separate indicators.

## Worker and memory boundaries

`exmon_app::shell::CaptureWorker` owns construction/discovery/open/configure,
`Session::pump`, observations and source cleanup. The synthetic source records
its construction thread and rejects polling from another thread. The GUI never
owns a transport adapter or calls capture methods. Commands/results use bounded
channels: four start commands, eight batches of at most 32 observations, each
with at most 64 bytes. The common Session queue is bounded to 64 items/4096 bytes.
Stop/shutdown use atomics plus worker wake-up, so a full result queue cannot
delay them. A generation tags every run; stale result batches are discarded.
The view drains at most four batches per frame and retains at most 1024 records.
Only visible rows format up to twelve payload bytes. There is no unbounded
drain, append-only display history or GUI-thread capture/decoding/storage I/O.

When the display channel is full, capture counts continue and the skipped
observations increment display decimation. That is distinct from common Session
application drops and backend loss. History evictions describe retention, not
backend loss. The synthetic source has no unknown external backend loss.

## Safe preferences

A separate `PreferencesWorker` loads and writes `exmon_storage::layout::Layout`.
Only schema version, logical window width/height, split ratio and theme persist.
Capture state, selected devices, filters, raw data, TX permissions and schedules
are absent; unknown JSON fields are rejected. Files are limited to 4096 bytes,
width to 900..3840, height to 600..2160 and split to 0.25..0.75. Nonfinite values,
invalid versions and unknown themes fail explicitly. Defaults remain usable.
Writes use a new temporary file, flush/sync and same-directory rename; a failed
write reports an unavailable-save notice without stopping capture. Existing
valid data survives invalid new settings. Resize/drag updates are coalesced
after 250 ms of quiet. Framework persistence is disabled so eframe does not
write preferences on the GUI thread. Worker shutdown flushes the last accepted
layout and joins during application teardown; OS filesystem delays can affect
shutdown, not normal frame rendering.

The default location is `ExchangeMonitor/layout-v1.json` below LOCALAPPDATA on
Windows, `~/Library/Application Support` on macOS, or XDG_CONFIG_HOME/`~/.config`
on Linux. No absolute screen position is saved. Missing environment roots or
unreadable/corrupt preferences show a notice and use defaults. Reopening always
starts idle with no selected physical interface and no transmission authority.

## Local verification

```text
cargo test -p exmon-storage --test layout --locked
cargo test -p exmon-app --test shell --locked
cargo test -p exmon-gui --test shell --locked -- --nocapture
cargo run -p exmon-gui --example native_shell_smoke --locked
```

GUI interaction tests exercise the production view with its real background
worker: Start/Stop, theme, clear and transport selection while batches arrive.
A bounded one-second responsiveness scenario checks frame progress and reports
the slowest frame (200 ms ceiling after warm-up). It is shell acceptance, not a
performance/soak benchmark. Wgpu renders screenshots at 900x600 and 1360x900
logical pixels with light/dark and 1x/1.5x scale into ignored target output.
The native smoke uses a hidden viewport, the actual eframe integration, mock
traffic, an eight-second deadline and a screenshot written on another thread.
It never opens a physical transport. The screenshot must be inspected before
recording native visual acceptance. Other OS/packaged behavior belongs to
EXM-035/037; no release, install, signing or stand qualification is inferred.

Upstream APIs were checked against
[eframe 0.37](https://docs.rs/eframe/0.37.0/eframe/) and
[egui_kittest 0.37](https://docs.rs/egui_kittest/0.37.0/egui_kittest/).
