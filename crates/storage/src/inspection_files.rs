//! Explicit local capture reads; the parser enforces the same limit while reading.
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};
pub const MAX_BYTES: u64 = 32 * 1024 * 1024;
pub fn open(path: &Path) -> Result<impl Read, &'static str> {
    crate::settings_files::local(path).map_err(|_| "Invalid local capture path")?;
    let file = File::open(path).map_err(|_| "Cannot open capture file")?;
    let meta = file.metadata().map_err(|_| "Cannot inspect capture file")?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err("Capture must be a regular file of at most 32 MiB");
    }
    Ok(BufReader::new(file).take(MAX_BYTES + 1))
}
