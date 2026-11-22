//! Single-window desktop view; all capture and storage are application workers.
pub mod hid;
pub mod shell;
pub use eframe::egui;
pub use shell::Shell;
