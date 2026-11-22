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
EXM-014 remains In Progress: approved physical-device fidelity and the future UI
selection interaction tests are unavailable. This implementation cannot close
those acceptance criteria from mock/source inspection alone.
