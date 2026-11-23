# Session recording and recovery

Open **Sessions** in the header. Click it again to return to the active source view. Enter a local destination, database
budget (1..256 MiB) and record limit (1..1,000,000). Defaults are 64 MiB and
100,000 records. **Start recording** creates a new file; it never overwrites an
existing file or starts capture. Use the separate capture controls to monitor
the selected source. Recording is disabled at launch and after recovery.

Captured bytes may contain sensitive data. Files use ordinary filesystem access
and inherited directory permissions; choose a private directory. No captures are
uploaded or shared. Machine bindings, device serial selectors and credentials
are excluded. There is no automatic elevation, device open, TX or schedule
restoration. Stored portable definitions must pass the existing configuration
validator, including its rejection of enabled schedules.

The version-one SQLite database contains raw observations, portable definitions
and group names, normalized messages and numeric field/chart samples, chart
specifications and explicit annotations. Raw records preserve captured bytes,
original length, direction, typed transport metadata, monotonic/source timestamps
and generation/interface/epoch/sequence keys. Message samples have their own
timestamps, field IDs, units, finite engineering values or explicit gaps, source
spans and definition revision. Definitions carry their source generation; zero
identifies the initial profile snapshot before a source generation is assigned.
Chart specifications preserve selected fields, sample windows and average windows.
Numeric samples are recorded before chart display decimation; rendered plot points
and moving averages can be derived from those samples and specifications.

Raw capture batches reach the recorder before the shell's GUI output queue.
The separate HID reports worker also records RX observations before its view queue;
its collection index uses the high bit of the session-local interface key, with
its actual open epoch and receive sequence. The Sessions action remains available
while HID is open; starting recording never discovers, opens or sends HID reports.
Incoming definitions/names and processed message samples reach it before their
display queue. Pausing Traffic or failing to drain GUI updates does not pause raw
recording. Derived samples cover messages actually processed: earlier capture or
incoming queue loss can leave missing analysis. Capture backend loss, application
drops, GUI display decimation and recording input drops remain separate. Recording
drop counts refer to raw/message/metadata input attempts, not distinct physical
packets. The last known affected source boundary is preserved when available;
unknown metadata loss is not assigned invented source coordinates.

Enter **Session annotation** and click **Add annotation** while recording. The
optional source reference comes from the pinned Incoming row and may be absent
from the recording because of its time range or loss. Annotations use the bounded
control queue so a full capture-data queue does not prevent their admission.
Queued is not yet committed: observe the committed count or complete Stop.

**Stop recording** revokes admission immediately, independently of data-queue
capacity. The worker drains the bounded accepted data and annotation queues,
commits them, and records finalized status and loss. A new Start is refused until
finalization finishes. Capture continues independently. Budget exhaustion, SQLite
write failure and input loss are explicit; an incomplete recording is never
reported as lossless. Reaching the count, logical-byte or physical-page limit
stops recording rather than growing or deleting data indefinitely. A failed Start
may leave an incomplete new file; existing destinations are still never replaced.

After restart, explicitly select the existing file and click **Recover session**.
An unfinalized file is labelled interrupted. SQLite rolls back an incomplete
transaction; previously committed records survive. Recovery uses one read snapshot
and validates application/schema identity, version, counts, lengths and each
requested typed record. It can roll back the selected file's hot journal but does
not perform application-schema repair or migration. An active writer has an
exclusive SQLite lease and recovery reports Busy within a bounded wait.
Corrupt/future/foreign files fail explicitly; a failed recovery/page request
preserves the previous committed view. Semantic record validation occurs as pages
are read; opening a file is not a claim that every record has already been decoded.

Browse **Committed session records** with Previous/Next, then select a record for
details. Pages contain at most 64 records and 1 MiB of serialized record data;
one selection may stay pinned separately. Raw previews show the first 256 bytes;
complete bytes and definitions remain in the store. Recovery displays inert data:
it does not apply settings, reconnect an interface, start capture, send bytes or
enable a schedule. Full CSV/JSON export belongs to EXM-032.

The recorder owns all SQLite calls, serialization of captured data and local file
I/O. Capture and GUI producers use bounded admission without waiting for disk.
The data queue holds eight batches of at most 96 rows / 1 MiB of referenced data;
the separate control queue holds eight bounded commands. Profiles are immutable
shared references to already validated active configuration. Chart controls and
annotation text are bounded before admission. Encoded records are at most 1 MiB,
transactions at most 4 MiB, SQLite cache is 2 MiB and mmap is disabled. Typed object
and allocator overhead is separate from serialized-byte limits. SQL lengths,
columns, eight schema objects, attachment/trigger limits, query instruction budget and 100 ms busy wait
bound untrusted reads. Paths reject Windows device, UNC, alternate-stream and
reserved-device forms. SQLite extensions and dynamic SQL are not enabled.

The database uses 4096-byte pages, DELETE journaling and synchronous EXTRA, with
exclusive writer ownership. Database pages obey the configured budget; a temporary
rollback journal is a separate finite disk cost for modified pages and headers.
Recovery refuses a side journal larger than twice the hard database cap plus
1 MiB. Multi-database super-journal footer references are rejected before SQLite
opens the dataset; their embedded filenames are never read or resolved by the
application. Normal single-database hot-journal recovery remains supported.
These policies follow the upstream [SQLite PRAGMA reference](https://www.sqlite.org/pragma.html)
and the pinned [rusqlite API](https://docs.rs/rusqlite/0.43.0/rusqlite/).
The bundled dependency avoids a machine-wide SQLite installation.

Focused checks are `cargo test -p exmon-storage --test sessions --locked`,
`cargo test -p exmon-app --test sessions --locked`, and
`cargo test -p exmon-gui --test sessions --locked -- --nocapture`.
The abrupt-exit storage test runs its ignored subprocess fixture explicitly,
leaves a real hot journal, and compares two deterministic recoveries. It simulates
process interruption, not power loss or damaged hardware. UI checks exercise
actual start/annotation/stop/recovery/selection/paging controls and the supported
size/theme/DPI matrix. `cargo run -p exmon-gui --example native_sessions_smoke --locked`
records synthetic traffic in a hidden native window, stops, recovers, saves
`target/exm031-native.png` and exits within eight seconds. Physical devices,
macOS, packaged permissions and signing remain separately unqualified.
