//! Studio preview of one exact saved Work revision (ADR 0054 D03, D01b, #650).
//!
//! Authority is the Catalog read authority: the owner and current reader/editor members, checked
//! in the same read transaction as the data, never a bypass in a public query. A preview reads
//! only the Catalog head the creator named; it writes no publication, receipt, history, search or
//! progress row, and a missing or unauthorized Work is `404` before any field is read.
use super::super::{
    domain::{admit_preview, ContentUnit, Work, WorkId},
    turso,
};
use super::{
    authorize, current_actor, cursor_after, page_limit, work, CatalogCursor, CatalogStoreError,
};
use crate::contexts::editorial::Action;
use crate::contract::studio::{PreviewUnit, PreviewWork, StudioPreview};
use crate::platform::store::{authenticate, normalized_uuid, Store, StoreError};

/// Narrow projection of the Work: the text Theatre would show and nothing Studio-private.
pub(crate) fn preview_work(work: &Work) -> PreviewWork {
    PreviewWork {
        id: work.id().as_str().into(),
        kind: work.kind().as_str().into(),
        revision: work.revision(),
        title: work.details().title.clone(),
        synopsis: work.details().synopsis.clone(),
        language: work.details().language.clone(),
    }
}

pub(crate) fn preview_unit(unit: &ContentUnit, ordinal: u32) -> PreviewUnit {
    PreviewUnit {
        id: unit.id().as_str().into(),
        kind: unit.kind().as_str().into(),
        revision: unit.revision(),
        ordinal,
        title: unit.details().title.clone(),
        description: unit.details().description.clone(),
    }
}

impl Store {
    /// The cursor is bound to actor, Work, revision and the Catalog epoch, so any committed
    /// Catalog write, including a permission change, ends a walk with `stale_cursor`.
    pub async fn catalog_preview(
        &self,
        token: &str,
        id: &WorkId,
        work_revision: u64,
        limit: u64,
        cursor: Option<&CatalogCursor>,
    ) -> Result<StudioPreview, CatalogStoreError> {
        let limit = page_limit(limit)?;
        let client = self.connection().await?;
        let tx = client.read().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        let epoch = turso::lock_clock(&tx).await?;
        let actor = current_actor(&tx, token).await?;
        let work = work(&tx, id).await?;
        authorize(&tx, &actor, &work, Action::Read).await?;
        admit_preview(&work, work_revision)?;
        let scope = format!("preview:{actor}:{}:{work_revision}", id.as_str());
        let after = cursor_after(cursor, &scope, epoch)?
            .map(|after| {
                let (position, unit) = after
                    .split_once(':')
                    .ok_or(CatalogStoreError::InvalidCursor)?;
                let position = position
                    .parse::<u32>()
                    .map_err(|_| CatalogStoreError::InvalidCursor)?;
                if position > i32::MAX as u32 {
                    return Err(CatalogStoreError::InvalidCursor);
                }
                normalized_uuid(unit).map_err(|_| CatalogStoreError::InvalidCursor)?;
                Ok((position, unit))
            })
            .transpose()?;
        let mut units = turso::list_units(&tx, id.as_str(), after, false, limit + 1).await?;
        let more = units.len() > limit as usize;
        units.truncate(limit as usize);
        let mut rows = Vec::with_capacity(units.len());
        if let Some(first) = units.first() {
            let mut ordinal =
                turso::active_unit_ordinal(&tx, id.as_str(), first.position(), first.id().as_str())
                    .await?;
            for unit in &units {
                rows.push(preview_unit(unit, ordinal));
                ordinal += 1;
            }
        }
        let next_cursor = match (more, units.last()) {
            (true, Some(last)) => Some(
                CatalogCursor {
                    scope,
                    epoch,
                    after: format!("{}:{}", last.position(), last.id().as_str()),
                }
                .encode()?,
            ),
            _ => None,
        };
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(StudioPreview {
            work: preview_work(&work),
            units: rows,
            next_cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::catalog::domain::{
        ContentUnitId, ContentUnitKind, NewContentUnit, NewWork, OwnerId, ScriptRef, SourceRef,
        UnitDetails, Visibility, WorkDetails, WorkKind,
    };
    use serde_json::{json, Value};

    const WORK: &str = "10000000-0000-4000-8000-000000000001";
    const UNIT: &str = "20000000-0000-4000-8000-000000000001";
    const SCRIPT: &str = "30000000-0000-4000-8000-000000000001";
    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    /// Every Studio-private field carries a value no preview field may repeat.
    fn private_fixture() -> (Work, ContentUnit) {
        let work = Work::new(NewWork {
            id: WorkId::parse(WORK).unwrap(),
            kind: WorkKind::Novel,
            owner_id: OwnerId::parse("owner-private-sentinel").unwrap(),
            details: WorkDetails {
                title: "Ánh đèn cuối sân khấu".into(),
                synopsis: "Một lời hẹn dưới mái hiên.".into(),
                language: "vi-VN".into(),
                visibility: Visibility::Public,
                rights_refs: vec!["rights-private-sentinel".into()],
            },
            at: 1_700_000_000_123,
        })
        .unwrap();
        let unit = ContentUnit::new(
            &work,
            NewContentUnit {
                id: ContentUnitId::parse(UNIT).unwrap(),
                kind: ContentUnitKind::Chapter,
                details: UnitDetails {
                    title: "Chương một".into(),
                    description: "Người kể chuyện trở về.".into(),
                    source: Some(
                        SourceRef::new("source-private-sentinel".into(), SHA.into()).unwrap(),
                    ),
                    script: Some(
                        ScriptRef::new(SCRIPT.into(), 3, format!("sir-e1:sha256:{SHA}")).unwrap(),
                    ),
                    content_digest: Some(SHA.into()),
                },
                position: 7,
                at: 1_700_000_000_456,
            },
        )
        .unwrap();
        (work, unit)
    }

    #[test]
    fn preview_projection_is_an_exact_allowlist_that_drops_every_studio_private_field() {
        let (work, unit) = private_fixture();
        let work_json = serde_json::to_value(preview_work(&work)).unwrap();
        let unit_json = serde_json::to_value(preview_unit(&unit, 2)).unwrap();
        assert_eq!(
            work_json,
            json!({
                "id": WORK, "kind": "novel", "revision": 1,
                "title": "Ánh đèn cuối sân khấu",
                "synopsis": "Một lời hẹn dưới mái hiên.", "language": "vi-VN"
            })
        );
        assert_eq!(
            unit_json,
            json!({
                "id": UNIT, "kind": "chapter", "revision": 1, "ordinal": 2,
                "title": "Chương một", "description": "Người kể chuyện trở về."
            })
        );
        // The Catalog position (7) is a tombstone-sensitive locator, not the public rank.
        let text = Value::Array(vec![work_json, unit_json]).to_string();
        for private in [
            "private-sentinel",
            SHA,
            SCRIPT,
            "visibility",
            "1700000000123",
            "1700000000456",
            "lifecycle",
        ] {
            assert!(!text.contains(private), "{private}");
        }
    }
}
