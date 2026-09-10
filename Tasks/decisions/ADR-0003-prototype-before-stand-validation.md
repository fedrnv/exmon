# ADR-0003 — Build a complete virtual prototype before stand validation

Status: Accepted  
Date: 2026-09-10

## Decision

Complete the end-to-end Exchange Monitor prototype before relying on the physical exchange stand. All transport adapters are represented by deterministic mocks, replay sources, loopback, or virtual devices so the same application pipeline and GUI can be exercised.

The prototype includes interface selection, raw and classified incoming views, outgoing templates, framing, fields, charts, moving averages, one-shot and scheduled transmission, configuration, diagnostics, and session persistence.

Physical RS-485, USB, CAN, timing, electrical, and real-device behavior is a subsequent validation phase. Missing hardware blocks only the affected physical evidence, not implementation of the remaining product.

## Consequences

Backend traits and fixtures must be implemented early. Hardware-specific completion claims require stand evidence. Agents continue through all mockable and platform-independent tasks before requesting owner action.
