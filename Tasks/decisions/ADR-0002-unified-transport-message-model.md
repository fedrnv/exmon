# ADR-0002 — Separate transports from messages

Status: Accepted  
Date: 2026-09-10

## Decision

Represent Ethernet frames, TCP/UDP data, serial byte streams, USB transfers, and CAN frames through transport adapters. Convert them into a shared timestamped observation model, then apply framing, classification, decoding, visualization, persistence, and transmission above that layer.

## Consequences

TCP and serial require stream framing; UDP and CAN already expose datagram/frame boundaries; raw USB capture and active USB communication are separate capabilities. Transport-specific metadata remains available without leaking backend implementation into the GUI.
