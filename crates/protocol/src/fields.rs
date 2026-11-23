//! Field-level worker codec. Whole-message/HEX/checksum transactions belong to EXM022.
use exmon_domain::{
    config::{Endian, Field, Protocol, Representation},
    fields::{
        self, DefCode, DefinitionError, Literal, Number, NumericError, Operation, integer_layout,
    },
};
use std::{cmp::Ordering, collections::BTreeMap, fmt};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    Input,
    Bounds,
    Type,
    ReadOnly,
    Constant,
    Range,
    Enum,
    NonFinite,
    PrecisionLoss,
    Arithmetic,
    NotNumeric,
    Dependency,
    Text,
    NonInvertible,
    Quantization,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub field: Option<usize>,
    pub issue: Issue,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "field {:?}: {:?}", self.field, self.issue)
    }
}
impl std::error::Error for Error {}
fn issue(e: NumericError) -> Issue {
    match e {
        NumericError::NotNumeric => Issue::NotNumeric,
        NumericError::NonFinite => Issue::NonFinite,
        NumericError::PrecisionLoss => Issue::PrecisionLoss,
        NumericError::Arithmetic => Issue::Arithmetic,
    }
}
#[derive(Clone)]
pub struct Decoded {
    pub index: usize,
    pub raw: Option<Literal>,
    pub raw_number: Option<Number>,
    pub number: Result<Number, Issue>,
    pub label: Option<String>,
    pub violations: Vec<Issue>,
}
impl fmt::Debug for Decoded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecodedField")
            .field("index", &self.index)
            .field("violations", &self.violations)
            .finish()
    }
}
pub struct Schema {
    fields: Vec<Field>,
    order: Vec<usize>,
    indices: BTreeMap<String, usize>,
    limit: usize,
}
impl fmt::Debug for Schema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldSchema")
            .field("fields", &self.fields.len())
            .field("limit", &self.limit)
            .finish()
    }
}
impl Schema {
    pub fn new(definitions: &[Field], limit: u32) -> Result<Self, DefinitionError> {
        let order = fields::validate(definitions, limit)?;
        Ok(Self {
            fields: definitions.to_vec(),
            order,
            indices: definitions
                .iter()
                .enumerate()
                .map(|(i, f)| (f.id.clone(), i))
                .collect(),
            limit: limit as usize,
        })
    }
    pub fn for_protocol(protocol: &Protocol) -> Result<Self, DefinitionError> {
        let limit = protocol
            .framing
            .limit(protocol.escape)
            .map_err(|_| DefinitionError {
                field: None,
                code: DefCode::Bounds,
            })?;
        Self::new(&protocol.fields, limit)
    }
    pub fn field(&self, index: usize) -> Option<&Field> {
        self.fields.get(index)
    }
    pub fn len(&self) -> usize {
        self.fields.len()
    }
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
    pub fn decode(&self, message: &[u8]) -> Result<Vec<Decoded>, Error> {
        if message.len() > self.limit {
            return Err(Error {
                field: None,
                issue: Issue::Input,
            });
        }
        let mut report: Vec<Option<Decoded>> = vec![None; self.fields.len()];
        for &index in &self.order {
            let f = &self.fields[index];
            let decoded = if let Some(expression) = &f.derived {
                match self.evaluate(expression, &report) {
                    Err(error) => Decoded {
                        index,
                        raw: None,
                        raw_number: None,
                        number: Err(error),
                        label: None,
                        violations: vec![error],
                    },
                    Ok(raw) => {
                        let violations = derived_violations(f, raw);
                        let label = f
                            .enumeration
                            .iter()
                            .find(|e| {
                                e.value
                                    .number()
                                    .is_ok_and(|v| raw.compare(v) == Ok(Ordering::Equal))
                            })
                            .map(|e| e.name.clone());
                        let number = if let Some(first) = violations.first() {
                            Err(*first)
                        } else {
                            raw.project(f.scale, f.bias).map_err(issue)
                        };
                        Decoded {
                            index,
                            raw: None,
                            raw_number: Some(raw),
                            number,
                            label,
                            violations,
                        }
                    }
                }
            } else {
                match read(f, message) {
                    Err(error) => Decoded {
                        index,
                        raw: None,
                        raw_number: None,
                        number: Err(error),
                        label: None,
                        violations: vec![error],
                    },
                    Ok(raw) => {
                        let violations = raw_violations(f, &raw);
                        let label = f
                            .enumeration
                            .iter()
                            .find(|e| e.value == raw)
                            .map(|e| e.name.clone());
                        let raw_number = raw.number().ok();
                        let number = if let Some(first) = violations.first() {
                            Err(*first)
                        } else {
                            raw.number()
                                .and_then(|n| n.project(f.scale, f.bias))
                                .map_err(issue)
                        };
                        Decoded {
                            index,
                            raw: Some(raw),
                            raw_number,
                            number,
                            label,
                            violations,
                        }
                    }
                }
            };
            report[index] = Some(decoded);
        }
        Ok(report
            .into_iter()
            .map(|r| r.expect("validated topological order covers all fields"))
            .collect())
    }
    fn evaluate(
        &self,
        expression: &[Operation],
        report: &[Option<Decoded>],
    ) -> Result<Number, Issue> {
        let mut stack = Vec::with_capacity(expression.len());
        for op in expression {
            match op {
                Operation::Field { id } => {
                    let index = *self.indices.get(id).ok_or(Issue::Dependency)?;
                    stack.push(
                        report[index]
                            .as_ref()
                            .ok_or(Issue::Dependency)?
                            .number
                            .map_err(|_| Issue::Dependency)?,
                    );
                }
                Operation::Literal { value } => stack.push(value.number().map_err(issue)?),
                Operation::Neg | Operation::Abs => {
                    let a = stack.pop().ok_or(Issue::Arithmetic)?;
                    stack.push(match a {
                        Number::Integer(v) => Number::Integer(
                            if matches!(op, Operation::Neg) {
                                v.checked_neg()
                            } else {
                                v.checked_abs()
                            }
                            .ok_or(Issue::Arithmetic)?,
                        ),
                        Number::Real(v) => Number::real(if matches!(op, Operation::Neg) {
                            -v
                        } else {
                            v.abs()
                        })
                        .map_err(issue)?,
                    });
                }
                _ => {
                    let b = stack.pop().ok_or(Issue::Arithmetic)?;
                    let a = stack.pop().ok_or(Issue::Arithmetic)?;
                    stack.push(binary(op, a, b)?);
                }
            }
        }
        stack.pop().ok_or(Issue::Arithmetic)
    }
    /// Validates one explicit field edit before touching the caller-owned destination.
    pub fn encode_field(
        &self,
        index: usize,
        value: &Literal,
        message: &mut [u8],
    ) -> Result<(), Error> {
        let fail = |issue| Error {
            field: Some(index),
            issue,
        };
        let f = self.fields.get(index).ok_or_else(|| fail(Issue::Bounds))?;
        if message.len() > self.limit {
            return Err(fail(Issue::Input));
        }
        if f.derived.is_some() {
            return Err(fail(Issue::ReadOnly));
        }
        if !fields::literal_fits(f, value) {
            return Err(fail(Issue::Type));
        }
        let start = f.offset as usize;
        let end = start + f.length as usize;
        let current = message.get(start..end).ok_or_else(|| fail(Issue::Bounds))?;
        if let Some(error) = raw_violations(f, value).first() {
            return Err(fail(*error));
        }
        let bytes = match value {
            Literal::Unsigned { value } => encode_integer(f, *value, current),
            Literal::Signed { value } => encode_integer(f, *value as u64, current),
            Literal::Float32 { bits } => match f.endian {
                Endian::Little => bits.to_le_bytes(),
                Endian::Big => bits.to_be_bytes(),
            }
            .to_vec(),
            Literal::Float64 { bits } => match f.endian {
                Endian::Little => bits.to_le_bytes(),
                Endian::Big => bits.to_be_bytes(),
            }
            .to_vec(),
            Literal::Bytes { value } => value.clone(),
            Literal::Text { value } => value.as_bytes().to_vec(),
        };
        message[start..end].copy_from_slice(&bytes);
        Ok(())
    }
    pub fn encode_label(&self, index: usize, label: &str, message: &mut [u8]) -> Result<(), Error> {
        let f = self.fields.get(index).ok_or(Error {
            field: Some(index),
            issue: Issue::Bounds,
        })?;
        if f.derived.is_some() {
            return Err(Error {
                field: Some(index),
                issue: Issue::ReadOnly,
            });
        }
        let value = &f
            .enumeration
            .iter()
            .find(|e| e.name == label)
            .ok_or(Error {
                field: Some(index),
                issue: Issue::Enum,
            })?
            .value;
        self.encode_field(index, value, message)
    }
    /// Quantization is explicit: zero tolerance requires exact numeric equality; returns actual value.
    pub fn encode_number(
        &self,
        index: usize,
        desired: Number,
        tolerance: f64,
        message: &mut [u8],
    ) -> Result<Number, Error> {
        let fail = |issue| Error {
            field: Some(index),
            issue,
        };
        let f = self.fields.get(index).ok_or_else(|| fail(Issue::Bounds))?;
        if f.derived.is_some() {
            return Err(fail(Issue::ReadOnly));
        }
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(fail(Issue::Input));
        }
        if let Number::Real(v) = desired
            && !v.is_finite()
        {
            return Err(fail(Issue::NonFinite));
        }
        if !fields::is_numeric(f.representation) {
            return Err(fail(Issue::NotNumeric));
        }
        if f.scale == 0.0 {
            return Err(fail(Issue::NonInvertible));
        }
        let raw = if f.scale == 1.0 && f.bias == 0.0 {
            desired
        } else {
            Number::real((desired.to_f64().map_err(|e| fail(issue(e)))? - f.bias) / f.scale)
                .map_err(|e| fail(issue(e)))?
        };
        let value = match f.representation {
            Representation::Unsigned | Representation::Signed => {
                let (_, width, mask) = integer_layout(f).expect("validated integer field");
                let (min, max) = if f.representation == Representation::Unsigned {
                    (0, i128::from(mask))
                } else {
                    (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
                };
                let rounded = match raw {
                    Number::Integer(v) => Number::Integer(v),
                    Number::Real(v) => Number::real(v.round()).map_err(|e| fail(issue(e)))?,
                };
                if rounded
                    .compare(Number::Integer(min))
                    .map_err(|e| fail(issue(e)))?
                    == Ordering::Less
                    || rounded
                        .compare(Number::Integer(max))
                        .map_err(|e| fail(issue(e)))?
                        == Ordering::Greater
                {
                    return Err(fail(Issue::Range));
                }
                let integer = match rounded {
                    Number::Integer(v) => v,
                    Number::Real(v) => v as i128,
                };
                if f.representation == Representation::Unsigned {
                    Literal::Unsigned {
                        value: integer as u64,
                    }
                } else {
                    Literal::Signed {
                        value: integer as i64,
                    }
                }
            }
            Representation::Float => {
                let v = raw.to_f64().map_err(|e| fail(issue(e)))?;
                if f.length == 4 {
                    Literal::Float32 {
                        bits: (v as f32).to_bits(),
                    }
                } else {
                    Literal::Float64 { bits: v.to_bits() }
                }
            }
            _ => return Err(fail(Issue::NotNumeric)),
        };
        let actual = value
            .number()
            .and_then(|n| n.project(f.scale, f.bias))
            .map_err(|e| fail(issue(e)))?;
        if !near(actual, desired, tolerance).map_err(fail)? {
            return Err(fail(Issue::Quantization));
        }
        self.encode_field(index, &value, message)?;
        Ok(actual)
    }
}
fn word(bytes: &[u8], endian: Endian) -> u64 {
    let mut out = 0u64;
    for i in 0..bytes.len() {
        let at = if endian == Endian::Big {
            i
        } else {
            bytes.len() - 1 - i
        };
        out = (out << 8) | u64::from(bytes[at]);
    }
    out
}
fn read(f: &Field, message: &[u8]) -> Result<Literal, Issue> {
    let start = f.offset as usize;
    let bytes = message
        .get(start..start + f.length as usize)
        .ok_or(Issue::Bounds)?;
    match f.representation {
        Representation::Unsigned | Representation::Signed => {
            let (shift, width, mask) = integer_layout(f).ok_or(Issue::Type)?;
            let value = (word(bytes, f.endian) >> shift) & mask;
            Ok(if f.representation == Representation::Unsigned {
                Literal::Unsigned { value }
            } else {
                Literal::Signed {
                    value: if value & (1u64 << (width - 1)) != 0 {
                        (value | !mask) as i64
                    } else {
                        value as i64
                    },
                }
            })
        }
        Representation::Float => {
            let bits = word(bytes, f.endian);
            Ok(if f.length == 4 {
                Literal::Float32 { bits: bits as u32 }
            } else {
                Literal::Float64 { bits }
            })
        }
        Representation::Bytes => Ok(Literal::Bytes {
            value: bytes.to_vec(),
        }),
        Representation::Ascii | Representation::Utf8 => {
            if f.representation == Representation::Ascii && !bytes.is_ascii() {
                return Err(Issue::Text);
            }
            Ok(Literal::Text {
                value: std::str::from_utf8(bytes)
                    .map_err(|_| Issue::Text)?
                    .to_owned(),
            })
        }
        Representation::Derived => Err(Issue::ReadOnly),
    }
}
fn encode_integer(f: &Field, value: u64, current: &[u8]) -> Vec<u8> {
    let (shift, _, mask) = integer_layout(f).expect("validated integer field");
    let bits = (word(current, f.endian) & !(mask << shift)) | ((value & mask) << shift);
    let raw = match f.endian {
        Endian::Little => bits.to_le_bytes(),
        Endian::Big => bits.to_be_bytes(),
    };
    if f.endian == Endian::Little {
        raw[..f.length as usize].to_vec()
    } else {
        raw[8 - f.length as usize..].to_vec()
    }
}
fn raw_violations(f: &Field, value: &Literal) -> Vec<Issue> {
    let mut out = vec![];
    if f.constant.as_ref().is_some_and(|c| c != value) {
        out.push(Issue::Constant);
    }
    let number = value.number();
    if f.constraints.finite && number.is_err() {
        out.push(Issue::NonFinite);
    }
    if f.constraints.min.is_some() || f.constraints.max.is_some() {
        match number {
            Ok(n) if !fields::numeric_constraints(f, n) => out.push(Issue::Range),
            Err(e) => out.push(issue(e)),
            _ => {}
        }
    }
    if f.constraints.enum_closed && !f.enumeration.iter().any(|e| e.value == *value) {
        out.push(Issue::Enum);
    }
    out
}
fn derived_violations(f: &Field, value: Number) -> Vec<Issue> {
    let mut out = vec![];
    if f.constant
        .as_ref()
        .is_some_and(|c| c.number().and_then(|n| n.compare(value)) != Ok(Ordering::Equal))
    {
        out.push(Issue::Constant);
    }
    if !fields::numeric_constraints(f, value) {
        out.push(Issue::Range);
    }
    if f.constraints.enum_closed
        && !f
            .enumeration
            .iter()
            .any(|e| e.value.number().and_then(|n| n.compare(value)) == Ok(Ordering::Equal))
    {
        out.push(Issue::Enum);
    }
    out
}
fn binary(op: &Operation, a: Number, b: Number) -> Result<Number, Issue> {
    if matches!(op, Operation::Min | Operation::Max) {
        let order = a.compare(b).map_err(issue)?;
        return Ok(
            if matches!(op, Operation::Min) == (order != Ordering::Greater) {
                a
            } else {
                b
            },
        );
    }
    if let (Number::Integer(x), Number::Integer(y)) = (a, b) {
        let value = match op {
            Operation::Add => Some(x.checked_add(y)),
            Operation::Sub => Some(x.checked_sub(y)),
            Operation::Mul => Some(x.checked_mul(y)),
            Operation::Div => {
                if y == 0 {
                    return Err(Issue::Arithmetic);
                }
                let remainder = x.checked_rem(y).ok_or(Issue::Arithmetic)?;
                if remainder == 0 {
                    Some(x.checked_div(y))
                } else {
                    None
                }
            }
            _ => return Err(Issue::Arithmetic),
        };
        if let Some(value) = value {
            return value.map(Number::Integer).ok_or(Issue::Arithmetic);
        }
    }
    let x = a.to_f64().map_err(issue)?;
    let y = b.to_f64().map_err(issue)?;
    if matches!(op, Operation::Div) && y == 0.0 {
        return Err(Issue::Arithmetic);
    }
    Number::real(match op {
        Operation::Add => x + y,
        Operation::Sub => x - y,
        Operation::Mul => x * y,
        Operation::Div => x / y,
        _ => return Err(Issue::Arithmetic),
    })
    .map_err(issue)
}
fn near(a: Number, b: Number, tolerance: f64) -> Result<bool, Issue> {
    if a.compare(b).map_err(issue)? == Ordering::Equal {
        return Ok(true);
    }
    if tolerance == 0.0 {
        return Ok(false);
    }
    if let (Number::Integer(x), Number::Integer(y)) = (a, b) {
        let delta = x
            .checked_sub(y)
            .and_then(i128::checked_abs)
            .ok_or(Issue::Arithmetic)?;
        return Ok(Number::Integer(delta)
            .compare(Number::Real(tolerance))
            .map_err(issue)?
            != Ordering::Greater);
    }
    Ok((a.to_f64().map_err(issue)? - b.to_f64().map_err(issue)?).abs() <= tolerance)
}
