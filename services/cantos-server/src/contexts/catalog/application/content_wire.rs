//! Raw Content Identity transport admission; storage uses the same constructors.
use cantos_contracts as dto;

use super::CatalogStoreError;
use crate::contexts::catalog::domain::{
    content::*, ContentUnitId, OwnerId, ScriptRef, SourceRef, WorkId,
};
use crate::platform::store::StoreError;

fn invalid(path: &str, rule: &str) -> CatalogStoreError {
    StoreError::InvalidRequestFields(vec![dto::FieldIssue {
        path: path.into(),
        rule: rule.into(),
    }])
    .into()
}

pub(super) fn content_error(error: ContentError) -> CatalogStoreError {
    let (field, rule) = match error {
        ContentError::InvalidId => ("id", "invalid_uuid"),
        ContentError::InvalidText => ("details.body.text", "invalid_text"),
        ContentError::InvalidLanguage => ("details.language", "invalid_language"),
        ContentError::InvalidRights => ("details.rights_refs", "invalid_rights_refs"),
        ContentError::InvalidProvenance => ("details.provenance_note", "invalid_provenance"),
        ContentError::InvalidRevision => ("revision", "invalid_revision"),
        ContentError::InvalidTimestamp | ContentError::Encoding => {
            return StoreError::CorruptRevision.into()
        }
    };
    invalid(field, rule)
}

fn source(value: dto::CatalogSourceRef) -> Result<SourceRef, CatalogStoreError> {
    SourceRef::new(value.id, value.sha256)
        .map_err(|_| invalid("details.body.source", "invalid_source"))
}
fn script(value: dto::CatalogScriptRef) -> Result<ScriptRef, CatalogStoreError> {
    if crate::platform::ids::canonical_uuid(&value.id).as_deref() != Some(value.id.as_str()) {
        return Err(invalid("details.body.script.id", "invalid_uuid"));
    }
    ScriptRef::new(value.id, value.revision, value.export_digest).map_err(|error| {
        if error == crate::contexts::catalog::domain::CatalogError::InvalidRevision {
            invalid("details.body.script.revision", "invalid_revision")
        } else {
            invalid("details.body.script", "invalid_script")
        }
    })
}

pub fn validate_revision_id(value: &str) -> Result<ContentRevisionId, CatalogStoreError> {
    ContentRevisionId::parse(value).map_err(content_error)
}

pub fn admit_append(
    unit: &str,
    request: dto::AppendContentRequest,
) -> Result<(String, super::AppendContent), CatalogStoreError> {
    let command = super::AppendContent {
        unit_id: crate::contexts::catalog::wire::validate_unit_id(unit)
            .map_err(StoreError::InvalidRequestFields)?,
        id: validate_revision_id(&request.id)?,
        expected_revision: request.expected_revision,
        expected_work_revision: request.expected_work_revision,
        details: admit_details(request.details)?,
    };
    Ok((request.operation_id, command))
}

pub fn admit_details(value: dto::ContentDetails) -> Result<ContentDetails, CatalogStoreError> {
    let body = match value.body {
        dto::ContentBody::Recording {} => ContentBody::Recording,
        dto::ContentBody::Text {
            text,
            source: original,
        } => ContentBody::Text {
            text: ManuscriptText::new(&text).map_err(content_error)?,
            source: original.map(source).transpose()?,
        },
        dto::ContentBody::Source { source: original } => ContentBody::Source {
            source: source(original)?,
        },
        dto::ContentBody::Script {
            script: reference,
            source: original,
        } => ContentBody::Script {
            script: script(reference)?,
            source: original.map(source).transpose()?,
        },
    };
    ContentDetails::new(
        value.language,
        body,
        value.rights_refs,
        value.provenance_note,
    )
    .map_err(content_error)
}

