//! Allocation-free admission of the narrow wire profile before prost allocates values.
use super::{MAX_PROTOBUF_BYTES, MAX_PROTOBUF_DEPTH, MAX_PROTOBUF_FIELDS, MAX_PROTOBUF_ISSUES};
use crate::contract::codec::DecodeError;

#[derive(Clone, Copy)]
pub(super) enum Schema {
    Validate,
    Save,
    Revision,
    Issue,
    Validation,
    ImageDeclarations,
    CreateImageIntent,
    ImageIntent,
    ImageVariant,
    ImageAsset,
    ImageResponse,
    ArtworkSelection,
    ClearArtwork,
    SetArtwork,
    ArtworkResponse,
}
#[derive(Clone, Copy)]
enum Field {
    String,
    Uint64,
    RepeatedIssue,
    Uint32,
    Enum(u64),
    RepeatedString,
    Message(Schema),
    Change(Schema),
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
            (Self::ImageDeclarations, 1)
            | (Self::CreateImageIntent, 1..=3)
            | (Self::ImageIntent, 1..=5 | 9 | 10)
            | (Self::ImageVariant, 1 | 6)
            | (Self::ImageAsset, 1..=5 | 9)
            | (Self::ArtworkSelection, 1..=3)
            | (Self::SetArtwork, 1)
            | (Self::ArtworkResponse, 1) => Some(Field::String),
            (Self::ImageDeclarations, 2) => Some(Field::RepeatedString),
            (Self::CreateImageIntent, 4)
            | (Self::ImageIntent, 7 | 8)
            | (Self::ImageVariant, 2)
            | (Self::ImageAsset, 8)
            | (Self::SetArtwork, 2)
            | (Self::ArtworkResponse, 2 | 4) => Some(Field::Uint64),
            (Self::ImageVariant, 4 | 5) => Some(Field::Uint32),
            (Self::CreateImageIntent, 5) | (Self::ImageVariant, 3) => Some(Field::Enum(2)),
            (Self::ImageResponse, 3) => Some(Field::Enum(3)),
            (Self::CreateImageIntent, 6) | (Self::ImageAsset, 7) => {
                Some(Field::Message(Self::ImageDeclarations))
            }
            (Self::ImageIntent, 6) => Some(Field::Message(Self::CreateImageIntent)),
            (Self::ImageAsset, 6) => Some(Field::Message(Self::ImageVariant)),
            (Self::ImageResponse, 1) => Some(Field::Message(Self::ImageIntent)),
            (Self::ImageResponse, 2) => Some(Field::Message(Self::ImageAsset)),
            (Self::ArtworkResponse, 3) => Some(Field::Message(Self::ArtworkSelection)),
            (Self::SetArtwork, 3) => Some(Field::Change(Self::ArtworkSelection)),
            (Self::SetArtwork, 4) => Some(Field::Change(Self::ClearArtwork)),
            _ => None,
        }
    }
}

pub(super) fn admit(schema: Schema, bytes: &[u8]) -> Result<(), DecodeError> {
    if bytes.len() > MAX_PROTOBUF_BYTES {
        return Err(DecodeError::TooLarge);
    }
    let mut budget = MAX_PROTOBUF_FIELDS;
    scan(schema, bytes, 1, &mut budget, Limits::PILOT)
}

#[derive(Clone, Copy)]
struct Limits {
    depth: usize,
    repeated: usize,
}
impl Limits {
    const PILOT: Self = Self {
        depth: MAX_PROTOBUF_DEPTH,
        repeated: MAX_PROTOBUF_ISSUES,
    };
    const ARTWORK: Self = Self {
        depth: super::MAX_ARTWORK_PROTOBUF_DEPTH,
        repeated: super::MAX_ARTWORK_RIGHTS_REFS,
    };
}

pub(super) fn admit_artwork(schema: Schema, bytes: &[u8]) -> Result<(), DecodeError> {
    if bytes.len() > super::MAX_ARTWORK_PROTOBUF_BYTES {
        return Err(DecodeError::TooLarge);
    }
    let mut budget = super::MAX_ARTWORK_PROTOBUF_FIELDS;
    scan(schema, bytes, 1, &mut budget, Limits::ARTWORK)
}

