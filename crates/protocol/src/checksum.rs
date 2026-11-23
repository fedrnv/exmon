//! Trusted Rust checksum extensions receive only bounded declared coverage.
use exmon_domain::{
    checksum::{self, ByteRange, Checksum},
    config::{Endian, Protocol},
};
use std::{
    collections::BTreeMap,
    fmt,
    panic::{RefUnwindSafe, catch_unwind},
    sync::Arc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    Registry,
    Definition,
    UnknownAlgorithm,
    Width,
    Bounds,
    Algorithm,
    Panic,
    ResultWidth,
    Mismatch,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub checksum: Option<usize>,
    pub issue: Issue,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checksum {:?}: {:?}", self.checksum, self.issue)
    }
}
impl std::error::Error for Error {}

/// A finite, repeatable iterator over covered bytes; no access to the full message.
#[derive(Clone, Copy)]
pub struct Input<'a> {
    message: &'a [u8],
    ranges: &'a [ByteRange],
}
impl Input<'_> {
    pub fn bytes(&self) -> impl Iterator<Item = u8> + '_ {
        self.ranges.iter().flat_map(|r| {
            self.message[r.offset as usize..r.end().unwrap() as usize]
                .iter()
                .copied()
        })
    }
    pub fn len(&self) -> usize {
        self.ranges.iter().map(|r| r.length as usize).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
impl fmt::Debug for Input<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChecksumInput")
            .field("bytes", &self.len())
            .finish()
    }
}

