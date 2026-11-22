//! Bounded JSON boundary; no file or device I/O and no implicit migration.
use exmon_domain::config::{ConfigError, MachineBindings, ProjectConfig, SCHEMA_VERSION};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const MAX_DOCUMENT_BYTES: usize = 1_048_576;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("configuration exceeds 1048576 bytes; split the document")]
    TooLarge,
    #[error(
        "invalid configuration JSON at line {line}, column {column}; check required fields, types and unknown or duplicate keys"
    )]
    Json { line: usize, column: usize },
    #[error("unsupported schema version {0}; use version 1 or an explicit migration tool")]
    Version(u32),
    #[error(transparent)]
    Validation(#[from] ConfigError),
}
fn json_error(error: serde_json::Error) -> LoadError {
    // Never format serde's message: it may contain sensitive untrusted text.
    LoadError::Json {
        line: error.line(),
        column: error.column(),
    }
}
fn decode<T: DeserializeOwned>(input: &[u8]) -> Result<T, LoadError> {
    if input.len() > MAX_DOCUMENT_BYTES {
        return Err(LoadError::TooLarge);
    }
    #[derive(Deserialize)]
    struct Version {
        schema_version: u32,
    }
    let version: Version = serde_json::from_slice(input).map_err(json_error)?;
    if version.schema_version != SCHEMA_VERSION {
        return Err(LoadError::Version(version.schema_version));
    }
    serde_json::from_slice(input).map_err(json_error)
}
fn encode(value: &impl Serialize) -> Result<Vec<u8>, LoadError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(json_error)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(LoadError::TooLarge);
    }
    Ok(bytes)
}
pub fn load_project(input: &[u8]) -> Result<ProjectConfig, LoadError> {
    let project: ProjectConfig = decode(input)?;
    project.validate()?;
    Ok(project)
}
pub fn save_project(project: &ProjectConfig) -> Result<Vec<u8>, LoadError> {
    project.validate()?;
    encode(project)
}
pub fn load_bindings(input: &[u8], project: &ProjectConfig) -> Result<MachineBindings, LoadError> {
    let bindings: MachineBindings = decode(input)?;
    bindings.validate(project)?;
    Ok(bindings)
}
pub fn save_bindings(
    bindings: &MachineBindings,
    project: &ProjectConfig,
) -> Result<Vec<u8>, LoadError> {
    bindings.validate(project)?;
    encode(bindings)
}
