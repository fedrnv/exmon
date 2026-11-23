//! Portable periodic settings. A definition is never runtime authority.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Drift {
    #[default]
    FixedRate,
    FixedDelay,
}
impl Drift {
    pub fn is_default(&self) -> bool {
        *self == Self::FixedRate
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Missed {
    #[default]
    Skip,
    Stop,
}
impl Missed {
    pub fn is_default(&self) -> bool {
        *self == Self::Skip
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub interval_ms: u32,
    pub count: Option<u64>,
    pub drift: Drift,
    pub missed: Missed,
    pub max_lateness_ms: Option<u32>,
}
impl Options {
    pub fn validate(self) -> Result<(), &'static str> {
        if !(1..=86_400_000).contains(&self.interval_ms)
            || self.count == Some(0)
            || self.max_lateness_ms.is_some_and(|v| v > 86_400_000)
        {
            return Err("invalid periodic interval, count or lateness window");
        }
        Ok(())
    }
    pub fn interval_ns(self) -> u64 {
        u64::from(self.interval_ms) * 1_000_000
    }
    pub fn lateness_ns(self) -> u64 {
        u64::from(self.max_lateness_ms.unwrap_or(self.interval_ms)) * 1_000_000
    }
}
