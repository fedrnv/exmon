//! Single-window desktop view; all capture and storage are application workers.
pub mod charts;
pub mod hid;
pub mod incoming;
pub mod inspection;
mod network_filters;
pub mod outgoing;
pub mod settings;
pub mod shell;
pub use eframe::egui;
pub use shell::Shell;
