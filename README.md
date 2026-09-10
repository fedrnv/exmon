# Exchange Monitor

Exchange Monitor is a cross-platform Rust desktop application for capturing, classifying, decoding, visualizing, creating, and transmitting data across network and hardware communication interfaces.

## Product scope

- Windows, macOS, and Linux.
- Ethernet raw capture and PCAP/PCAPNG workflows.
- TCP and UDP monitoring with port filters and TCP stream reconstruction.
- Serial interfaces including RS-232, RS-422, RS-485, and USB CDC.
- USB HID and USB Bulk/Interrupt.
- CAN capture and transmission where supported.
- Automatic message classification by transport metadata, length, header, and configurable framing.
- Named message types, field definitions, HEX/ASCII inspection, charts, moving averages, manual transmission, and scheduled transmission.
- A single desktop window with interface selection, incoming and outgoing split panels, one tab per message type, and dedicated chart tabs.

Planning follows the append-only FDTasks v3 repository format under `Tasks/`.

## Technology

The accepted starting architecture is Rust with `egui/eframe`. Platform capture backends and permissions are isolated behind transport adapters. See `Tasks/decisions/`.

## Status

Planning initialized. Implementation has not started.
