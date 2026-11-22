//! Exchange Monitor capture layer. See docs/domain-model.md for the domain contract.

pub mod adapter;
pub mod ethernet;
#[cfg(feature = "native-pcap")]
pub mod native_pcap;
pub mod native_serial;
pub mod serial;
