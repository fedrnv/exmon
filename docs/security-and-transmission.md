# Privileges, privacy and transmission safety

Product safety contract defined by EXM-006, following ADR-0001 through ADR-0003
and the shared domain/configuration contracts. This document specifies required
adapter and application behavior. It does not claim those adapters or a native
application are implemented or qualified. Platform references were reviewed on
2026-11-22; adapter acceptance must recheck supported OS/driver versions.

## Privilege boundary

Run the GUI and parsers as an ordinary user. Keep OS handles and unsafe APIs in
small reviewed transport adapters. Discovery must report available, unavailable
with reason, or unsupported for each capability, including RX and TX separately.
A discovered name is not proof of permission. On access denial, report the
affected capability and the minimal manual remedy; keep mocks/replay usable.
Never silently elevate, install a driver, relax an ACL, change group membership,
disable OS protections or switch to a more privileged backend.

| Transport | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Ethernet/raw TCP/UDP observation | Npcap is an external driver dependency; honor its installed admin-only access policy. Missing driver/access means unavailable capture, not permission to auto-install or relaunch elevated. | libpcap live capture needs permitted BPF device access. Administrator-managed access is an external setup step; no automatic chmod or root GUI. | libpcap live capture requires suitable raw capture authority, commonly CAP_NET_RAW. Provision only an isolated reviewed adapter/helper if needed; do not grant capabilities to the whole GUI or a general interpreter. |
| Active TCP/UDP sockets | Socket ownership and OS firewall/access rules apply; opening an application socket does not grant visibility into another process. | Same separation between ordinary sockets and privileged passive raw capture. | Same separation; interface/firewall/network configuration is an administrator responsibility. |
| Serial including CDC and RS-232/422/485 | Open the selected COM resource through the approved driver and OS access checks. Exclusive ownership can fail if another process holds it. | Selected serial device access is governed by local device permissions and driver ownership; probe rather than assuming every user has access. | Selected tty device permissions/ACLs and distribution-managed groups apply. No hardcoded group name or world-writable device remedy. |
| USB HID/Bulk/Interrupt | Backend-compatible driver and device access are required. Replacing a driver or claiming an occupied interface requires separate explicit setup authorization. | OS driver ownership and backend support constrain access. Do not seize an interface from a system driver or bypass platform protections automatically. | Device-node permissions/ACLs and kernel interface ownership constrain access. A narrowly scoped administrator-managed rule may be needed; never grant access to every USB device. |
| CAN | Require a specifically supported vendor adapter/SDK and trusted driver; no general SocketCAN assumption. Missing supported backend is unsupported. | Require a specifically supported vendor backend and permitted device access; absent backend is unsupported. | SocketCAN exposes configured CAN interfaces through sockets. Controller setup, bitrate and bringing links up belong to a separately authorized administrator/setup path. Opening a socket never authorizes TX. |
| Offline capture/replay | Ordinary read access to the selected file; no live-capture driver privilege required. | Ordinary file access only. | Ordinary file access only. |

