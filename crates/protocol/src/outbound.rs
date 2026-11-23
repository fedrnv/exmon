//! Worker-side framing of an explicitly prepared normalized outgoing message.
use crate::framing::{Event, Framer, Source};
use exmon_domain::config::{Endian, Escape, Framing};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Configuration,
    Bounds,
    Header,
    Ambiguous,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "outbound framing: {self:?}")
    }
}
impl std::error::Error for Error {}

/// `header` explicitly preserves bytes before a length field; it must have
/// exactly offset+width bytes. Other framing kinds require an empty header.
/// Success proves one exact message under the production framing contract.
pub fn encode(
    rule: &Framing,
    escape: Option<Escape>,
    header: &[u8],
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let limit = rule.limit(escape).map_err(|_| Error::Configuration)? as usize;
    if payload.len() > limit || header.len() > limit {
        return Err(Error::Bounds);
    }
    let mut prefix = vec![];
    let mut suffix: &[u8] = &[];
    let mut protected = vec![];
    match rule {
        Framing::LengthPrefix {
            offset,
            width,
            endian,
            ..
        } => {
            let size = *offset as usize + usize::from(*width);
            if header.len() != size {
                return Err(Error::Header);
            }
            if payload.len() as u64 >= 1u64 << (u32::from(*width) * 8) {
                return Err(Error::Bounds);
            }
            prefix.extend_from_slice(header);
            let length = payload.len() as u32;
            let bytes = match endian {
                Endian::Little => length.to_le_bytes(),
                Endian::Big => length.to_be_bytes(),
            };
            let data = if *endian == Endian::Little {
                &bytes[..usize::from(*width)]
            } else {
                &bytes[4 - usize::from(*width)..]
            };
            prefix[*offset as usize..].copy_from_slice(data);
        }
        _ if !header.is_empty() => return Err(Error::Header),
        Framing::Fixed { length } if payload.len() != *length as usize => {
            return Err(Error::Bounds);
        }
        Framing::Delimiter { bytes, .. } => {
            suffix = bytes;
            protected.push(bytes[0]);
        }
        Framing::Markers { start, end, .. } => {
            prefix.extend_from_slice(start);
            suffix = end;
            protected.extend([start[0], end[0]]);
        }
        _ => {}
    }
    if prefix.len() + payload.len() + suffix.len() > limit {
        return Err(Error::Bounds);
    }
    let mut wire = Vec::with_capacity((prefix.len() + payload.len() + suffix.len()).min(65536));
    // Start/end markers are deliberately unescaped; length header bytes use the
    // same escape representation as their payload (length counts decoded bytes).
    let mut append = |byte: u8, protect: bool| -> Result<(), Error> {
        if let Some(e) = escape
            && (byte == e.byte || protect)
        {
            if wire.len() > 65534 {
                return Err(Error::Bounds);
            }
            wire.extend([e.byte, byte ^ e.xor]);
        } else {
            if wire.len() >= 65536 {
                return Err(Error::Bounds);
            }
            wire.push(byte);
        }
        Ok(())
    };
    for byte in prefix {
        append(byte, false)?;
    }
    for byte in payload {
        append(*byte, protected.contains(byte))?;
    }
    for byte in suffix {
        append(*byte, false)?;
    }
    // A reserved escape byte cannot occur in validated marker/delimiter bytes;
    // for length headers append above correctly escapes it as ordinary data.
    let mut framer = Framer::new(rule.clone(), escape).map_err(|_| Error::Configuration)?;
    let source = Source {
        observation: 0,
        offset: 0,
    };
    let mut events = if matches!(rule, Framing::Datagram { .. }) {
        framer.push_frame(source, 0, &wire)
    } else {
        framer.push(source, 0, &wire)
    }
    .map_err(|_| Error::Bounds)?;
    events.extend(framer.finish(0).map_err(|_| Error::Bounds)?);
    let mut messages = events.iter().filter_map(|e| {
        if let Event::Message(m) = e {
            Some(m)
        } else {
            None
        }
    });
    let one = messages.next().ok_or(Error::Ambiguous)?;
    if one.bytes() != payload
        || messages.next().is_some()
        || events.iter().any(|e| matches!(e, Event::Diagnostic { .. }))
    {
        return Err(Error::Ambiguous);
    }
    Ok(wire)
}
