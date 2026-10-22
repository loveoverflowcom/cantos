//! Private storage DTOs deliberately cannot deserialize admitted domain values. Every restore
//! reparses identities, links and metadata through the same constructors as a new revision.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::contexts::catalog::domain::{
    CatalogError, ContentUnit, ContentUnitId, ContentUnitKind, Lifecycle, OwnerId, ScriptRef,
    SourceRef, StoredContentUnit, StoredWork, UnitDetails, Visibility, Work, WorkDetails, WorkId,
    WorkKind,
};
use crate::platform::store::StoreError;

const MAX_SNAPSHOT_BYTES: usize = 128 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWork {
    id: String,
    kind: String,
    owner_id: String,
    details: RawWorkDetails,
    lifecycle: String,
    revision: u64,
    created_at: i64,
    updated_at: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWorkDetails {
    title: String,
    synopsis: String,
    language: String,
    visibility: String,
    rights_refs: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnit {
    id: String,
    work_id: String,
    kind: String,
    details: RawUnitDetails,
    position: u32,
    lifecycle: String,
    revision: u64,
    created_at: i64,
    updated_at: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnitDetails {
    title: String,
    description: String,
    source: Option<RawSource>,
    script: Option<RawScript>,
    content_digest: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSource {
    id: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScript {
    id: String,
    revision: u64,
    export_digest: String,
}

fn admit<T>(value: Result<T, CatalogError>) -> Result<T, StoreError> {
    value.map_err(|_| StoreError::CorruptRevision)
}

fn bounded(bytes: &[u8]) -> Result<(), StoreError> {
    if bytes.is_empty() || bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(StoreError::CorruptRevision);
    }
    Ok(())
}

pub(super) fn work_kind(value: &str) -> Result<WorkKind, StoreError> {
    admit(WorkKind::parse(value))
}

pub(in super::super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    let bytes = serde_json::to_vec(value).map_err(|_| StoreError::CorruptRevision)?;
    bounded(&bytes)?;
    Ok(bytes)
}

pub(in super::super) fn decode_work(bytes: &[u8]) -> Result<Work, StoreError> {
    bounded(bytes)?;
    let raw: RawWork = serde_json::from_slice(bytes).map_err(|_| StoreError::CorruptRevision)?;
    let id = admit(WorkId::parse(&raw.id))?;
    if id.as_str() != raw.id {
        return Err(StoreError::CorruptRevision);
    }
    admit(Work::from_stored(StoredWork {
        id,
        kind: work_kind(&raw.kind)?,
        owner_id: admit(OwnerId::parse(&raw.owner_id))?,
        details: WorkDetails {
            title: raw.details.title,
            synopsis: raw.details.synopsis,
            language: raw.details.language,
            visibility: admit(Visibility::parse(&raw.details.visibility))?,
            rights_refs: raw.details.rights_refs,
        },
        lifecycle: admit(Lifecycle::parse(&raw.lifecycle))?,
        revision: raw.revision,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
    }))
}

pub(in super::super) fn decode_unit(
    bytes: &[u8],
    parent_kind: WorkKind,
) -> Result<ContentUnit, StoreError> {
    bounded(bytes)?;
    let raw: RawUnit = serde_json::from_slice(bytes).map_err(|_| StoreError::CorruptRevision)?;
    let id = admit(ContentUnitId::parse(&raw.id))?;
    let work_id = admit(WorkId::parse(&raw.work_id))?;
    if id.as_str() != raw.id || work_id.as_str() != raw.work_id {
        return Err(StoreError::CorruptRevision);
    }
    let source = raw
        .details
        .source
        .map(|source| admit(SourceRef::new(source.id, source.sha256)))
        .transpose()?;
    let script = raw
        .details
        .script
        .map(|script| {
            admit(ScriptRef::new(
                script.id,
                script.revision,
                script.export_digest,
            ))
        })
        .transpose()?;
    admit(ContentUnit::from_stored(
        StoredContentUnit {
            id,
            work_id,
            kind: admit(ContentUnitKind::parse(&raw.kind))?,
            details: UnitDetails {
                title: raw.details.title,
                description: raw.details.description,
                source,
                script,
                content_digest: raw.details.content_digest,
            },
            position: raw.position,
            lifecycle: admit(Lifecycle::parse(&raw.lifecycle))?,
            revision: raw.revision,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        },
        parent_kind,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::catalog::domain::{NewContentUnit, NewWork};

    fn work() -> Work {
        Work::new(NewWork {
            id: WorkId::parse("00000000-0000-0000-0000-000000000001").unwrap(),
            owner_id: OwnerId::parse("lan").unwrap(),
            kind: WorkKind::Podcast,
            details: WorkDetails {
                title: "Ánh đèn cuối sân khấu".to_owned(),
                synopsis: "Những cuộc trò chuyện sau buổi diễn.".to_owned(),
                language: "vi-VN".to_owned(),
                visibility: Visibility::Private,
                rights_refs: vec![],
            },
            at: 1000,
        })
        .unwrap()
    }

    fn unit(parent: &Work) -> ContentUnit {
        ContentUnit::new(
            parent,
            NewContentUnit {
                id: ContentUnitId::parse("00000000-0000-0000-0000-000000000002").unwrap(),
                kind: ContentUnitKind::Episode,
                details: UnitDetails {
                    title: "Tập một: Người kể chuyện".to_owned(),
                    description: "Một tập không yêu cầu Script IR.".to_owned(),
                    source: None,
                    script: None,
                    content_digest: None,
                },
                position: 7,
                at: 1000,
            },
        )
        .unwrap()
    }

    #[test]
    fn stored_snapshots_round_trip_through_admission() {
        let parent = work();
        let child = unit(&parent);
        assert_eq!(decode_work(&encode(&parent).unwrap()).unwrap(), parent);
        assert_eq!(
            decode_unit(&encode(&child).unwrap(), parent.kind()).unwrap(),
            child
        );
    }

    #[test]
    fn work_restore_rejects_invalid_metadata_and_revision() {
        let mut value = serde_json::to_value(work()).unwrap();
        value["revision"] = serde_json::json!(0);
        assert!(matches!(
            decode_work(&serde_json::to_vec(&value).unwrap()),
            Err(StoreError::CorruptRevision)
        ));
        value["revision"] = serde_json::json!(1);
        value["details"]["title"] = serde_json::json!("  ");
        assert!(matches!(
            decode_work(&serde_json::to_vec(&value).unwrap()),
            Err(StoreError::CorruptRevision)
        ));
    }

    #[test]
    fn unit_restore_rechecks_parent_category_and_link_validation() {
        let parent = work();
        let child = unit(&parent);
        assert!(matches!(
            decode_unit(&encode(&child).unwrap(), WorkKind::Novel),
            Err(StoreError::CorruptRevision)
        ));
        let mut value = serde_json::to_value(child).unwrap();
        value["details"]["script"] = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000003",
            "revision": 1,
            "export_digest": "not-a-digest"
        });
        assert!(matches!(
            decode_unit(&serde_json::to_vec(&value).unwrap(), parent.kind()),
            Err(StoreError::CorruptRevision)
        ));
    }

    #[test]
    fn snapshot_decoder_rejects_unknown_fields_duplicate_fields_and_oversize() {
        let mut value = serde_json::to_value(work()).unwrap();
        value["unrecognized"] = serde_json::json!(true);
        assert!(matches!(
            decode_work(&serde_json::to_vec(&value).unwrap()),
            Err(StoreError::CorruptRevision)
        ));
        let bytes = encode(&work()).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let duplicate = text.replacen("{", "{\"revision\":1,", 1);
        assert!(matches!(
            decode_work(duplicate.as_bytes()),
            Err(StoreError::CorruptRevision)
        ));
        assert!(matches!(
            decode_work(&vec![b' '; MAX_SNAPSHOT_BYTES + 1]),
            Err(StoreError::CorruptRevision)
        ));
    }

    #[test]
    fn snapshot_decoder_rejects_noncanonical_entity_ids() {
        let mut value = serde_json::to_value(work()).unwrap();
        value["id"] = serde_json::json!("{00000000-0000-0000-0000-000000000001}");
        assert!(matches!(
            decode_work(&serde_json::to_vec(&value).unwrap()),
            Err(StoreError::CorruptRevision)
        ));
    }
}
