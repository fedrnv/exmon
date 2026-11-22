# Selected asynchronous USB Bulk/Interrupt transfers

`usb_transfers::Usb<S>` owns explicit device inspection, interface claims and
completion-based transfers. The optional `native-usb` feature supplies
`NativeUsb` using [nusb 0.2.9](https://docs.rs/nusb/0.2.9/nusb/). No local unsafe
code, device reset, kernel-driver detach or active-configuration change occurs.
Device handles and endpoint queues stay in the capture backend. All descriptor
inspection, claim/release and native lifecycle operations run on a background
worker; they can invoke blocking OS setup calls.

Enumeration exposes at most 64 local device entries (VID/PID and selection index)
without opening them. `inspect(index)` is a separate explicit action that opens
only that device to inspect its configurations/interfaces/alternate settings and
endpoints. No interface claim or transfer follows implicitly. Descriptors are
bounded to 16 configurations, 32 interface/alternate entries per configuration
and 30 endpoints per entry; malformed duplicate identities, invalid endpoint
address bits and packet sizes outside 1..1024 fail. Re-inspection invalidates the
previous descriptor selection, including failed re-inspection. Raw paths, serial
numbers and driver strings are not emitted to logs or stored in fixtures.

`claim(Selection)` requires a known descriptor, matching active configuration,
interface/alternate and explicitly selected IN/OUT endpoint addresses with correct
directions. Control/isochronous endpoints are listed but return Unsupported for
this transfer implementation. Selecting an inactive configuration returns
Unsupported instead of changing device state. The selected alternate setting is
applied as part of explicit claim. Claim conflict returns Busy (which can include
a kernel-driver owner), not automatic detach. Windows requires an appropriate
WinUSB association; missing drivers/access remain external setup requirements.
These ownership boundaries follow the upstream
[nusb device documentation](https://docs.rs/nusb/0.2.9/nusb/struct.Device.html).

## Transfers and cancellation

IN is submitted explicitly by `submit_in(length)`, OUT only by
`submit_out(bytes)`. No claim, descriptor import, reconnect or poll submits an
OUT transfer. Up to one IN and one OUT are outstanding simultaneously, each
bounded to 65536 bytes. IN length must be a positive multiple of the endpoint's
maximum packet size; explicit OUT may be zero length. No unbounded buffering,
retry queue or periodic scheduler is created. Native submit uses asynchronous
endpoint queues, and completion polling waits for zero time; it is not a blocking
transfer hidden behind an async name. The application must poll at a bounded
cadence off the GUI thread and provide explicit TX authorization.

`poll(direction)` returns Pending (`None`) or a `Completion` containing direction,
endpoint, actual transferred bytes, optional actual length and a status:
Success, Timeout, Stall, Disconnected, Cancelled or Fault. A partial completion
retains its bytes and status even on failure; it is never converted to full
success or automatically resent. Completion readiness wins over an elapsed
deadline when the worker observes both in the same poll. Each request otherwise
has a monotonic timeout from submission, at most one second; an incomplete
request past its deadline closes the claim and reports Timeout.

Stop, timeout and failed completion release endpoints/interface/device and cancel
the other pending direction. When cancellation cannot confirm the actual bytes
already transferred, actual_length is None, not a fabricated zero. Cancelled
results remain available in two fixed slots and must be drained before a new
claim. Dropping the controller also releases ownership. OS resources and buffers
remain owned by nusb until its cancellation cleanup is safe; no local code frees
an active native buffer. Already submitted OUT may have irreversible effects.
Do not interpret cancellation as recalling bytes from the device.

Stall is visible and stops the claim. Recovery requires a deliberate new
inspection/claim after the application handles the error; this implementation
does not automatically clear a halt, change a configuration or resume output.
Permission, conflict, unsupported capability and disconnect setup errors have
fixed bounded codes. Platform-specific behavior and real cancellation latency
still need hardware qualification. nusb provides the native queue/ownership
semantics described in its
[endpoint API](https://docs.rs/nusb/0.2.9/nusb/struct.Endpoint.html).

This completion API is separate from the common synchronous `Adapter::transmit`
hook: returning a submitted count there would falsely claim completion. Future
application integration consumes completion events and converts their bytes and
status into the shared processing/event pipeline; it must preserve failed/partial
results and obtain explicit user authorization before every new OUT source.

## Verification and physical boundary

`cargo test -p exmon-capture --test usb_transfers --locked` covers Bulk/Interrupt
descriptors, selected endpoint type/direction, pending results, exact IN data,
partial OUT data, bounded outstanding requests, bad selections, inactive
configuration, claim conflict, stall, disconnect and cancellation. A unit test
sets an expired monotonic submission time directly to verify Timeout/unknown
actual count without sleeps. Native Windows and Linux code is compile-checked;
mock fixtures exercise the production controller, not a reference decoder.

Physical-device validation is unavailable because there is no approved exchange
stand/device or physical transfer authorization. No physical USB device was
opened/claimed, no driver changed and no OUT submitted in these tests. When an
approved device is available, record OS/backend/driver, configuration/interface/
alternate/endpoints, known synthetic IN/OUT data, timeout/stall behavior,
claim/release conflicts and cancellation latency. Keep real identifiers private.
Compile success is not evidence of physical behavior on Windows, Linux or macOS.
