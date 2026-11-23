//! Preserved message drafts and atomic field/HEX/checksum transactions; no I/O.
use crate::{
    checksum::{self, Plan, Registry, Status},
    fields::{self, Decoded, Schema},
};
use exmon_domain::{
    checksum::ByteRange,
    config::Protocol,
    fields::{DefinitionError, Literal, Number},
};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Input,
    EditLimit,
    Hex,
    Bounds,
    ReadOnly,
    Revision,
    Definition(DefinitionError),
    Field(fields::Error),
    Checksum(checksum::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "message codec: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Report validity is distinct from numeric chart availability (e.g. raw IEEE NaN).
#[derive(Debug, Clone)]
pub struct Report {
    pub fields: Vec<Decoded>,
    pub checksums: Vec<Status>,
}
impl Report {
    pub fn is_valid(&self) -> bool {
        self.fields.iter().all(|f| f.violations.is_empty())
            && self.checksums.iter().all(|s| s.result.is_ok())
    }
}
#[derive(Debug)]
pub struct Invalid {
    pub report: Report,
}
impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("message validation failed")
    }
}
impl std::error::Error for Invalid {}

/// A byte snapshot only. This is never interface permission or send authority.
pub struct Validated {
    bytes: Vec<u8>,
    revision: u64,
}
impl Validated {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
}
impl fmt::Debug for Validated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedMessage")
            .field("length", &self.bytes.len())
            .field("revision", &self.revision)
            .finish()
    }
}

pub enum Edit<'a> {
    Field {
        index: usize,
        value: &'a Literal,
    },
    Number {
        index: usize,
        value: Number,
        tolerance: f64,
    },
    Label {
        index: usize,
        label: &'a str,
    },
    Hex {
        offset: u32,
        text: &'a str,
    },
}
pub struct Codec {
    schema: Schema,
    checksums: Plan,
    limit: usize,
}
impl fmt::Debug for Codec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageCodec")
            .field("fields", &self.schema.len())
            .field("limit", &self.limit)
            .finish()
    }
}
impl Codec {
    pub fn new(protocol: &Protocol, registry: &Registry) -> Result<Self, Error> {
        let schema = Schema::for_protocol(protocol).map_err(Error::Definition)?;
        let checksums = Plan::new(protocol, registry).map_err(Error::Checksum)?;
        let limit = protocol
            .framing
            .limit(protocol.escape)
            .map_err(|_| Error::Input)? as usize;
        Ok(Self {
            schema,
            checksums,
            limit,
        })
    }
    pub fn decode(&self, bytes: &[u8]) -> Result<Draft<'_>, Error> {
        if bytes.len() > self.limit {
            return Err(Error::Input);
        }
        Ok(Draft {
            codec: self,
            original: bytes.to_vec(),
            current: bytes.to_vec(),
            revision: 0,
        })
    }
    fn report(&self, bytes: &[u8]) -> Report {
        Report {
            fields: self
                .schema
                .decode(bytes)
                .expect("draft length admitted by immutable codec"),
            checksums: self.checksums.inspect(bytes),
        }
    }
}

