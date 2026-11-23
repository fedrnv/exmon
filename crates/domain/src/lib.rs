//! Exchange Monitor domain layer. See docs/domain-model.md for the domain contract.

use serde::{Deserialize, Serialize};

pub mod checksum;
pub mod classification;
pub mod config;
pub mod fields;
pub mod metrics;
pub mod network_filter;
pub mod observation;
pub mod schedule;

/// Direction relative to the selected capture interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Rx,
    Tx,
    Unknown,
}

/// Failure shared by domain validation boundaries.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("captured length {captured} exceeds original length {original}")]
    InvalidLength { captured: usize, original: usize },
}

/// Check length metadata without allocating or interpreting untrusted bytes.
pub fn validate_lengths(captured: usize, original: Option<usize>) -> Result<(), ValidationError> {
    if let Some(original) = original
        && captured > original
    {
        return Err(ValidationError::InvalidLength { captured, original });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_roundtrip_and_unknown_input() {
        for direction in [Direction::Rx, Direction::Tx, Direction::Unknown] {
            let json = serde_json::to_string(&direction).unwrap();
            assert_eq!(serde_json::from_str::<Direction>(&json).unwrap(), direction);
        }
        assert!(serde_json::from_str::<Direction>("\"sideways\"").is_err());
    }

    #[test]
    fn length_validation_preserves_unknown_and_truncated_semantics() {
        assert_eq!(validate_lengths(0, Some(0)), Ok(()));
        assert_eq!(validate_lengths(2, Some(4)), Ok(()));
        assert_eq!(validate_lengths(usize::MAX, None), Ok(()));
        let error = validate_lengths(4, Some(2)).unwrap_err();
        assert_eq!(
            error,
            ValidationError::InvalidLength {
                captured: 4,
                original: 2
            }
        );
        assert_eq!(
            error.to_string(),
            "captured length 4 exceeds original length 2"
        );
    }
}
