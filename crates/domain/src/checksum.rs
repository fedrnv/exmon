//! Declarative checksum ownership, bounded coverage and dependency admission.
use crate::config::{Endian, Field};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_CHECKSUMS: usize = 32;
pub const MAX_COVERAGE: u32 = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByteRange {
    pub offset: u32,
    pub length: u32,
}
impl ByteRange {
    pub fn end(self) -> Option<u32> {
        self.offset.checked_add(self.length)
    }
    pub fn overlaps(self, other: Self) -> bool {
        if self.length == 0 || other.length == 0 {
            return false;
        }
        match (self.end(), other.end()) {
            (Some(a), Some(b)) => self.offset < b && other.offset < a,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checksum {
    pub id: String,
    pub algorithm: String,
    pub offset: u32,
    pub length: u32,
    pub endian: Endian,
    pub ranges: Vec<ByteRange>,
}
impl Checksum {
    pub fn output(&self) -> ByteRange {
        ByteRange {
            offset: self.offset,
            length: self.length,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    Count,
    Id,
    Bounds,
    Overlap,
    Coverage,
    Cycle,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub checksum: Option<usize>,
    pub code: Code,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "checksum definition {:?} at {:?}",
            self.code, self.checksum
        )
    }
}
impl std::error::Error for Error {}
fn fail(index: usize, code: Code) -> Error {
    Error {
        checksum: Some(index),
        code,
    }
}
fn key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

/// Validate before cloning or reading message data; returns deterministic topological order.
pub fn validate(checksums: &[Checksum], fields: &[Field], limit: u32) -> Result<Vec<usize>, Error> {
    if checksums.len() > MAX_CHECKSUMS || !(1..=65536).contains(&limit) || fields.len() > 256 {
        return Err(Error {
            checksum: None,
            code: Code::Count,
        });
    }
    let mut ids = BTreeSet::new();
    let mut coverage = 0u32;
    for (i, c) in checksums.iter().enumerate() {
        if !key(&c.id) || !key(&c.algorithm) || !ids.insert(c.id.as_str()) {
            return Err(fail(i, Code::Id));
        }
        let output = c.output();
        if ![1, 2, 4, 8].contains(&c.length) || output.end().is_none_or(|end| end > limit) {
            return Err(fail(i, Code::Bounds));
        }
        if checksums[..i]
            .iter()
            .any(|prior| output.overlaps(prior.output()))
        {
            return Err(fail(i, Code::Overlap));
        }
        // Inspection fields may lie wholly inside a host-owned checksum output.
        for field in fields.iter().filter(|f| f.length != 0) {
            let wire = ByteRange {
                offset: field.offset,
                length: field.length,
            };
            let end = wire
                .end()
                .filter(|end| *end <= limit)
                .ok_or_else(|| fail(i, Code::Bounds))?;
            if wire.overlaps(output) && (wire.offset < output.offset || end > output.end().unwrap())
            {
                return Err(fail(i, Code::Overlap));
            }
        }
        if c.ranges.is_empty() || c.ranges.len() > 32 {
            return Err(fail(i, Code::Coverage));
        }
        let mut previous_end = 0;
        for range in &c.ranges {
            let end = range
                .end()
                .filter(|end| *end <= limit)
                .ok_or_else(|| fail(i, Code::Bounds))?;
            if range.length == 0 || range.offset < previous_end || range.overlaps(output) {
                return Err(fail(i, Code::Coverage));
            }
            previous_end = end;
            coverage = coverage
                .checked_add(range.length)
                .filter(|v| *v <= MAX_COVERAGE)
                .ok_or_else(|| fail(i, Code::Coverage))?;
        }
    }
    let deps: Vec<Vec<usize>> = checksums
        .iter()
        .map(|c| {
            checksums
                .iter()
                .enumerate()
                .filter(|(_, other)| c.ranges.iter().any(|r| r.overlaps(other.output())))
                .map(|(i, _)| i)
                .collect()
        })
        .collect();
    let mut done = vec![false; checksums.len()];
    let mut order = Vec::with_capacity(checksums.len());
    while order.len() < checksums.len() {
        let index = (0..checksums.len())
            .find(|i| !done[*i] && deps[*i].iter().all(|d| done[*d]))
            .ok_or(Error {
                checksum: None,
                code: Code::Cycle,
            })?;
        done[index] = true;
        order.push(index);
    }
    Ok(order)
}
