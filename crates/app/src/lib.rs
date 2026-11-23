//! Exchange Monitor app layer. See docs/domain-model.md for the domain contract.

pub mod charts;
pub mod diagnostics;
pub mod hid_worker;
pub mod incoming;
pub mod inspection;
pub mod network_filters;
pub mod outgoing;
pub mod scheduler;
pub mod sessions;
pub mod settings;
pub mod shell;
pub mod transmission;

/// Initialize process logging once at the application entry point.
///
/// The caller supplies the filter explicitly; device input never controls it.
/// An existing subscriber is reported as an error rather than panicking.
pub fn init_logging(filter: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = tracing_subscriber::EnvFilter::try_new(filter)?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()?;
    tracing::debug!("Exchange Monitor logging initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn invalid_logging_filter_is_rejected() {
        assert!(super::init_logging("[invalid").is_err());
    }
}
