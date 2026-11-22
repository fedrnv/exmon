# Selected USB HID report adapter

`Hid<S>` implements the shared capture Adapter. `NativeHid`, enabled through
`native-hid`, uses [hidapi 2.6.7](https://docs.rs/hidapi/2.6.7/hidapi/) with the
Windows native backend or Linux native/basic-udev backend; macOS uses upstream
platform support. No local unsafe code, driver replacement or elevation is used.
The native module is opt-in so mock/prototype checks need no USB access.

Enumeration exposes at most 64 USB HID collections with VID, PID, usage page and
usage; non-USB HID buses are excluded by the native source. Device paths remain
private to that source. Serial numbers, path strings and product strings are not
copied into capture metadata or Debug output. Enumeration can query OS device
descriptors/capabilities; it creates no application report handle and sends no
output report. The UI must show local enumerated identities and require explicit
selection rather than opening the first matching VID/PID.

Logical Open checks an exact current enumeration id without opening hardware.
Configure sets the maximum report buffer, 1..4096 bytes. Explicit Start opens
only the stored native path. Native Start refreshes descriptors and compares the
selected path and VID/PID/usage before open, then checks opened-handle identity
again before report I/O. A stale/missing/mismatching selection fails; it never
falls back to another device. This is identity consistency, not cryptographic
device authentication. Re-discovery while selected is rejected. Removal/error
closes the handle; reopening requires explicit Stop/close/select/start actions.

Report mode is explicit: Numbered or Unnumbered. Do not guess from a first byte
that happens to be zero. Numbered RX preserves its first report-ID byte and also
exposes the id in `Metadata::Hid`; zero is invalid in numbered mode. Unnumbered
RX preserves every byte, including the first payload byte, and exposes id zero.
VID/PID/usage and direction Rx accompany each observation. No artificial leading
zero is inserted into captured unnumbered input. Monotonic timestamp is relative
to Start. Original wire length remains unknown: HIDAPI reports returned bytes,
not an independently measured original length for an undersized read buffer.
The operator must configure buffers large enough for the approved device report
descriptor; full-buffer reads are not evidence of complete physical fidelity.

Explicit `transmit` accepts the HIDAPI output convention: report id first,
followed by payload. Unnumbered output must start with zero; numbered output must
start with a nonzero id. No id is inserted or stripped by the adapter. Size is
bounded by the configured report maximum. A write occurs only on the selected
started handle and only after this explicit method call; partial/zero results
are returned honestly, never silently retried. Backend-invalid counts/errors
close the handle. Capture lifecycle never generates TX. Feature/control reports
are outside this task's output-report implementation.

These input/output conventions follow the upstream
[HIDAPI report documentation](https://docs.rs/hidapi/2.6.7/hidapi/struct.HidDevice.html).
The native backend may select an interrupt output endpoint or fall back to a
control transfer; the adapter does not claim passive USB observation. Active HID
access and input reads are device operations requiring explicit user action.

Poll performs one 10 ms timeout read, checks cancellation before/after and returns
at most one report. It rejects a byte budget smaller than the configured report
buffer so it does not deliberately split reports to fit a smaller batch. Queuing
and slow-consumer loss accounting use the common Session. Read/write errors use
fixed permission/disconnect/backend codes; raw HIDAPI error text stays internal.
OS error classification is best effort; an unrecognized native failure remains
Backend rather than being misreported as a specific physical removal.

Stop and write require exclusive mutable worker ownership. Cancellation of reads
does not recall an already submitted output report. Native write timing/cancel
support is upstream/platform-dependent and is not claimed as a hard real-time
guarantee. The inspected Windows backend waits up to 1000 ms for an overlapped
write; its synchronous completion may report zero, which must not cause a retry.
Use a background TX worker and verify actual shutdown/Stop behavior against the
approved device before physical qualification. No GUI call may perform HID I/O.

`cargo test -p exmon-capture --test hid --locked` uses two synthetic devices and
bounded mock reports to test exact selection, no automatic opens/writes,
numbered/unnumbered fidelity, partial writes, invalid sizes, cancellation,
permission denial and removal. Native Windows and Linux compile checks are
separate from runtime qualification. No physical HID was opened for these tests.
The production UI selection boundary is now covered by audited mock interaction
tests. EXM-014 remains In Progress because approved physical-device fidelity is
unavailable; a passing virtual UI/source test does not close that criterion.

## Desktop HID controls

The shell's **HID reports** view uses `HidPanel` and `HidWorker`. Default builds
report the native backend Unsupported; enabling the existing backend is explicit:

```text
cargo run -p exmon-gui --features native-hid --locked
```

Nothing discovers or opens a HID report session on launch. Discover HID performs
bounded descriptor enumeration on a worker. Choose an exact collection and known
Numbered/Unnumbered format and capacity, then Open selected HID. A new collection
or discovery clears the previous format and output draft. Active selection is
locked until Close HID completes. RX shows VID/PID/usage, report ID, direction and
exact bytes. Native descriptor probing may itself query devices, so discovery
and report access are separate explicit actions. No device was probed for these
development checks.

Send HID report accepts HEX with the HIDAPI ID byte once, including leading zero
for unnumbered output. HEX parsing runs on the worker, bounded to 4096 decoded
bytes and 12288 ASCII input bytes. There is no schedule, automatic retry or send
on a saturated/truncated text input. The UI editor scrolls within 140 logical
pixels and disables Send at its text limit until the operator shortens/reviews
the input. There is also no automatic send
on discovery/open/reconnect. Every command includes selected id and session
generation. Failed refresh invalidates old selections. Wrong-device, stale or
revoked commands fail before a source write.

Close atomically revokes queued operations without waiting for a full result
queue or GUI-thread I/O. A worker CAS lease linearizes entry into an open/write
operation with revocation; an operation that already acquired that lease may
complete after Close was requested. UI reports closing and actual accepted-byte
completion without claiming recall or wire delivery. Native I/O latency remains
platform-dependent as described above. Final handle close and all report I/O
remain worker-owned. This one-shot HID path does not implement the future general
transmission/scheduler contracts.

There are four queued commands, eight queued reports, at most 64 discovered
collections, and 256 retained displayed reports. UI snapshots use a short
nonblocking try-lock and bounded cloning; views drain at most four reports per
frame and render visible rows. Source loss is unknown; display-channel skips
are counted separately. Device ids, output drafts, open state and modes do not
enter safe layout preferences. Application exit revokes and joins the worker;
an already pending platform write can delay teardown.

```text
cargo test -p exmon-app --test hid_worker --locked
cargo test -p exmon-gui --test hid --locked
cargo test -p exmon-gui --features native-hid --test hid --locked
cargo run -p exmon-gui --example native_hid_smoke --locked
```

Feature-enabled tests inject the same audited synthetic source into the real
worker/UI and never discover native hardware. The default Unsupported test is
compiled only without native-hid. The hidden native renderer smoke hardcodes a
synthetic two-collection source and one explicit selected report; it cannot open
physical hardware even in a feature-enabled build. It has an eight-second UI
deadline and writes the screenshot from a separate thread. Approved real-device
RX/TX fidelity, report descriptors, platform behavior and physical Stop latency
still require explicit owner-provided hardware/access and exact evidence.
