//! Theatre wire projection of a resolved listener publication. The domain decides eligibility;
//! this module only maps those facts onto `crate::contract::theatre::discovery` DTOs. It never reads the
//! private envelope fields (owner, candidate, delivery object, binding, review evidence).
use super::domain::listening::ListenerPublication;
use crate::contexts::catalog::domain::{ContentUnitKind, WorkKind};
use crate::contract::theatre::discovery as dto;
use crate::contract::theatre::library::{AudioPresentation, WorkPresentation};

/// `first_published_at_ms` is the work's immutable discovery anchor, not this release's time.
pub(super) fn work_summary(
    publication: &ListenerPublication,
    first_published_at_ms: i64,
) -> dto::WorkSummary {
    let work = publication.discoverable();
    dto::WorkSummary {
        work_id: publication.work().id().as_str().into(),
        kind: work_kind(work.kind),
        title: work.title.into(),
        description: work.description.into(),
        language: work.language.into(),
        first_published_at_ms,
    }
}

/// `position` is the unit's immutable listing anchor, not its current Studio position.
pub(super) fn unit_summary(publication: &ListenerPublication, position: u32) -> dto::UnitSummary {
    let d = publication.release().document();
    let unit = publication.unit();
    dto::UnitSummary {
        unit_id: unit.id().as_str().into(),
        kind: unit_kind(unit.kind()),
        position,
        title: unit.details().title.clone(),
        description: unit.details().description.clone(),
        publication_id: d.id.clone(),
        published_at_ms: d.published_at,
        duration_ms: d.manifest.media.duration_ms,
    }
}

pub(super) fn presented_work_summary(
    publication: &ListenerPublication,
    first_published_at_ms: i64,
    unit_count: u64,
) -> dto::PresentedWorkSummary {
    let work = work_summary(publication, first_published_at_ms);
    dto::PresentedWorkSummary {
        work_id: work.work_id,
        kind: work.kind,
        title: work.title,
        description: work.description,
        language: work.language,
        first_published_at_ms: work.first_published_at_ms,
        unit_count,
    }
}

pub(super) fn presented_unit_summary(
    publication: &ListenerPublication,
    position: u32,
    ordinal: u64,
) -> dto::PresentedUnitSummary {
    let unit = unit_summary(publication, position);
    dto::PresentedUnitSummary {
        unit_id: unit.unit_id,
        kind: unit.kind,
        position: unit.position,
        title: unit.title,
        description: unit.description,
        publication_id: unit.publication_id,
        published_at_ms: unit.published_at_ms,
        duration_ms: unit.duration_ms,
        ordinal,
    }
}

pub(super) fn work_presentation(publication: &ListenerPublication) -> WorkPresentation {
    let work = publication.discoverable();
    WorkPresentation {
        work_id: publication.work().id().as_str().into(),
        publication_id: publication.release().document().id.clone(),
        title: work.title.into(),
        kind: work_kind(work.kind),
        language: work.language.into(),
    }
}

pub(super) fn audio_presentation(publication: &ListenerPublication) -> AudioPresentation {
    AudioPresentation {
        work: work_presentation(publication),
        unit_id: publication.unit().id().as_str().into(),
        unit_title: publication.unit().details().title.clone(),
    }
}

pub(super) fn publication_manifest(publication: &ListenerPublication) -> dto::PublicationManifest {
    let d = publication.release().document();
    let m = &d.manifest;
    let a = &m.media;
    dto::PublicationManifest {
        schema_version: "1".into(),
        publication_id: d.id.clone(),
        work_id: m.work_id.clone(),
        unit_id: m.unit_id.clone(),
        rendition_id: m.rendition_id.clone(),
        manifest_digest: m.digest.clone(),
        sha256: a.sha256.clone(),
        byte_length: a.byte_length,
        duration_ms: a.duration_ms,
        container: a.container.clone(),
        codec: a.codec.clone(),
        channels: a.channels,
        sample_rate_hz: a.sample_rate_hz,
        profile_id: a.profile_id.clone(),
        language: m.language.clone(),
        published_at_ms: d.published_at,
    }
}

pub(super) fn work_kind(kind: WorkKind) -> dto::WorkKind {
    match kind {
        WorkKind::Novel => dto::WorkKind::Novel,
        WorkKind::Story => dto::WorkKind::Story,
        WorkKind::Podcast => dto::WorkKind::Podcast,
    }
}

