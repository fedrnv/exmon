# ADR-0001 — Use Rust and egui

Status: Accepted  
Date: 2026-09-10

## Decision

Implement Exchange Monitor as a Rust Cargo workspace with an egui/eframe desktop GUI. Keep capture and protocol processing outside the GUI thread and expose transport-neutral domain interfaces.

## Rationale

Rust provides native performance, controlled memory use, safe concurrency, and cross-platform packaging without a Python runtime. egui keeps the initial product in one language and supports the required custom split panels, tabs, tables, HEX views, and real-time plots.

## Consequences

Platform capture libraries and drivers remain external dependencies where the operating system requires them. Native-looking controls are secondary to predictable cross-platform behavior and high-rate visualization.
