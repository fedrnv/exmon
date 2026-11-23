//! Typed wire values, bounded field definitions and checked numeric projection.
use crate::config::{Endian, Field, Representation};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt,
};
pub const MAX_VARIABLE_BYTES: usize = 1_048_576;
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Literal {
    Unsigned { value: u64 },
    Signed { value: i64 },
    Float32 { bits: u32 },
    Float64 { bits: u64 },
    Bytes { value: Vec<u8> },
    Text { value: String },
}
impl fmt::Debug for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FieldLiteral([REDACTED])")
    }
}
impl Literal {
    pub fn number(&self) -> Result<Number, NumericError> {
        match self {
            Self::Unsigned { value } => Ok(Number::Integer(i128::from(*value))),
            Self::Signed { value } => Ok(Number::Integer(i128::from(*value))),
            Self::Float32 { bits } => Number::real(f64::from(f32::from_bits(*bits))),
            Self::Float64 { bits } => Number::real(f64::from_bits(*bits)),
            _ => Err(NumericError::NotNumeric),
        }
    }
    fn cost(&self) -> usize {
        match self {
            Self::Bytes { value } => value.len(),
            Self::Text { value } => value.len(),
            _ => 8,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericError {
    NotNumeric,
    NonFinite,
    PrecisionLoss,
    Arithmetic,
}
#[derive(Clone, Copy, PartialEq)]
pub enum Number {
    Integer(i128),
    Real(f64),
}
impl fmt::Debug for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FieldNumber([REDACTED])")
    }
}
impl Number {
    pub fn real(value: f64) -> Result<Self, NumericError> {
        if value.is_finite() {
            Ok(Self::Real(value))
        } else {
            Err(NumericError::NonFinite)
        }
    }
    pub fn to_f64(self) -> Result<f64, NumericError> {
        match self {
            Self::Real(v) => {
                if v.is_finite() {
                    Ok(v)
                } else {
                    Err(NumericError::NonFinite)
                }
            }
            Self::Integer(v) => {
                let f = v as f64;
                if f >= i128::MAX as f64 || f < i128::MIN as f64 || f as i128 != v {
                    Err(NumericError::PrecisionLoss)
                } else {
                    Ok(f)
                }
            }
        }
    }
    pub fn compare(self, other: Self) -> Result<Ordering, NumericError> {
        match (self, other) {
            (Self::Integer(a), Self::Integer(b)) => Ok(a.cmp(&b)),
            (Self::Real(a), Self::Real(b)) => {
                if a.is_finite() && b.is_finite() {
                    a.partial_cmp(&b).ok_or(NumericError::NonFinite)
                } else {
                    Err(NumericError::NonFinite)
                }
            }
            (Self::Integer(a), Self::Real(b)) => {
                if !b.is_finite() {
                    return Err(NumericError::NonFinite);
                }
                if b >= i128::MAX as f64 {
                    return Ok(Ordering::Less);
                }
                if b < i128::MIN as f64 {
                    return Ok(Ordering::Greater);
                }
                let t = b.trunc();
                let cmp = a.cmp(&(t as i128));
                Ok(if cmp != Ordering::Equal {
                    cmp
                } else if b > t {
                    Ordering::Less
                } else if b < t {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                })
            }
            (Self::Real(a), Self::Integer(_)) => {
                other.compare(Self::Real(a)).map(Ordering::reverse)
            }
        }
    }
    pub fn project(self, scale: f64, bias: f64) -> Result<Self, NumericError> {
        if let Self::Real(v) = self
            && !v.is_finite()
        {
            return Err(NumericError::NonFinite);
        }
        if !scale.is_finite() || !bias.is_finite() {
            return Err(NumericError::NonFinite);
        }
        if scale == 1.0 && bias == 0.0 {
            return Ok(self);
        }
        if scale == 0.0 {
            return Self::real(bias);
        }
        Self::real(self.to_f64()? * scale + bias)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bits {
    pub offset: u8,
    pub width: u8,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumEntry {
    pub value: Literal,
    pub name: String,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<Literal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<Literal>,
    #[serde(default, skip_serializing_if = "false_value")]
    pub finite: bool,
    #[serde(default, skip_serializing_if = "false_value")]
    pub enum_closed: bool,
}
fn false_value(v: &bool) -> bool {
    !*v
}
impl Constraints {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Field { id: String },
    Literal { value: Literal },
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Abs,
    Min,
    Max,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefCode {
    Count,
    Id,
    Bounds,
    Type,
    Overlap,
    Transform,
    Constant,
    Enumeration,
    Constraint,
    Expression,
    Reference,
    Cycle,
    Budget,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionError {
    pub field: Option<usize>,
    pub code: DefCode,
}
impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "field definition {:?} at {:?}", self.code, self.field)
    }
}
impl std::error::Error for DefinitionError {}
fn def(index: usize, code: DefCode) -> DefinitionError {
    DefinitionError {
        field: Some(index),
        code,
    }
}
pub fn integer_layout(field: &Field) -> Option<(u8, u8, u64)> {
    if !matches!(
        field.representation,
        Representation::Unsigned | Representation::Signed
    ) || ![1, 2, 4, 8].contains(&field.length)
    {
        return None;
    }
    let bits = field.bits.unwrap_or(Bits {
        offset: 0,
        width: (field.length * 8) as u8,
    });
    if bits.width == 0 || u32::from(bits.offset) + u32::from(bits.width) > field.length * 8 {
        return None;
    }
    Some((
        bits.offset,
        bits.width,
        if bits.width == 64 {
            u64::MAX
        } else {
            (1u64 << bits.width) - 1
        },
    ))
}
pub fn is_numeric(r: Representation) -> bool {
    matches!(
        r,
        Representation::Unsigned
            | Representation::Signed
            | Representation::Float
            | Representation::Derived
    )
}
pub fn literal_fits(field: &Field, value: &Literal) -> bool {
    match (field.representation, value) {
        (Representation::Unsigned, Literal::Unsigned { value }) => {
            integer_layout(field).is_some_and(|(_, _, mask)| *value <= mask)
        }
        (Representation::Signed, Literal::Signed { value }) => {
            let Some((_, w, _)) = integer_layout(field) else {
                return false;
            };
            let v = i128::from(*value);
            v >= -(1i128 << (w - 1)) && v < 1i128 << (w - 1)
        }
        (Representation::Float, Literal::Float32 { .. }) => field.length == 4,
        (Representation::Float, Literal::Float64 { .. }) => field.length == 8,
        (Representation::Bytes, Literal::Bytes { value }) => value.len() == field.length as usize,
        (Representation::Ascii, Literal::Text { value }) => {
            value.len() == field.length as usize && value.is_ascii()
        }
        (Representation::Utf8, Literal::Text { value }) => value.len() == field.length as usize,
        (Representation::Derived, _) => value.number().is_ok(),
        _ => false,
    }
}
pub fn numeric_constraints(field: &Field, n: Number) -> bool {
    field.constraints.min.as_ref().is_none_or(|v| {
        v.number()
            .and_then(|bound| n.compare(bound))
            .is_ok_and(|o| o != Ordering::Less)
    }) && field.constraints.max.as_ref().is_none_or(|v| {
        v.number()
            .and_then(|bound| n.compare(bound))
            .is_ok_and(|o| o != Ordering::Greater)
    })
}
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
}
/// Shared config/runtime admission. Returned order is topological, never recursive.
pub fn validate(fields: &[Field], limit: u32) -> Result<Vec<usize>, DefinitionError> {
    if fields.len() > 256 || !(1..=65536).contains(&limit) {
        return Err(DefinitionError {
            field: None,
            code: DefCode::Count,
        });
    }
    let mut ids = BTreeMap::new();
    for (i, f) in fields.iter().enumerate() {
        if !valid_id(&f.id) || ids.insert(f.id.as_str(), i).is_some() {
            return Err(def(i, DefCode::Id));
        }
    }
    let mut occupied = vec![0u8; limit as usize];
    let mut cost = 0usize;
    let mut deps = vec![BTreeSet::new(); fields.len()];
    for (i, f) in fields.iter().enumerate() {
        let mut add_cost = |n: usize| -> Result<(), DefinitionError> {
            cost = cost
                .checked_add(n)
                .filter(|n| *n <= MAX_VARIABLE_BYTES)
                .ok_or_else(|| def(i, DefCode::Budget))?;
            Ok(())
        };
        add_cost(f.id.len() + f.unit.len())?;
        if !f.scale.is_finite()
            || !f.bias.is_finite()
            || f.unit.len() > 32
            || f.unit.chars().any(char::is_control)
        {
            return Err(def(i, DefCode::Transform));
        }
        let derived = f.representation == Representation::Derived;
        if derived {
            if f.offset != 0 || f.length != 0 || f.bits.is_some() || f.derived.is_none() {
                return Err(def(i, DefCode::Bounds));
            }
        } else {
            if f.length == 0 || f.offset.checked_add(f.length).is_none_or(|end| end > limit) {
                return Err(def(i, DefCode::Bounds));
            }
            if f.derived.is_some() {
                return Err(def(i, DefCode::Type));
            }
            if match f.representation {
                Representation::Unsigned | Representation::Signed => {
                    ![1, 2, 4, 8].contains(&f.length)
                }
                Representation::Float => ![4, 8].contains(&f.length),
                _ => false,
            } {
                return Err(def(i, DefCode::Type));
            }
            if let Some(bits) = f.bits {
                if !matches!(
                    f.representation,
                    Representation::Unsigned | Representation::Signed
                ) || bits.width == 0
                    || u32::from(bits.offset) + u32::from(bits.width) > f.length * 8
                {
                    return Err(def(i, DefCode::Type));
                }
                for bit in bits.offset..bits.offset + bits.width {
                    let byte = u32::from(bit) / 8;
                    let at = f.offset
                        + if f.endian == Endian::Little {
                            byte
                        } else {
                            f.length - 1 - byte
                        };
                    let flag = 1 << (bit % 8);
                    if occupied[at as usize] & flag != 0 {
                        return Err(def(i, DefCode::Overlap));
                    }
                    occupied[at as usize] |= flag;
                }
            } else {
                for at in f.offset..f.offset + f.length {
                    if occupied[at as usize] != 0 {
                        return Err(def(i, DefCode::Overlap));
                    }
                    occupied[at as usize] = 255;
                }
            }
        }
        if f.enumeration.len() > 256 {
            return Err(def(i, DefCode::Enumeration));
        }
        let mut enum_values = BTreeSet::new();
        let mut enum_names = BTreeSet::new();
        let mut derived_values = vec![];
        for entry in &f.enumeration {
            add_cost(entry.name.len())?;
            add_cost(entry.value.cost())?;
            if !crate::classification::valid_name(&entry.name)
                || !literal_fits(f, &entry.value)
                || !enum_values.insert(&entry.value)
                || !enum_names.insert(&entry.name)
            {
                return Err(def(i, DefCode::Enumeration));
            }
            if derived {
                let n = entry
                    .value
                    .number()
                    .map_err(|_| def(i, DefCode::Enumeration))?;
                if derived_values
                    .iter()
                    .any(|prior: &Number| prior.compare(n) == Ok(Ordering::Equal))
                {
                    return Err(def(i, DefCode::Enumeration));
                }
                derived_values.push(n);
            }
        }
        for value in [f.constraints.min.as_ref(), f.constraints.max.as_ref()]
            .into_iter()
            .flatten()
        {
            add_cost(value.cost())?;
            if !is_numeric(f.representation) || !literal_fits(f, value) || value.number().is_err() {
                return Err(def(i, DefCode::Constraint));
            }
        }
        if let (Some(min), Some(max)) = (&f.constraints.min, &f.constraints.max)
            && min.number().unwrap().compare(max.number().unwrap()) != Ok(Ordering::Less)
            && min.number().unwrap().compare(max.number().unwrap()) != Ok(Ordering::Equal)
        {
            return Err(def(i, DefCode::Constraint));
        }
        if f.constraints.finite && !is_numeric(f.representation)
            || f.constraints.enum_closed && f.enumeration.is_empty()
        {
            return Err(def(i, DefCode::Constraint));
        }
        if f.constraints.enum_closed
            && !f.enumeration.iter().any(|e| {
                let number = e.value.number();
                if (f.constraints.finite
                    || f.constraints.min.is_some()
                    || f.constraints.max.is_some())
                    && number.is_err()
                {
                    return false;
                }
                number.is_err() || number.is_ok_and(|n| numeric_constraints(f, n))
            })
        {
            return Err(def(i, DefCode::Constraint));
        }
        if let Some(value) = &f.constant {
            add_cost(value.cost())?;
            if !literal_fits(f, value)
                || (f.constraints.finite
                    || f.constraints.min.is_some()
                    || f.constraints.max.is_some())
                    && value.number().is_err()
                    && is_numeric(f.representation)
                || value.number().is_ok_and(|n| !numeric_constraints(f, n))
                || f.constraints.enum_closed
                    && !f.enumeration.iter().any(|e| {
                        if derived {
                            e.value.number().unwrap().compare(value.number().unwrap())
                                == Ok(Ordering::Equal)
                        } else {
                            e.value == *value
                        }
                    })
            {
                return Err(def(i, DefCode::Constant));
            }
        }
        if let Some(expression) = &f.derived {
            if expression.is_empty() || expression.len() > 64 {
                return Err(def(i, DefCode::Expression));
            }
            let mut depth = 0;
            for op in expression {
                match op {
                    Operation::Field { id } => {
                        add_cost(id.len())?;
                        let &index = ids
                            .get(id.as_str())
                            .ok_or_else(|| def(i, DefCode::Reference))?;
                        if !is_numeric(fields[index].representation) {
                            return Err(def(i, DefCode::Type));
                        }
                        deps[i].insert(index);
                        depth += 1;
                    }
                    Operation::Literal { value } => {
                        add_cost(value.cost())?;
                        if value.number().is_err() {
                            return Err(def(i, DefCode::Type));
                        }
                        depth += 1;
                    }
                    Operation::Neg | Operation::Abs => {
                        if depth < 1 {
                            return Err(def(i, DefCode::Expression));
                        }
                    }
                    _ => {
                        if depth < 2 {
                            return Err(def(i, DefCode::Expression));
                        }
                        depth -= 1;
                    }
                }
            }
            if depth != 1 {
                return Err(def(i, DefCode::Expression));
            }
        }
    }
    let mut order = Vec::with_capacity(fields.len());
    let mut done = vec![false; fields.len()];
    while order.len() < fields.len() {
        let Some(index) =
            (0..fields.len()).find(|i| !done[*i] && deps[*i].iter().all(|d| done[*d]))
        else {
            return Err(DefinitionError {
                field: None,
                code: DefCode::Cycle,
            });
        };
        done[index] = true;
        order.push(index);
    }
    Ok(order)
}
