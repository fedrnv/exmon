//! Explicit bounded settings-file operations. Never overwrite or follow external references.
use crate::config::MAX_DOCUMENT_BYTES;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Read,
    TooLarge,
    Exists,
    WriteIncomplete,
}
pub fn read(path: &Path) -> Result<Vec<u8>, Error> {
    local(path)?;
    let file = File::open(path).map_err(|_| Error::Read)?;
    let metadata = file.metadata().map_err(|_| Error::Read)?;
    if !metadata.is_file() {
        return Err(Error::Read);
    }
    if metadata.len() > MAX_DOCUMENT_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    let mut file = file.take(MAX_DOCUMENT_BYTES as u64 + 1);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|_| Error::Read)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::TooLarge);
    }
    Ok(bytes)
}
pub fn create(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    local(path)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(Error::TooLarge);
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Error::Exists
            } else {
                Error::WriteIncomplete
            }
        })?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| Error::WriteIncomplete)
}

fn local(path: &Path) -> Result<(), Error> {
    let text = path.to_str().ok_or(Error::Read)?;
    if text.is_empty() || text.len() > 4096 || text.chars().any(char::is_control) {
        return Err(Error::Read);
    }
    #[cfg(windows)]
    for component in path.components() {
        use std::path::{Component, Prefix};
        match component {
            Component::Prefix(p) => {
                if !matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) {
                    return Err(Error::Read);
                }
            }
            Component::Normal(n) => {
                let s = n.to_str().ok_or(Error::Read)?;
                let stem = s
                    .split('.')
                    .next()
                    .unwrap_or("")
                    .trim_end_matches(' ')
                    .to_ascii_uppercase();
                if s.contains(':')
                    || ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
                    || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                        && stem.len() == 4
                        && stem.as_bytes()[3].is_ascii_digit())
                {
                    return Err(Error::Read);
                }
            }
            _ => {}
        }
    }
    Ok(())
}
