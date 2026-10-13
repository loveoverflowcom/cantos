//! Allocation-free admission of the narrow wire profile before prost allocates values.
use super::{MAX_PROTOBUF_BYTES, MAX_PROTOBUF_DEPTH, MAX_PROTOBUF_FIELDS, MAX_PROTOBUF_ISSUES};
use crate::wire::DecodeError;

#[derive(Clone, Copy)]
pub(super) enum Schema {
    Validate,
    Save,
    Revision,
    Issue,
    Validation,
}
#[derive(Clone, Copy)]
enum Field {
    String,
    Uint64,
    RepeatedIssue,
}
impl Schema {
    fn field(self, number: u64) -> Option<Field> {
        match (self, number) {
            (Self::Validate, 1)
            | (Self::Save, 2 | 3)
            | (Self::Revision, 1 | 3..=7)
            | (Self::Issue, 1 | 2) => Some(Field::String),
            (Self::Save, 1) | (Self::Revision, 2) => Some(Field::Uint64),
            (Self::Validation, 1) => Some(Field::RepeatedIssue),
            _ => None,
        }
    }
}

pub(super) fn admit(schema: Schema, bytes: &[u8]) -> Result<(), DecodeError> {
    if bytes.len() > MAX_PROTOBUF_BYTES {
        return Err(DecodeError::TooLarge);
    }
    let mut budget = MAX_PROTOBUF_FIELDS;
    scan(schema, bytes, 1, &mut budget)
}

fn scan(schema: Schema, bytes: &[u8], depth: usize, budget: &mut usize) -> Result<(), DecodeError> {
    if depth > MAX_PROTOBUF_DEPTH {
        return Err(DecodeError::TooDeep);
    }
    let mut cursor = 0;
    let mut seen = 0_u16;
    let mut issues = 0;
    while cursor < bytes.len() {
        *budget = budget.checked_sub(1).ok_or(DecodeError::TooLarge)?;
        let key = read_varint(bytes, &mut cursor)?;
        let number = key >> 3;
        let wire_type = key & 7;
        if number == 0 || number > (1 << 29) - 1 {
            return Err(DecodeError::Malformed);
        }
        let field = schema.field(number).ok_or(DecodeError::Shape)?;
        let repeated = matches!(field, Field::RepeatedIssue);
        let bit = 1_u16 << number;
        if !repeated && seen & bit != 0 {
            return Err(DecodeError::Shape);
        }
        seen |= bit;
        match field {
            Field::Uint64 if wire_type == 0 => {
                read_varint(bytes, &mut cursor)?;
            }
            Field::String | Field::RepeatedIssue if wire_type == 2 => {
                let length = read_varint(bytes, &mut cursor)?;
                let length = usize::try_from(length).map_err(|_| DecodeError::Truncated)?;
                let end = cursor.checked_add(length).ok_or(DecodeError::Truncated)?;
                let content = bytes.get(cursor..end).ok_or(DecodeError::Truncated)?;
                cursor = end;
                if repeated {
                    issues += 1;
                    if issues > MAX_PROTOBUF_ISSUES {
                        return Err(DecodeError::TooLarge);
                    }
                    scan(Schema::Issue, content, depth + 1, budget)?;
                } else {
                    std::str::from_utf8(content).map_err(|_| DecodeError::InvalidUtf8)?;
                }
            }
            _ => return Err(DecodeError::Shape),
        }
    }
    Ok(())
}

/// Canonical varints only. Ten bytes at most, no overflow or redundant final zero.
fn read_varint(bytes: &[u8], cursor: &mut usize) -> Result<u64, DecodeError> {
    let mut value = 0_u64;
    for index in 0..10 {
        let byte = *bytes.get(*cursor).ok_or(DecodeError::Truncated)?;
        *cursor += 1;
        if index == 9 && byte > 1 {
            return Err(DecodeError::Malformed);
        }
        value |= u64::from(byte & 127) << (index * 7);
        if byte & 128 == 0 {
            if index > 0 && byte == 0 {
                return Err(DecodeError::Malformed);
            }
            return Ok(value);
        }
    }
    Err(DecodeError::Malformed)
}