fn scan(
    schema: Schema,
    bytes: &[u8],
    depth: usize,
    budget: &mut usize,
    limits: Limits,
) -> Result<(), DecodeError> {
    if depth > limits.depth {
        return Err(DecodeError::TooDeep);
    }
    let mut cursor = 0;
    let mut seen = 0_u16;
    let mut repeated_count = 0;
    let mut change_seen = false;
    while cursor < bytes.len() {
        *budget = budget.checked_sub(1).ok_or(DecodeError::TooLarge)?;
        let key = read_varint(bytes, &mut cursor)?;
        let number = key >> 3;
        let wire_type = key & 7;
        if number == 0 || number > (1 << 29) - 1 {
            return Err(DecodeError::Malformed);
        }
        let field = schema.field(number).ok_or(DecodeError::Shape)?;
        // All schema-reviewed fields fit this mask. Checked shift also fails closed
        // if a future schema is extended without reviewing the admission mask.
        let bit = 1_u16
            .checked_shl(u32::try_from(number).map_err(|_| DecodeError::Shape)?)
            .ok_or(DecodeError::Shape)?;
        let repeated = matches!(field, Field::RepeatedIssue | Field::RepeatedString);
        if !repeated && seen & bit != 0 {
            return Err(DecodeError::Shape);
        }
        if matches!(field, Field::Change(_)) {
            if change_seen {
                return Err(DecodeError::Shape);
            }
            change_seen = true;
        }
        seen |= bit;
        match field {
            Field::Uint64 | Field::Uint32 | Field::Enum(_) if wire_type == 0 => {
                let value = read_varint(bytes, &mut cursor)?;
                match field {
                    Field::Uint32 if value > u64::from(u32::MAX) => {
                        return Err(DecodeError::Shape);
                    }
                    Field::Enum(maximum) if value == 0 || value > maximum => {
                        return Err(DecodeError::Shape);
                    }
                    _ => {}
                }
            }
            Field::String
            | Field::RepeatedString
            | Field::RepeatedIssue
            | Field::Message(_)
            | Field::Change(_)
                if wire_type == 2 =>
            {
                if matches!(field, Field::RepeatedString) {
                    repeated_count += 1;
                    if repeated_count > limits.repeated {
                        return Err(DecodeError::TooLarge);
                    }
                }
                let length = read_varint(bytes, &mut cursor)?;
                let length = usize::try_from(length).map_err(|_| DecodeError::Truncated)?;
                let end = cursor.checked_add(length).ok_or(DecodeError::Truncated)?;
                let content = bytes.get(cursor..end).ok_or(DecodeError::Truncated)?;
                cursor = end;
                // Preserve the older pilot's exact error priority: malformed
                // wire types/lengths precede its repeated-issue item bound.
                if matches!(field, Field::RepeatedIssue) {
                    repeated_count += 1;
                    if repeated_count > limits.repeated {
                        return Err(DecodeError::TooLarge);
                    }
                }
                match field {
                    Field::RepeatedIssue => {
                        scan(Schema::Issue, content, depth + 1, budget, limits)?
                    }
                    Field::Message(child) | Field::Change(child) => {
                        scan(child, content, depth + 1, budget, limits)?;
                    }
                    _ => {
                        std::str::from_utf8(content).map_err(|_| DecodeError::InvalidUtf8)?;
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artwork_deepest_legal_path_uses_the_full_depth_margin() {
        // Response.intent.request.declarations.provenance. Presence validation is
        // the adapter's separate obligation; this tests only the prescan primitive.
        let nested = [10, 6, 50, 4, 50, 2, 10, 0];
        assert_eq!(admit_artwork(Schema::ImageResponse, &nested), Ok(()));
        let mut budget = super::super::MAX_ARTWORK_PROTOBUF_FIELDS;
        assert_eq!(
            scan(
                Schema::ImageResponse,
                &nested,
                2,
                &mut budget,
                Limits::ARTWORK
            ),
            Err(DecodeError::TooDeep)
        );
    }

    #[test]
    fn total_field_budget_is_shared_across_nested_values_and_exact_at_256() {
        let mut budget = 3;
        assert_eq!(
            scan(
                Schema::ImageResponse,
                &[10, 6, 50, 4, 50, 2, 10, 0],
                1,
                &mut budget,
                Limits::ARTWORK
            ),
            Err(DecodeError::TooLarge)
        );
        // Legal artwork declarations hit rights=64 first. Exercise the common
        // primitive's total-field bound independently with a relaxed item budget.
        let fields = [18, 0].repeat(256);
        let limits = Limits {
            depth: 4,
            repeated: 256,
        };
        let mut budget = 256;
        assert_eq!(
            scan(Schema::ImageDeclarations, &fields, 1, &mut budget, limits),
            Ok(())
        );
        assert_eq!(budget, 0);
        let mut exceeded = fields;
        exceeded.extend([18, 0]);
        let mut budget = 256;
        assert_eq!(
            scan(Schema::ImageDeclarations, &exceeded, 1, &mut budget, limits),
            Err(DecodeError::TooLarge)
        );
    }

    #[test]
    fn every_reviewed_field_number_fits_the_checked_seen_mask() {
        let schemas = [
            Schema::Validate,
            Schema::Save,
            Schema::Revision,
            Schema::Issue,
            Schema::Validation,
            Schema::ImageDeclarations,
            Schema::CreateImageIntent,
            Schema::ImageIntent,
            Schema::ImageVariant,
            Schema::ImageAsset,
            Schema::ImageResponse,
            Schema::ArtworkSelection,
            Schema::ClearArtwork,
            Schema::SetArtwork,
            Schema::ArtworkResponse,
        ];
        for schema in schemas {
            for number in 16..=512 {
                assert!(schema.field(number).is_none());
            }
        }
    }
}
