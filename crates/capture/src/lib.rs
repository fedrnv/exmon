//! Exchange Monitor capture layer. See docs/domain-model.md for the domain contract.

pub mod adapter;
pub mod can;
pub mod ethernet;
pub mod hid;
pub mod native_can;
#[cfg(feature = "native-hid")]
pub mod native_hid;
#[cfg(feature = "native-pcap")]
pub mod native_pcap;
pub mod native_serial;
#[cfg(feature = "native-usb")]
pub mod native_usb;
pub mod native_usbmon;
pub mod serial;
pub mod sockets;
pub mod synthetic;
pub mod usb_transfers;
pub mod usbmon;
pub mod virtual_can;
