# Passive USB host observation

`usbmon` is a read-only record parser and bounded reader; `native_usbmon` opens
only an explicitly selected Linux usbmon bus file. It does not claim a USB
interface or send traffic. Active HID and Bulk/Interrupt communication remain
separate modules and are never used as a fallback for missing passive capture.

The [Linux usbmon documentation](https://docs.kernel.org/usb/usbmon.html) describes
host-controller request traces rather than a complete electrical bus trace.
This implementation reads its deprecated `1u` text interface; binary/ioctl/mmap
usbmon and old `1t` records are not implemented. An administrator must prepare
usbmon/debugfs and access permissions externally. No module loading, mounting,
permission changes or device reconfiguration occurs in the application. Missing
setup returns Unsupported and access denial returns PermissionDenied.

Native Linux support requires the explicit `native-usbmon` feature. The source
enumerates bounded bus numbers under `/sys/kernel/debug/usb/usbmon` and opens
only the corresponding `<bus>u` path read-only, nonblocking, close-on-exec and
without following a final symlink. Bus zero observes all buses: UI must disclose
that scope and require explicit user initiation. Keyboard and other unrelated
traffic may be sensitive; select the narrowest authorized bus and keep raw
traces out of Git/logs. This source has no write or transmit operation.

The parser preserves submission/callback/error event, transfer type/direction,
bus/device/endpoint, numeric status and optional timing/frame/error values,
setup fields/tag, bounded ISO descriptor metadata, reported data length/data tag,
and every captured payload byte. Timestamp is converted with checked arithmetic
and stored as source time separately from monotonic ingest. Missing/short data
stays missing/short. ISO sparse-buffer data can exceed reported actual length;
that length remains metadata, not a fabricated contiguous wire size. An original
length cannot be inferred for ISO sparse buffers. Up to five printed ISO entries
are retained along with the reported total count; omitted descriptors are not
invented. URB tags are treated as opaque hexadecimal identity and are not logged.

The preceding format/ABI facts follow the linked kernel documentation. Product
bounds are 262144 bytes per line, 65536 captured payload bytes and fixed-size
setup/ISO metadata. Split fields have bounded cardinality and offsets use checked
addition. Reader buffering is 4096 bytes plus one bounded partial line. Each poll
consumes at most 4096 bytes, returning Pending if a complete line is unavailable.
Oversized input enters discard-to-newline recovery and reports an error; EOF with
an incomplete record reports an error instead of emitting invented bytes.
`Reader<R>` requires nonblocking R for live worker use; a blocking arbitrary Read
implementation does not become nonblocking by wrapping it.

`Record::into_observation(ingest_ns)` produces shared immutable observations with
`Metadata::UsbMon`, preserving capture time separately. Direction Tx in such an
observation means an observed host-to-device request, never a transmit command.
Observation Debug redacts transport details and bytes. The consumer must account
for rejected records and application queue loss. Backend drop statistics are not
implemented by this text source; report their availability as unknown, not zero.
Supported format/compile target is not equivalent to qualified live capture.

`Usbmon<NativeUsbmon>` implements the common background-worker `Adapter` for
bounded `Session` queues, explicit start/stop and cancellation. Enumeration/open
do not start capture. Snapshot truncation preserves reported size and metadata;
the descriptor is RX-only even when a passive record describes observed Tx.
The text source reports backend loss as unknown once per activation.

## Other platform evaluation

Windows USBPcap is a separate external filter driver/extcap facility. Its
[upstream project](https://desowin.org/usbpcap/index.html) documents a signed
installer and reboot requirement. EXM-016 evaluates it but does not implement or
install that backend; Windows passive USB capture therefore returns Unsupported.
The project's [capture limitations](https://desowin.org/usbpcap/capture_limitations.html)
also distinguish request traces from electrical USB packets. Never substitute
Npcap Ethernet capture or an active WinUSB transfer and call it passive USB.

Apple's [IOUSBHost documentation](https://developer.apple.com/documentation/iousbhost)
describes device access. Its
[virtual-machine device-access entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.vm.device-access)
concerns device capture for virtualization; neither is evidence that this
application can passively observe arbitrary host USB traffic. No macOS passive
backend is implemented, so it returns Unsupported. There is no elevation,
protection bypass or driver-installation fallback on either platform.

## Verification

Synthetic `usbmon` integration tests cover all four transfer kinds, three event
types, exact payloads, setup, status and ISO metadata, missing/truncated/sparse
data, malformed numbers/hex/addresses, overflow, partial nonblocking reads, EOF
and explicit Unsupported on the current Windows host. Native Linux code is
compile-checked through `cargo check -p exmon-capture --features native-usbmon
--target x86_64-unknown-linux-gnu --locked`.

Native Linux acceptance passed in portable QEMU 11.1.0 with software-only TCG,
Alpine standard 3.23.8 and Linux 6.18.60-0-lts. A read-only, emulated USB mass
storage disk supplied a known repeating 0..255 byte pattern. The production
`Usbmon<NativeUsbmon>` captured its bulk callbacks with matching bus/device,
endpoint direction, successful status, source timestamp and exact 32 printed
payload bytes from a reported 512-byte transfer. The text facility's short
capture is preserved; the missing 480 bytes are never fabricated. Two start/read/
cancel/stop rounds passed, RX-only transmit refusal stayed explicit, and backend
loss remained unknown. Missing usbmon setup returned Unsupported; an unprivileged
guest reader received PermissionDenied. These are native Linux runtime results
on emulated USB, without physical device qualification.

The opt-in tests are deliberately ignored during ordinary test runs:

```text
cargo test -p exmon-capture --features native-usbmon --test usbmon --locked native_qemu_usbmon_preserves_payload_and_lifecycle -- --ignored --exact --nocapture
```

Use only an isolated guest with no physical passthrough. The test requires
`EXMON_TEST_USBMON=qemu-fixture`, exact guest DMI fixture identities, the fixed
`sda` QEMU vendor and synthetic USB serial. It derives a nonzero selected bus
from that device's sysfs ancestry before activating capture. It never accepts an
arbitrary device path. The synthetic I/O generator is separate from the passive
source: a five-second bounded `dd` reads one sector into `/dev/null` after the
fixture checks. Capture itself still performs no active USB operation.

For reproduction, reuse the manual Alpine standard boot recipe in
[CAN verification](can.md), with networking disabled and these QEMU additions:

```text
-smbios type=1,manufacturer=EXMON-QEMU,product=EXMON-USBMON-FIXTURE
-device qemu-xhci
-drive file=<synthetic.raw>,if=none,id=synthetic,format=raw,readonly=on
-device usb-storage,drive=synthetic,serial=EXMON-SYNTHETIC
```

Create `synthetic.raw` as exactly 1 MiB with byte `i % 256` at offset `i`.
Share only the test executable on the narrow read-only artifact drive, copy it
into `/tmp/usbmon-tests` in the guest and set mode 755. Static musl cross-linking
uses the CAN recipe with feature `native-usbmon` and test `usbmon`. After boot,
debugfs is mounted by Alpine's startup; run these manual phases:

```sh
EXMON_TEST_USBMON_PHASE=missing /tmp/usbmon-tests --ignored --exact native_qemu_usbmon_setup_errors_are_explicit --nocapture
modprobe usbmon
su nobody -s /bin/sh -c 'EXMON_TEST_USBMON_PHASE=denied /tmp/usbmon-tests --ignored --exact native_qemu_usbmon_setup_errors_are_explicit --nocapture'
EXMON_TEST_USBMON=qemu-fixture /tmp/usbmon-tests --ignored --exact native_qemu_usbmon_preserves_payload_and_lifecycle --nocapture
/tmp/usbmon-tests --nocapture
poweroff
```

The missing phase must precede module loading; denied runs as an unprivileged
guest account. Do not change host permissions or install host drivers. These
fixtures do not qualify physical USB, other Linux configurations, binary usbmon,
USBPcap or a macOS passive backend. Exact runtime/source/tool hashes and outcomes
are recorded in immutable EXM-016 evidence; no raw URB identifiers are committed.