/// Only application-linked, trusted implementations: pure, deterministic, bounded,
/// nonblocking and panic-free. This interface is not a native-code sandbox.
pub trait Algorithm: Send + Sync + RefUnwindSafe {
    fn calculate(&self, input: Input<'_>) -> Result<u64, Failure>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failure;
#[derive(Clone)]
struct Entry {
    width: u32,
    algorithm: Arc<dyn Algorithm>,
}
#[derive(Clone)]
pub struct Registry {
    entries: BTreeMap<String, Entry>,
}
impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChecksumRegistry")
            .field("count", &self.entries.len())
            .finish()
    }
}
impl Registry {
    pub fn empty() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
    pub fn register(
        &mut self,
        key: &str,
        width: u32,
        algorithm: Arc<dyn Algorithm>,
    ) -> Result<(), Error> {
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            || ![1, 2, 4, 8].contains(&width)
            || self.entries.len() >= 64
            || self.entries.contains_key(key)
        {
            return Err(Error {
                checksum: None,
                issue: Issue::Registry,
            });
        }
        self.entries
            .insert(key.to_owned(), Entry { width, algorithm });
        Ok(())
    }
}
impl Default for Registry {
    fn default() -> Self {
        let mut registry = Self::empty();
        registry
            .register("sum8", 1, Arc::new(Builtin::Sum8))
            .unwrap();
        registry
            .register("xor8", 1, Arc::new(Builtin::Xor8))
            .unwrap();
        registry
            .register("crc16_ccitt_false", 2, Arc::new(Builtin::Crc16))
            .unwrap();
        registry
    }
}
enum Builtin {
    Sum8,
    Xor8,
    Crc16,
}
impl Algorithm for Builtin {
    fn calculate(&self, input: Input<'_>) -> Result<u64, Failure> {
        Ok(match self {
            Self::Sum8 => u64::from(input.bytes().fold(0u8, u8::wrapping_add)),
            Self::Xor8 => u64::from(input.bytes().fold(0u8, |a, b| a ^ b)),
            Self::Crc16 => {
                let mut crc = 0xffffu16;
                for byte in input.bytes() {
                    crc ^= u16::from(byte) << 8;
                    for _ in 0..8 {
                        crc = if crc & 0x8000 != 0 {
                            (crc << 1) ^ 0x1021
                        } else {
                            crc << 1
                        };
                    }
                }
                u64::from(crc)
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub index: usize,
    pub result: Result<(), Issue>,
}
pub(crate) struct Plan {
    definitions: Vec<Checksum>,
    algorithms: Vec<Entry>,
    order: Vec<usize>,
}
impl Plan {
    pub(crate) fn new(protocol: &Protocol, registry: &Registry) -> Result<Self, Error> {
        let limit = protocol.framing.limit(protocol.escape).map_err(|_| Error {
            checksum: None,
            issue: Issue::Definition,
        })?;
        let order =
            checksum::validate(&protocol.checksums, &protocol.fields, limit).map_err(|e| {
                Error {
                    checksum: e.checksum,
                    issue: Issue::Definition,
                }
            })?;
        let algorithms = protocol
            .checksums
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let entry = registry.entries.get(&c.algorithm).ok_or(Error {
                    checksum: Some(i),
                    issue: Issue::UnknownAlgorithm,
                })?;
                if entry.width != c.length {
                    return Err(Error {
                        checksum: Some(i),
                        issue: Issue::Width,
                    });
                }
                Ok(entry.clone())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            definitions: protocol.checksums.clone(),
            algorithms,
            order,
        })
    }
    pub(crate) fn owned(&self, range: ByteRange) -> bool {
        self.definitions.iter().any(|c| c.output().overlaps(range))
    }
    fn calculate(&self, index: usize, message: &[u8]) -> Result<u64, Issue> {
        let definition = &self.definitions[index];
        if definition
            .output()
            .end()
            .is_none_or(|end| end as usize > message.len())
            || definition
                .ranges
                .iter()
                .any(|r| r.end().is_none_or(|end| end as usize > message.len()))
        {
            return Err(Issue::Bounds);
        }
        let value = catch_unwind(|| {
            self.algorithms[index].algorithm.calculate(Input {
                message,
                ranges: &definition.ranges,
            })
        })
        .map_err(|_| Issue::Panic)?
        .map_err(|_| Issue::Algorithm)?;
        if definition.length < 8 && value >= 1u64 << (definition.length * 8) {
            return Err(Issue::ResultWidth);
        }
        Ok(value)
    }
    pub(crate) fn inspect(&self, message: &[u8]) -> Vec<Status> {
        self.definitions
            .iter()
            .enumerate()
            .map(|(index, c)| {
                let result = self.calculate(index, message).and_then(|expected| {
                    let bytes = &message[c.offset as usize..c.output().end().unwrap() as usize];
                    let stored = bytes.iter().enumerate().fold(0u64, |value, (i, b)| {
                        value
                            | (u64::from(*b)
                                << (8 * if c.endian == Endian::Little {
                                    i
                                } else {
                                    bytes.len() - 1 - i
                                }))
                    });
                    if stored == expected {
                        Ok(())
                    } else {
                        Err(Issue::Mismatch)
                    }
                });
                Status { index, result }
            })
            .collect()
    }
    pub(crate) fn update(
        &self,
        before: &[u8],
        candidate: &mut [u8],
        force: bool,
    ) -> Result<(), Error> {
        for &i in &self.order {
            let c = &self.definitions[i];
            let dirty = c.ranges.iter().any(|r| {
                let range = (r.offset as usize).min(before.len())
                    ..(r.end().unwrap() as usize).min(before.len());
                before[range.clone()] != candidate[range]
            });
            if !force && !dirty {
                continue;
            }
            let value = self.calculate(i, candidate).map_err(|issue| Error {
                checksum: Some(i),
                issue,
            })?;
            let bytes = match c.endian {
                Endian::Little => value.to_le_bytes(),
                Endian::Big => value.to_be_bytes(),
            };
            let data = if c.endian == Endian::Little {
                &bytes[..c.length as usize]
            } else {
                &bytes[8 - c.length as usize..]
            };
            candidate[c.offset as usize..c.output().end().unwrap() as usize].copy_from_slice(data);
        }
        Ok(())
    }
}