The libpcap capture authority and offline-file distinctions above follow its
[upstream manual](https://raw.githubusercontent.com/the-tcpdump-group/libpcap/master/pcap.3pcap.in).
Npcap installation, signing and admin-only restrictions are documented in its
[Users' Guide](https://npcap.com/guide/npcap-users-guide.html).
Windows serial handles and exclusive access follow
[Microsoft's communications resource documentation](https://learn.microsoft.com/en-us/windows/win32/devio/communications-resource-handles).
Linux CAN is a network-interface/socket API as described by the
[kernel SocketCAN documentation](https://docs.kernel.org/networking/can.html).
The remaining matrix cells are product access requirements: actual driver/OS
support must be measured by each adapter, never inferred from a transport label.

Use OS-distributed or vendor-authenticated drivers/libraries from a trusted
installation path. Verify origin, supported version/architecture and applicable
signature before loading; refuse unexpected libraries from capture directories
or the current working directory. A signed driver is not proof of safe hardware
behavior. Installation, updates, redistribution rights, helper deployment and
privilege provisioning require their own explicit setup/release authorization.
No driver-signing bypass, automatic device reset or automatic kernel-driver
detach is allowed. USB configuration changes and resets can affect device state
or conflict with an existing driver, as explained in the
[libusb caveats](https://libusb.sourceforge.io/api-1.0/libusb_caveats.html).

## Explicit transmission boundary

Application launch, configuration import/reload, session recovery, replay,
interface discovery and reconnect must create no active schedule or TX request.
Persist schedules disabled. Reject stored `enabled: true` rather than silently
restoring it. `tx_allowed` is a configuration permission, never a send command.
Captured bytes, file contents, decoded fields and AI-generated drafts have no
authority to send. A new user action is required after reconnect or invalidation.

| Event | Required transition and observable result |
| --- | --- |
| Launch/load/recovery | Disabled, no pending TX. Imported templates stay drafts or validated inactive templates. |
| Explicit one-shot Send | Validate chosen interface epoch, TX capability, permission, bytes, lengths and current template revision; enqueue one bounded request. Failure produces a bounded error and sends nothing. |
| Explicit schedule Enable | Show selected target, template and interval; validate current revisions and positive bounded interval; transition disabled -> enabled for that epoch only. |
| Timer deadline while enabled | Background worker revalidates epoch, template/config revision and enabled generation immediately before submission. A timer event is not independent authorization. |
| User Stop/disable | Revoke authorization before acknowledging stop; cancel queued scheduled requests and prevent further submissions from the revoked generation. Never wait behind the normal TX queue to process Stop. |
| Interface/permission loss, invalid config, template change, worker failure or shutdown | Revoke enabled generation, cancel queued scheduled requests, report reason and return disabled. No catch-up burst or automatic rearm. |
| Already submitted transfer | Report actual completion/cancellation/failure. Bytes accepted by a driver may be irreversible; Stop cannot promise recall from the wire. |

Serialize final authorization checks and the adapter submission boundary with
revocation so a stale checked request cannot slip past Stop. Use a cancellation
generation or an equivalent reviewed worker protocol; a GUI checkbox alone is
insufficient. Bound queues and scheduling state. Use monotonic deadlines and
skip/report missed periods instead of accumulating unbounded work. A schedule
never runs on the GUI thread. Configuration bounds are not proof that a physical
baud/bitrate/payload is safe: the adapter checks actual capability again.

Read-only monitoring still has possible electrical/protocol effects. Opening a
serial port can change control lines; CAN controllers can acknowledge frames;
USB reads are active transactions; a TCP connection sends protocol traffic.
Adapters must disclose these effects and require explicit user initiation before
physical open/connect or bus activity. Passive capture and active USB access are
distinct capabilities. Listen-only CAN and line-control behavior may be claimed
only when the selected backend/device actually supports and verifies them.
Mocks, replay and loopback remain the default path for unattended development.

## Capture files and privacy

Treat PCAP/PCAPNG, session data, protocol definitions, bindings and device bytes
as untrusted and potentially sensitive. Parse off the GUI thread with finite
input, record, nesting, length, offset, allocation and decompression bounds.
Check arithmetic before allocation/access. Reject unsupported versions/link
types explicitly; preserve source bytes within retention limits and label
truncation/loss. Do not execute file contents, load plugins/libraries from them,
resolve external references or arm TX from an imported file.

Reading a file grants no permission to capture or transmit. Saving/exporting
requires an explicit user-selected destination and must not overwrite an existing
file without explicit intent. Use ordinary user access, restrict local captures
to intended users, bound storage and show incomplete recordings/write failures.
Keep machine bindings separate from reusable protocols. Do not upload, email or
share captures automatically. Export raw data only through an explicit action;
warn in that flow when the chosen export includes raw sensitive content.

Operational logs default to fixed codes, numeric health and redacted context.
Never log payloads, device serial numbers, addresses, binding selectors, paths
or raw driver error strings by default. `exmon_app::diagnostics::redacted` hides
both Display and Debug; `log_health` emits no payload/identity fragments or hashes.
Diagnostic counts do not turn captured content into safe log text. Use bounded
errors and inspect exports separately: redacting logs does not sanitize a raw
capture or HEX view. Real captures, credentials and device identities must not
enter Git; only reviewed synthetic fixtures may be committed. File-extension
ignore rules are an aid, not a privacy guarantee.

## Verification boundary

EXM-006 acceptance combines source/document review and existing configuration
and redaction regressions. The current repository has six library crates and
no executable, adapter send implementation or launch hook; configuration load
does not perform I/O and rejects enabled schedules. This establishes today's
launch-safe baseline, not completion of future adapter/scheduler safety tests.

Subsequent runtime tasks must verify startup/reload/recovery/replay/reconnect
produce zero submissions; explicit Send produces exactly one; Enable is required
for periodic TX; Stop and invalidation defeat queued/stale requests; in-flight
outcomes remain honest; and denied privileges never cause an elevation fallback.
Use mocks and deterministic barriers for races before physical stand testing.
Physical CAN/USB/serial effects, driver installation and platform access tests
remain unverified until their separately authorized environments are available.
