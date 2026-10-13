//! Versioned Studio edge codec. No business, authorization, storage or Script IR interpretation.
//! Generated wire values expose presence; application DTOs stay ordinary Serde values.
use crate::{
    wire::{DecodeError, EncodeError},
    FieldIssue, RevisionResponse, SaveRevisionRequest, ScriptValidationResponse,
    ValidateScriptRequest,
};
use prost::Message;
mod generated;
mod scan;
use scan::Schema;

pub const MAX_PROTOBUF_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PROTOBUF_DEPTH: usize = 4;
pub const MAX_PROTOBUF_ISSUES: usize = 65_536;
pub const MAX_PROTOBUF_FIELDS: usize = 196_616;

/// Only reviewed pilot roots implement this trait. Values are raw DTOs, never witnesses.
pub trait ProtobufDto {
    fn encode_protobuf(&self) -> Result<Vec<u8>, EncodeError>;
    fn decode_protobuf(bytes: &[u8]) -> Result<Self, DecodeError>
    where
        Self: Sized;
}
#[inline]
pub fn encode<T: ProtobufDto + ?Sized>(value: &T) -> Result<Vec<u8>, EncodeError> {
    value.encode_protobuf()
}
#[inline]
pub fn decode<T: ProtobufDto>(bytes: &[u8]) -> Result<T, DecodeError> {
    T::decode_protobuf(bytes)
}

fn bounded_texts<'a>(texts: impl IntoIterator<Item = &'a str>) -> Result<(), EncodeError> {
    let mut bytes = 0_usize;
    for text in texts {
        bytes = bytes.checked_add(text.len()).ok_or(EncodeError)?;
        if bytes > MAX_PROTOBUF_BYTES {
            return Err(EncodeError);
        }
    }
    Ok(())
}

fn encode_message<M: Message>(value: &M) -> Result<Vec<u8>, EncodeError> {
    if value.encoded_len() > MAX_PROTOBUF_BYTES {
        return Err(EncodeError);
    }
    Ok(value.encode_to_vec())
}
fn decode_message<M: Message + Default>(schema: Schema, bytes: &[u8]) -> Result<M, DecodeError> {
    scan::admit(schema, bytes)?;
    M::decode(bytes).map_err(|_| DecodeError::Malformed)
}
fn required<T>(value: Option<T>) -> Result<T, DecodeError> {
    value.ok_or(DecodeError::Shape)
}