pub(super) fn unit_kind(kind: ContentUnitKind) -> dto::UnitKind {
    match kind {
        ContentUnitKind::Chapter => dto::UnitKind::Chapter,
        ContentUnitKind::Episode => dto::UnitKind::Episode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::{
        catalog::domain::{NewWork, Visibility, Work},
        media::domain::{
            listening::ListenerFacts,
            publication::{decide, tests::fixture, AudioRelease, ReleaseId},
        },
    };
    use serde_json::{json, Value};

    /// A public release resolved through the real domain decision, never built field by field.
    fn resolved(work_language: &str) -> (ListenerPublication, AudioRelease) {
        let mut f = fixture();
        let mut details = f.work.details().clone();
        details.visibility = Visibility::Public;
        details.language = work_language.into();
        f.work = Work::new(NewWork {
            id: f.work.id().clone(),
            kind: f.work.kind(),
            owner_id: f.work.owner_id().clone(),
            details,
            at: 1000,
        })
        .unwrap();
        let release = AudioRelease::publish(
            ReleaseId::parse("80000000-0000-4000-8000-000000000008").unwrap(),
            &f.actor,
            5000,
            None,
            decide(&f.facts()).unwrap(),
        )
        .unwrap();
        let publication = ListenerPublication::resolve(ListenerFacts {
            release: &release,
            pinned_work: &f.work,
            pinned_unit: &f.unit,
            current_work: &f.work,
            current_unit: &f.unit,
            rights: Some(&f.rights),
            approval: Some(&f.approval),
            active: true,
            owner_active: true,
            withdrawn: false,
            submission_revoked: false,
            now: 5000,
        })
        .unwrap()
        .unwrap();
        (publication, release)
    }

    /// The published contract, independent of this mapping: required fields and no extras.
    fn assert_schema(name: &str, value: &Value) {
        let root: Value = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/discovery.schema.json"
        ))
        .unwrap();
        let validator = jsonschema::validator_for(
            &json!({"$schema": root["$schema"], "$defs": root["$defs"], "$ref": format!("#/$defs/{name}")}),
        )
        .unwrap();
        assert!(validator.is_valid(value), "{name}: {value}");
    }

    #[test]
    fn manifest_projects_exact_release_identity_and_omits_private_envelope() {
        let (p, release) = resolved("vi-VN");
        let manifest = publication_manifest(&p);
        let d = release.document();
        assert_eq!(manifest.publication_id, d.id);
        assert_eq!(manifest.rendition_id, d.manifest.rendition_id);
        assert_eq!(manifest.manifest_digest, d.manifest.digest);
        assert_eq!(manifest.sha256, d.manifest.media.sha256);
        assert_eq!(manifest.published_at_ms, d.published_at);
        let fields = serde_json::to_value(&manifest).unwrap();
        assert_schema("PublicationManifest", &fields);
        for forbidden in [
            "owner_id",
            "candidate_id",
            "delivery_object_id",
            "binding",
            "review_evidence",
            "rights_refs",
            "url",
        ] {
            assert!(fields.get(forbidden).is_none(), "{forbidden}");
        }
    }

    #[test]
    fn summaries_use_published_audio_language_and_listing_anchors() {
        let (p, release) = resolved("en-US");
        let summary = work_summary(&p, 4321);
        assert_eq!(summary.language, "vi-VN");
        assert_eq!(summary.first_published_at_ms, 4321);
        assert_eq!(summary.work_id, p.work().id().as_str());
        assert_eq!(summary.title, p.work().details().title);
        assert_eq!(summary.description, p.work().details().synopsis);
        assert_schema("WorkSummary", &serde_json::to_value(&summary).unwrap());
        assert_eq!(publication_manifest(&p).language, "vi-VN");

        let unit = unit_summary(&p, 7);
        assert_eq!(unit.position, 7);
        assert_ne!(p.unit().position(), 7);
        assert_eq!(unit.unit_id, p.unit().id().as_str());
        assert_eq!(unit.publication_id, release.document().id);
        assert_eq!(
            unit.duration_ms,
            release.document().manifest.media.duration_ms
        );
        assert_schema("UnitSummary", &serde_json::to_value(&unit).unwrap());
    }

    #[test]
    fn presentation_uses_exact_released_metadata_and_separate_list_facts() {
        let (publication, release) = resolved("en-US");
        let work = presented_work_summary(&publication, 4321, 3);
        let unit = presented_unit_summary(&publication, 9, 2);
        assert_eq!(work.unit_count, 3);
        assert_eq!(work.language, "vi-VN");
        assert_eq!((unit.position, unit.ordinal), (9, 2));
        assert_schema(
            "PresentedWorkSummary",
            &serde_json::to_value(&work).unwrap(),
        );
        assert_schema(
            "PresentedUnitSummary",
            &serde_json::to_value(&unit).unwrap(),
        );
        let display = audio_presentation(&publication);
        assert_eq!(display.work.publication_id, release.document().id);
        assert_eq!(display.work.work_id, work.work_id);
        assert_eq!(display.work.title, work.title);
        assert_eq!(display.work.language, "vi-VN");
        assert_eq!(display.unit_id, unit.unit_id);
        assert_eq!(display.unit_title, unit.title);
        let fields = serde_json::to_value(display).unwrap();
        assert_eq!(fields.as_object().unwrap().len(), 3);
        assert_eq!(fields["work"].as_object().unwrap().len(), 5);
    }

    #[test]
    fn every_catalog_kind_has_its_wire_spelling() {
        for kind in [WorkKind::Novel, WorkKind::Story, WorkKind::Podcast] {
            assert_eq!(
                serde_json::to_value(work_kind(kind)).unwrap(),
                Value::from(kind.as_str())
            );
        }
        for kind in [ContentUnitKind::Chapter, ContentUnitKind::Episode] {
            assert_eq!(
                serde_json::to_value(unit_kind(kind)).unwrap(),
                Value::from(kind.as_str())
            );
        }
    }
}