pub struct Draft<'a> {
    codec: &'a Codec,
    original: Vec<u8>,
    current: Vec<u8>,
    revision: u64,
}
impl fmt::Debug for Draft<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageDraft")
            .field("length", &self.current.len())
            .field("revision", &self.revision)
            .finish()
    }
}
impl Draft<'_> {
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    pub fn bytes(&self) -> &[u8] {
        &self.current
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Lossless serialization, including invalid captured bytes. Use prepare for validation.
    pub fn encode(&self) -> Vec<u8> {
        self.current.clone()
    }
    pub fn inspect(&self) -> Report {
        self.codec.report(&self.current)
    }
    pub fn prepare(&self) -> Result<Validated, Invalid> {
        let report = self.inspect();
        if !report.is_valid() {
            return Err(Invalid { report });
        }
        Ok(Validated {
            bytes: self.encode(),
            revision: self.revision,
        })
    }
    /// Ordered batch; last explicit edit wins. HEX may create an invalid inspectable draft.
    pub fn edit(&mut self, edits: &[Edit<'_>]) -> Result<Report, Error> {
        if edits.len() > 256 {
            return Err(Error::EditLimit);
        }
        // Bound all replacement work before cloning the message or parsing HEX.
        let mut cost = 0usize;
        let mut text_cost = 0usize;
        for edit in edits {
            let length = match edit {
                Edit::Hex { text, .. } => {
                    text_cost = text_cost
                        .checked_add(text.len())
                        .filter(|n| *n <= 3 * 65536)
                        .ok_or(Error::EditLimit)?;
                    hex_length(text)?
                }
                Edit::Field { index, .. }
                | Edit::Number { index, .. }
                | Edit::Label { index, .. } => {
                    self.codec.schema.field(*index).ok_or(Error::Bounds)?.length as usize
                }
            };
            cost = cost
                .checked_add(length)
                .filter(|n| *n <= 65536)
                .ok_or(Error::EditLimit)?;
        }
        let mut candidate = self.current.clone();
        for edit in edits {
            match edit {
                Edit::Hex { offset, text } => {
                    let data = parse_hex(text)?;
                    let range = ByteRange {
                        offset: *offset,
                        length: data.len() as u32,
                    };
                    self.check_write(range)?;
                    candidate[range.offset as usize..range.end().unwrap() as usize]
                        .copy_from_slice(&data);
                }
                Edit::Field { index, value } => {
                    self.check_field(*index)?;
                    self.codec
                        .schema
                        .encode_field(*index, value, &mut candidate)
                        .map_err(Error::Field)?;
                }
                Edit::Number {
                    index,
                    value,
                    tolerance,
                } => {
                    self.check_field(*index)?;
                    self.codec
                        .schema
                        .encode_number(*index, *value, *tolerance, &mut candidate)
                        .map_err(Error::Field)?;
                }
                Edit::Label { index, label } => {
                    self.check_field(*index)?;
                    self.codec
                        .schema
                        .encode_label(*index, label, &mut candidate)
                        .map_err(Error::Field)?;
                }
            }
        }
        self.codec
            .checksums
            .update(&self.current, &mut candidate, false)
            .map_err(Error::Checksum)?;
        self.commit(candidate)
    }
    /// Explicitly repair every checksum, including preexisting captured mismatches.
    pub fn repair_checksums(&mut self) -> Result<Report, Error> {
        let mut candidate = self.current.clone();
        self.codec
            .checksums
            .update(&self.current, &mut candidate, true)
            .map_err(Error::Checksum)?;
        self.commit(candidate)
    }
    fn check_field(&self, index: usize) -> Result<(), Error> {
        let field = self.codec.schema.field(index).ok_or(Error::Bounds)?;
        // Leave virtual read-only diagnostics to the field codec.
        if field.length != 0 {
            self.check_write(ByteRange {
                offset: field.offset,
                length: field.length,
            })?;
        }
        Ok(())
    }
    fn check_write(&self, range: ByteRange) -> Result<(), Error> {
        if range
            .end()
            .is_none_or(|end| end as usize > self.current.len())
        {
            return Err(Error::Bounds);
        }
        if self.codec.checksums.owned(range) {
            return Err(Error::ReadOnly);
        }
        Ok(())
    }
    fn commit(&mut self, candidate: Vec<u8>) -> Result<Report, Error> {
        let next = if candidate == self.current {
            self.revision
        } else {
            self.revision.checked_add(1).ok_or(Error::Revision)?
        };
        let report = self.codec.report(&candidate);
        for status in &report.checksums {
            if let Err(
                issue @ (checksum::Issue::Algorithm
                | checksum::Issue::Panic
                | checksum::Issue::ResultWidth),
            ) = status.result
            {
                return Err(Error::Checksum(checksum::Error {
                    checksum: Some(status.index),
                    issue,
                }));
            }
        }
        self.current = candidate;
        self.revision = next;
        Ok(report)
    }
}
fn hex_length(text: &str) -> Result<usize, Error> {
    if text.len() > 3 * 65536 {
        return Err(Error::EditLimit);
    }
    let mut digits = 0usize;
    for byte in text.bytes() {
        if byte.is_ascii_hexdigit() {
            digits += 1;
        } else if !byte.is_ascii_whitespace() || !digits.is_multiple_of(2) {
            return Err(Error::Hex);
        }
    }
    if digits == 0 || !digits.is_multiple_of(2) {
        return Err(Error::Hex);
    }
    Ok(digits / 2)
}
fn parse_hex(text: &str) -> Result<Vec<u8>, Error> {
    let length = hex_length(text)?;
    let mut out = Vec::with_capacity(length);
    let mut high = None;
    for digit in text.bytes().filter(|b| !b.is_ascii_whitespace()) {
        let value = match digit {
            b'0'..=b'9' => digit - b'0',
            b'a'..=b'f' => digit - b'a' + 10,
            b'A'..=b'F' => digit - b'A' + 10,
            _ => return Err(Error::Hex),
        };
        if let Some(h) = high.take() {
            out.push(h * 16 + value);
        } else {
            high = Some(value);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_revision_never_commits_changed_bytes() {
        let protocol: Protocol = serde_json::from_str(
            r#"{"id":"p","name":"p","framing":{"kind":"datagram"},"fields":[]}"#,
        )
        .unwrap();
        let codec = Codec::new(&protocol, &Registry::default()).unwrap();
        let mut draft = codec.decode(&[1]).unwrap();
        draft.revision = u64::MAX;
        assert!(draft.edit(&[]).is_ok());
        assert_eq!(
            draft
                .edit(&[Edit::Hex {
                    offset: 0,
                    text: "02"
                }])
                .unwrap_err(),
            Error::Revision
        );
        assert_eq!(draft.bytes(), [1]);
        assert_eq!(draft.revision(), u64::MAX);
    }
}