impl From<&ValidateScriptRequest> for generated::ValidateScriptRequest {
    fn from(value: &ValidateScriptRequest) -> Self {
        let ValidateScriptRequest { script_json } = value;
        Self {
            script_json: Some(script_json.clone()),
        }
    }
}
impl TryFrom<generated::ValidateScriptRequest> for ValidateScriptRequest {
    type Error = DecodeError;
    fn try_from(value: generated::ValidateScriptRequest) -> Result<Self, Self::Error> {
        let generated::ValidateScriptRequest { script_json } = value;
        Ok(Self {
            script_json: required(script_json)?,
        })
    }
}
impl From<&SaveRevisionRequest> for generated::SaveRevisionRequest {
    fn from(value: &SaveRevisionRequest) -> Self {
        let SaveRevisionRequest {
            expected_revision,
            operation_id,
            script_json,
        } = value;
        Self {
            expected_revision: Some(*expected_revision),
            operation_id: Some(operation_id.clone()),
            script_json: Some(script_json.clone()),
        }
    }
}
impl TryFrom<generated::SaveRevisionRequest> for SaveRevisionRequest {
    type Error = DecodeError;
    fn try_from(value: generated::SaveRevisionRequest) -> Result<Self, Self::Error> {
        let generated::SaveRevisionRequest {
            expected_revision,
            operation_id,
            script_json,
        } = value;
        Ok(Self {
            expected_revision: required(expected_revision)?,
            operation_id: required(operation_id)?,
            script_json: required(script_json)?,
        })
    }
}
impl From<&RevisionResponse> for generated::RevisionResponse {
    fn from(value: &RevisionResponse) -> Self {
        let RevisionResponse {
            script_id,
            revision,
            accepted_by,
            accepted_at,
            content_digest,
            export_digest,
            script_json,
        } = value;
        Self {
            script_id: Some(script_id.clone()),
            revision: Some(*revision),
            accepted_by: Some(accepted_by.clone()),
            accepted_at: Some(accepted_at.clone()),
            content_digest: Some(content_digest.clone()),
            export_digest: Some(export_digest.clone()),
            script_json: Some(script_json.clone()),
        }
    }
}
impl TryFrom<generated::RevisionResponse> for RevisionResponse {
    type Error = DecodeError;
    fn try_from(value: generated::RevisionResponse) -> Result<Self, Self::Error> {
        let generated::RevisionResponse {
            script_id,
            revision,
            accepted_by,
            accepted_at,
            content_digest,
            export_digest,
            script_json,
        } = value;
        Ok(Self {
            script_id: required(script_id)?,
            revision: required(revision)?,
            accepted_by: required(accepted_by)?,
            accepted_at: required(accepted_at)?,
            content_digest: required(content_digest)?,
            export_digest: required(export_digest)?,
            script_json: required(script_json)?,
        })
    }
}
impl From<&FieldIssue> for generated::FieldIssue {
    fn from(value: &FieldIssue) -> Self {
        let FieldIssue { path, rule } = value;
        Self {
            path: Some(path.clone()),
            rule: Some(rule.clone()),
        }
    }
}
impl TryFrom<generated::FieldIssue> for FieldIssue {
    type Error = DecodeError;
    fn try_from(value: generated::FieldIssue) -> Result<Self, Self::Error> {
        let generated::FieldIssue { path, rule } = value;
        Ok(Self {
            path: required(path)?,
            rule: required(rule)?,
        })
    }
}
impl From<&ScriptValidationResponse> for generated::ScriptValidationResponse {
    fn from(value: &ScriptValidationResponse) -> Self {
        let ScriptValidationResponse { issues } = value;
        Self {
            issues: issues.iter().map(generated::FieldIssue::from).collect(),
        }
    }
}
impl TryFrom<generated::ScriptValidationResponse> for ScriptValidationResponse {
    type Error = DecodeError;
    fn try_from(value: generated::ScriptValidationResponse) -> Result<Self, Self::Error> {
        let generated::ScriptValidationResponse { issues } = value;
        Ok(Self {
            issues: issues
                .into_iter()
                .map(FieldIssue::try_from)
                .collect::<Result<_, _>>()?,
        })
    }
}
macro_rules! pilot_codec {
    ($dto:ty, $wire:ty, $schema:expr; $($text:ident),+) => {
        impl ProtobufDto for $dto {
            fn encode_protobuf(&self) -> Result<Vec<u8>, EncodeError> {
                bounded_texts([$(self.$text.as_str()),+])?;
                encode_message(&<$wire>::from(self))
            }
            fn decode_protobuf(bytes: &[u8]) -> Result<Self, DecodeError> {
                decode_message::<$wire>($schema, bytes)?.try_into()
            }
        }
    };
}
pilot_codec!(
    ValidateScriptRequest,
    generated::ValidateScriptRequest,
    Schema::Validate; script_json
);
pilot_codec!(
    SaveRevisionRequest,
    generated::SaveRevisionRequest,
    Schema::Save; operation_id, script_json
);
pilot_codec!(
    RevisionResponse,
    generated::RevisionResponse,
    Schema::Revision; script_id, accepted_by, accepted_at, content_digest, export_digest, script_json
);
impl ProtobufDto for ScriptValidationResponse {
    fn encode_protobuf(&self) -> Result<Vec<u8>, EncodeError> {
        if self.issues.len() > MAX_PROTOBUF_ISSUES {
            return Err(EncodeError);
        }
        bounded_texts(
            self.issues
                .iter()
                .flat_map(|issue| [issue.path.as_str(), issue.rule.as_str()]),
        )?;
        encode_message(&generated::ScriptValidationResponse::from(self))
    }
    fn decode_protobuf(bytes: &[u8]) -> Result<Self, DecodeError> {
        decode_message::<generated::ScriptValidationResponse>(Schema::Validation, bytes)?.try_into()
    }
}
