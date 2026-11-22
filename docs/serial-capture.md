# Serial and RS-4xx adapter

`Serial<S>` implements the common Adapter contract. `NativeSerial` uses the safe
[serial2 API](https://docs.rs/serial2/0.2.39/serial2/struct.SerialPort.html) on
Windows/macOS/Linux. All OS calls stay in the backend; no local unsafe code or
driver installation is required. Discovery returns at most 64 local names;
`port_name(id)` is for local UI selection, not logging or portable identity.
No port opens during construction, enumeration or logical selection. Only an
explicit Start opens and configures the selected physical port. Opening may
change electrical/control-line state, so it requires explicit user initiation.

Settings expose baud (1..4000000), character size, parity, stop bits, flow control,
read/write timeout (1..1000 ms) and requested electrical mode. Defaults are 9600,
8N1, no flow control, 10 ms and unchanged electrical mode. USB CDC is a serial
stream; RS-232/RS-422 and externally controlled RS-485 use the same stream API.
The OS/device must still support the chosen settings. Platform timeout precision
and actual baud accuracy are hardware-dependent and require stand validation.

The native backend enters raw mode explicitly, applies all selected line settings
and sets finite read and write timeouts. Poll performs one bounded read and
returns one observation or an empty batch on timeout; it never spins until data
arrives. Direction is Rx, bytes are unchanged, time is relative monotonic ingest
time and metadata preserves the configuration revision. Framing operates above
this layer: arbitrary read chunks are not message boundaries.

The explicit `transmit(bytes)` operation is valid only while started. It makes
one bounded backend write and returns the accepted byte count, including partial
writes. It never silently retries an accepted prefix or interprets a configuration
load/RX packet as a send request. A reported write is submission, not confirmed
delivery. A future application TX worker must bind explicit user authorization
and revision/epoch checks to this operation. No schedule exists in this adapter.
Stop and transmit require exclusive mutable ownership; serialize them on the
worker. An in-flight write can finish within its backend timeout before Stop
acknowledgement. Cancelled polling does not itself claim to recall bytes or
provide TX authorization; the worker must process Stop before further TX.

Disconnect/read/write errors close only this adapter's port. Unrelated sessions
are untouched. Reconnect is explicit through Session close/open/configure/start,
never an automatic rearm. Fixed error codes omit port selectors and raw device
error text. Unsupported electrical modes fail rather than emulate timing with
unverified software RTS toggles. Real port names and captures must not enter Git.

## Direction control and physical limits

Linux can request RS-422 or kernel-managed RS-485 through serial2's safe
[RS-4xx support](https://docs.rs/serial2/0.2.39/serial2/rs4xx/index.html).
RS-485 settings expose full duplex, RTS inversion and delays before/after send
(0..1000 ms); hardware/software flow control is prohibited concurrently with an
RS-485 direction-control request. The backend reads effective kernel settings
back and rejects mode/duplex/polarity/delay mismatches. No termination resistor
changes are requested. Drivers can still behave incorrectly despite readback;
this is capability evidence at the API level, not electrical qualification.

Windows/macOS do not expose this Linux mode-setting API; such software direction
requests return Unsupported before port open. Unchanged mode supports ordinary
serial and externally configured automatic-direction hardware, but does not
assert that the hardware is RS-485 or electrically safe. `direction_control()`
reports whether the currently opened backend can query the native mode; before
open or on unsupported platforms it reports Unsupported. Do not infer capability
from a USB device name or from a successful ordinary serial open.

## Verification

`cargo test -p exmon-capture --test serial --locked` drives the production adapter
with a bounded in-memory duplex virtual serial peer. It checks fragmented RX
chunks `A5`, `00 2A`, exact framing input `A5 00 2A`, explicit TX `A5 00 07`, no
startup output, settings propagation, timeout, partial writes without retry,
cancellation, unsupported direction mode and reconnect isolation. Peer queues
are limited to 64 bytes. This is simulated serial, not an installed Windows VCOM
driver or Unix pseudo-terminal qualification. The common Session tests separately
verify capture queue loss/bounds and lifecycle cancellation.

Physical RS-485 testing is unavailable: no exchange stand or authorized test
transceiver is connected in this environment. Baud/timing/electrical behavior,
line polarity, turnaround, termination, flow-control wiring, disconnect under
load and real-device protocol compatibility are not marked passed. When hardware
is available, record immutable evidence with backend/OS/device alias, settings,
known synthetic RX/TX data, observed direction timing and limitations. Keep real
device identifiers out of committed evidence. Native port access and electrical
qualification remain distinct from the portable simulated prototype acceptance.
