# Live chart tabs

Choose Charts in the main window, expand Select numeric series and select fields
from validated protocol definitions. Each of up to four tabs supports eight
series, labelled with field identity and units. Solid lines show admitted raw
engineering projections and dashed lines show trailing moving averages. Empty
units mean the definition provides no unit. Traffic returns to the RX/TX split;
capture and charts keep running while a different view is selected.

Chart sample window accepts 1..4096 retained samples per series. Chart average
length accepts 1..window. Apply chart window validates both before applying;
invalid text preserves prior settings and shows an error. Changing window or
average clears that chart's history and starts a fresh average. Adding/removing
series preserves unchanged series. A new capture generation clears all histories.
Settings are currently session-local; layout persistence never stores samples,
TX permissions or enabled schedules.

Time axis uses seconds relative to the first prepared field in this capture
session; Index axis uses each selected series' own admitted sample index.
Autoscale follows retained rendered extrema. Zoom buttons or the mouse wheel
zoom the view; drag the plot to pan. These actions disable autoscale. Reset chart
view resumes autoscaling. Manual chart ranges require four finite increasing
limits with finite nonzero spans. Invalid ranges retain the prior view. Shared
Y ranges are in each series' labelled units; use separate tabs for incompatible
units. This is an observed-sample plot, not reconstructed continuous wire data.

The incoming worker prepares checked finite f64 engineering projections without
parsing display strings. Invalid fields, checksum failures, unknown/truncated
quality, nonfinite values and integer precision loss are gaps, never zero.
Out-of-order timestamps are gaps. A gap breaks connecting lines and resets the
trailing average. Partial average windows divide by the available valid sample
count. The bounded rolling sum uses scaled contributions, periodically rebased;
nonrepresentable average results remain absent. Index exhaustion stops admitting
points rather than losing exact index identity.

The application chart worker owns history, averages and ordered min/max bucket
decimation. It retains at most 4096 samples and 512 rendered points per series;
raw and average extrema, endpoints and gap segment identities survive
presentation decimation. Raw observations and incoming selection/retention remain
unchanged. Prepared batches are routed before incoming display pause/eviction.
Upstream capture/processing/display losses remain their separate counters; chart
input drops, per-series invalid samples, retention evictions and rendered
point reductions are separate. A full chart input queue drops a batch immediately
and breaks line/average continuity when the missing batch is crossed.

The input queue admits eight batches of at most 96 prepared rows and 1MiB charged
row data each. Configuration is a bounded latest pending update, with at most
four charts/eight series and bounded labels. Snapshots share immutable point
arrays. History and queues are bounded, independent of session duration; native
capture, renderer caches and externally retained snapshots have separate budgets.
The GUI only draws bounded prepared points and handles controls; it performs no
capture, field decoding, file I/O or averaging. No chart action grants TX access.

Focused tests compare averages against independent deterministic vectors, cover
200000 samples and full 4x8x4096 history bounds, extrema/gaps/invalid configuration,
real incoming worker projections and queue pressure. Production GUI tests cover
series, tabs, windows, axes, zoom/pan, ranges and sustained capture plus inspected
900x600/1360x900, dark/light, 1x/1.5x images. Run the bounded hidden native Windows
scenario with `cargo run -p exmon-gui --example native_charts_smoke --locked`.
Physical/macOS and packaged/release qualification remain EXM-035/036/037.