pub fn response(
    value: &ContentRevision,
) -> Result<dto::ContentRevisionResponse, CatalogStoreError> {
    let details = serde_json::from_value(
        serde_json::to_value(value.details()).map_err(|_| StoreError::CorruptRevision)?,
    )
    .map_err(|_| StoreError::CorruptRevision)?;
    Ok(dto::ContentRevisionResponse {
        id: value.id().as_str().into(),
        work_id: value.work_id().as_str().into(),
        unit_id: value.unit_id().as_str().into(),
        revision: value.revision(),
        details,
        content_digest: value.content_digest().as_str().into(),
        revision_digest: value
            .revision_digest()
            .map_err(content_error)?
            .as_str()
            .into(),
        recorded_by: value.recorded_by().as_str().into(),
        recorded_at: value.recorded_at(),
    })
}

/// No deserializer can bypass typed text, ID or digest reconstruction.
pub(in crate::contexts::catalog) fn restore(
    raw: dto::ContentRevisionResponse,
) -> Result<ContentRevision, CatalogStoreError> {
    let value = ContentRevision::new(ContentRevisionFacts {
        id: ContentRevisionId::parse(&raw.id).map_err(content_error)?,
        work_id: WorkId::parse(&raw.work_id)?,
        unit_id: ContentUnitId::parse(&raw.unit_id)?,
        revision: raw.revision,
        details: admit_details(raw.details.clone())?,
        recorded_by: OwnerId::parse(&raw.recorded_by)?,
        recorded_at: raw.recorded_at,
    })
    .map_err(content_error)?;
    if response(&value)? != raw {
        return Err(StoreError::CorruptRevision.into());
    }
    Ok(value)
}

pub fn summary(value: &ContentRevision) -> Result<dto::ContentRevisionSummary, CatalogStoreError> {
    Ok(dto::ContentRevisionSummary {
        id: value.id().as_str().into(),
        revision: value.revision(),
        content_digest: value.content_digest().as_str().into(),
        revision_digest: value
            .revision_digest()
            .map_err(content_error)?
            .as_str()
            .into(),
        recorded_by: value.recorded_by().as_str().into(),
        recorded_at: value.recorded_at(),
    })
}

#[cfg(test)]
mod properties {
    use proptest::prelude::*;

    use super::*;
    use unicode_normalization::UnicodeNormalization;

    fn revision(text: &str) -> ContentRevision {
        ContentRevision::new(ContentRevisionFacts {
            id: ContentRevisionId::parse("00000000-0000-0000-0000-000000000001").unwrap(),
            work_id: WorkId::parse("00000000-0000-0000-0000-000000000002").unwrap(),
            unit_id: ContentUnitId::parse("00000000-0000-0000-0000-000000000003").unwrap(),
            revision: 1,
            details: ContentDetails::new(
                "vi-VN".into(),
                ContentBody::Text {
                    text: ManuscriptText::new(text).unwrap(),
                    source: None,
                },
                vec![],
                "Synthetic property fixture".into(),
            )
            .unwrap(),
            recorded_by: OwnerId::parse("lan").unwrap(),
            recorded_at: 1000,
        })
        .unwrap()
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 128, rng_seed: proptest::test_runner::RngSeed::Fixed(20_261_014), ..ProptestConfig::default() })]
        #[test]
        fn canonical_text_reconstruction_is_idempotent_and_nfd_invariant(
            chunks in proptest::collection::vec(prop::sample::select(vec!["Á", "A\u{301}", "Đ", "a", " ", "\n", "\r", "\r\n", "\t", "🎭", "\u{344}"]), 0..64)
        ) {
            let raw = format!("Ngày {}", chunks.concat());
            let text = ManuscriptText::new(&raw).unwrap();
            let again = ManuscriptText::new(text.as_str()).unwrap();
            prop_assert_eq!(&text, &again);
            prop_assert!(!text.as_str().contains('\r'));
            let decomposed = raw.nfd().collect::<String>();
            let first = revision(&raw);
            let equivalent = revision(&decomposed);
            prop_assert_eq!(&first, &equivalent);
            let dto = response(&first).unwrap();
            let reconstructed = restore(dto).unwrap();
            prop_assert_eq!(&first, &reconstructed);
            prop_assert_eq!(first.revision_digest().unwrap(), reconstructed.revision_digest().unwrap());
        }
    }
}
