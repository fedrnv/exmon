//! Only safe layout data; call filesystem functions from a storage worker.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    Dark,
    Light,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub schema: u32,
    pub width: f32,
    pub height: f32,
    pub split: f32,
    pub theme: Theme,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            schema: 1,
            width: 1200.,
            height: 760.,
            split: 0.55,
            theme: Theme::Dark,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutError {
    Invalid,
    Io,
}
impl Layout {
    pub fn valid(self) -> bool {
        self.schema == 1
            && self.width.is_finite()
            && self.height.is_finite()
            && self.split.is_finite()
            && (900.0..=3840.).contains(&self.width)
            && (600.0..=2160.).contains(&self.height)
            && (0.25..=0.75).contains(&self.split)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, LayoutError> {
        if bytes.len() > 4096 {
            return Err(LayoutError::Invalid);
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| LayoutError::Invalid)?;
        if !value.valid() {
            return Err(LayoutError::Invalid);
        }
        Ok(value)
    }
}
pub fn load(path: &Path) -> Result<Option<Layout>, LayoutError> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(LayoutError::Io),
    };
    let mut bytes = vec![];
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| LayoutError::Io)?;
    Layout::decode(&bytes).map(Some)
}
pub fn save(path: &Path, layout: Layout) -> Result<(), LayoutError> {
    if !layout.valid() {
        return Err(LayoutError::Invalid);
    }
    let parent = path.parent().ok_or(LayoutError::Io)?;
    fs::create_dir_all(parent).map_err(|_| LayoutError::Io)?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temporary = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| LayoutError::Io)?;
    let result = (|| {
        let bytes = serde_json::to_vec(&layout).map_err(|_| LayoutError::Invalid)?;
        file.write_all(&bytes).map_err(|_| LayoutError::Io)?;
        file.sync_all().map_err(|_| LayoutError::Io)?;
        drop(file);
        fs::rename(&temporary, path).map_err(|_| LayoutError::Io)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
