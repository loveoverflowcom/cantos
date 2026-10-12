//! Immutable external production instructions. Plain deterministic rules; no producer I/O.
use crate::contexts::catalog::domain::{
    content::{ContentBody, ContentRevision},
    OwnerId, WorkId, WorkKind,
};
use crate::contexts::ingestion::domain::receipt::{metadata_issues, receipt_digest};
use crate::platform::ids::canonical_uuid;
use crate::script_ir::read_script;
use cantos_contracts as dto;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use unicode_normalization::UnicodeNormalization;

pub const MAX_BRIEF_BYTES: usize = 256 * 1024;
pub const MAX_GRANT_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BriefError {
    InvalidId,
    InvalidVersion,
    InvalidGuidance,
    UnsupportedProfile,
    ContentMismatch,
    InvalidInstructions,
    InvalidDigest,
    ScopeMismatch,
    ExpiredGrant,
    RevokedGrant,
    Incompatible(dto::ProducerCapability),
    Encoding,
}

macro_rules! identity {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);
        impl $name {
            pub fn parse(value: &str) -> Result<Self, BriefError> {
                if canonical_uuid(value).as_deref() != Some(value) {
                    return Err(BriefError::InvalidId);
                }
                Ok(Self(value.into()))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
identity!(ProductionBriefId);
identity!(ProducerGrantId);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct BriefDigest(String);
impl BriefDigest {
    pub fn parse(value: &str) -> Result<Self, BriefError> {
        let suffix = value
            .strip_prefix("pb-r1:sha256:")
            .ok_or(BriefError::InvalidDigest)?;
        if suffix.len() != 64
            || !suffix
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(BriefError::InvalidDigest);
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn fingerprint<T: Serialize>(scheme: &str, value: &T) -> Result<String, BriefError> {
    // Value uses sorted map keys. Version1 forbids floats; arrays retain meaningful order.
    let value = serde_json::to_value(value).map_err(|_| BriefError::Encoding)?;
    let bytes = serde_json::to_vec(&value).map_err(|_| BriefError::Encoding)?;
    let mut hash = Sha256::new();
    hash.update(format!("cantos/{scheme}\n"));
    hash.update(bytes);
    Ok(format!("{scheme}:sha256:{:x}", hash.finalize()))
}
pub fn rights_digest(rights: &[String]) -> Result<String, BriefError> {
    let set: BTreeSet<_> = rights.iter().collect();
    fingerprint("pb-rights1", &set)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioProfile {
    OpusMono20,
    OpusMono24,
    OpusMono32,
    OpusMono40,
}
impl AudioProfile {
    pub fn parse(value: &str) -> Result<Self, BriefError> {
        match value {
            "opus-mono-20-v1" => Ok(Self::OpusMono20),
            "opus-mono-24-v1" => Ok(Self::OpusMono24),
            "opus-mono-32-v1" => Ok(Self::OpusMono32),
            "opus-mono-40-v1" => Ok(Self::OpusMono40),
            _ => Err(BriefError::UnsupportedProfile),
        }
    }
}

fn normalized(value: &str, max: usize, blank: bool) -> Result<String, BriefError> {
    if value.len() > max {
        return Err(BriefError::InvalidGuidance);
    }
    let value: String = value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .nfc()
        .collect();
    if value.len() > max
        || (!blank && value.trim().is_empty())
        || value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err(BriefError::InvalidGuidance);
    }
    Ok(value)
}

fn version(value: u64) -> Result<(), BriefError> {
    if value == 0 || value > i64::MAX as u64 {
        return Err(BriefError::InvalidVersion);
    }
    Ok(())
}

/// Raw transport cannot be a witness; this value has private fields and no Deserialize.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Guidance(dto::BriefGuidance);
impl Guidance {
    pub fn admit(mut raw: dto::BriefGuidance) -> Result<Self, BriefError> {
        AudioProfile::parse(&raw.delivery.profile_id)?;
        raw.editorial.notes = normalized(&raw.editorial.notes, 4096, true)?;
        for text in [
            &mut raw.delivery.permitted_use,
            &mut raw.delivery.attribution,
            &mut raw.delivery.naming,
            &mut raw.delivery.qa_instructions,
        ] {
            *text = normalized(text, 4096, false)?;
        }
        if let Some(voice) = &mut raw.voice {
            version(voice.revision)?;
            voice.notes = normalized(&voice.notes, 4096, true)?;
            if voice.pronunciation.len() > 128 || voice.casting.len() > 128 {
                return Err(BriefError::InvalidGuidance);
            }
            let mut terms = BTreeSet::new();
            for entry in &mut voice.pronunciation {
                entry.term = normalized(&entry.term, 256, false)?;
                entry.reading = normalized(&entry.reading, 256, false)?;
                if !terms.insert(entry.term.clone()) {
                    return Err(BriefError::InvalidGuidance);
                }
            }
            let mut speakers = BTreeSet::new();
            for cast in &mut voice.casting {
                cast.speaker = normalized(&cast.speaker, 128, false)?;
                cast.direction = normalized(&cast.direction, 1024, false)?;
                if !speakers.insert(cast.speaker.clone()) {
                    return Err(BriefError::InvalidGuidance);
                }
            }
        }
        if let Some(sound) = &mut raw.sound_design {
            version(sound.revision)?;
            sound.notes = normalized(&sound.notes, 4096, false)?;
        }
        let set: BTreeSet<_> = raw.required_capabilities.iter().copied().collect();
        if set.len() != raw.required_capabilities.len() {
            return Err(BriefError::InvalidGuidance);
        }
        raw.required_capabilities.sort();
        let base = match raw.editorial.mode {
            dto::ProductionMode::Narration => dto::ProducerCapability::Narration,
            dto::ProductionMode::Recording => dto::ProducerCapability::Recording,
            dto::ProductionMode::AudioDrama => dto::ProducerCapability::ScriptIr,
        };
        for needed in [
            Some(base),
            Some(dto::ProducerCapability::OggOpusMono),
            raw.voice
                .as_ref()
                .filter(|v| !v.pronunciation.is_empty())
                .map(|_| dto::ProducerCapability::Pronunciation),
            raw.voice
                .as_ref()
                .filter(|v| !v.casting.is_empty())
                .map(|_| dto::ProducerCapability::Casting),
            raw.sound_design
                .as_ref()
                .map(|_| dto::ProducerCapability::SoundDesign),
        ]
        .into_iter()
        .flatten()
        {
            if !set.contains(&needed) {
                return Err(BriefError::Incompatible(needed));
            }
        }
        Ok(Self(raw))
    }
    pub fn as_raw(&self) -> &dto::BriefGuidance {
        &self.0
    }
    pub fn check_capabilities(
        &self,
        available: &[dto::ProducerCapability],
    ) -> Result<(), BriefError> {
        if available.iter().copied().collect::<BTreeSet<_>>().len() != available.len() {
            return Err(BriefError::InvalidGuidance);
        }
        for needed in &self.0.required_capabilities {
            if !available.contains(needed) {
                return Err(BriefError::Incompatible(*needed));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionBrief {
    document: dto::ProductionBriefDocument,
    id: ProductionBriefId,
    digest: BriefDigest,
}
impl ProductionBrief {
    pub fn admit(
        mut raw: dto::ProductionBriefDocument,
        content: &ContentRevision,
    ) -> Result<Self, BriefError> {
        let id = ProductionBriefId::parse(&raw.id)?;
        version(raw.version)?;
        if raw.schema_version != "1" || raw.created_at < content.recorded_at() || raw.created_at < 0
        {
            return Err(BriefError::InvalidVersion);
        }
        WorkId::parse(&raw.work.id).map_err(|_| BriefError::InvalidId)?;
        OwnerId::parse(&raw.work.owner_id).map_err(|_| BriefError::InvalidId)?;
        OwnerId::parse(&raw.created_by).map_err(|_| BriefError::InvalidId)?;
        WorkKind::parse(&raw.work.kind).map_err(|_| BriefError::InvalidGuidance)?;
        normalized(&raw.work.title, 512, false)?;
        if raw.created_by != raw.work.owner_id
            || raw.work.id != content.work_id().as_str()
            || raw.content.work_id != content.work_id().as_str()
            || raw.content.id != content.id().as_str()
            || raw.content.unit_id != content.unit_id().as_str()
            || raw.content.revision != content.revision()
            || raw.content.recorded_by != content.recorded_by().as_str()
            || raw.content.recorded_at != content.recorded_at()
            || raw.content.content_digest != content.content_digest().as_str()
            || serde_json::to_value(&raw.content.details).map_err(|_| BriefError::Encoding)?
                != serde_json::to_value(content.details()).map_err(|_| BriefError::Encoding)?
            || raw.content.revision_digest
                != content
                    .revision_digest()
                    .map_err(|_| BriefError::Encoding)?
                    .as_str()
        {
            return Err(BriefError::ContentMismatch);
        }
        let unique: BTreeSet<_> = raw.work.rights_refs.iter().collect();
        if unique.len() != raw.work.rights_refs.len()
            || unique.len() > 64
            || raw
                .work
                .rights_refs
                .iter()
                .any(|id| OwnerId::parse(id).is_err())
            || raw.work.rights_digest != rights_digest(&raw.work.rights_refs)?
        {
            return Err(BriefError::InvalidDigest);
        }
        let guidance = Guidance::admit(raw.guidance.clone())?;
        raw.guidance = guidance.0;
        validate_instructions(content, &raw.instructions, &raw.guidance)?;
        if let dto::ProductionInstructions::Source { receipt, .. } = &raw.instructions {
            if receipt.imported_by != raw.work.owner_id {
                return Err(BriefError::ScopeMismatch);
            }
        }
        raw.digest.clear();
        let digest = BriefDigest::parse(&fingerprint("pb-r1", &raw)?)?;
        raw.digest = digest.as_str().into();
        if serde_json::to_vec(&raw)
            .map_err(|_| BriefError::Encoding)?
            .len()
            > MAX_BRIEF_BYTES
        {
            return Err(BriefError::InvalidInstructions);
        }
        Ok(Self {
            document: raw,
            id,
            digest,
        })
    }
    pub fn document(&self) -> &dto::ProductionBriefDocument {
        &self.document
    }
    pub fn id(&self) -> &ProductionBriefId {
        &self.id
    }
    pub fn digest(&self) -> &BriefDigest {
        &self.digest
    }
    pub fn freshness(
        &self,
        content: &ContentRevision,
        rights: &[String],
        latest_version: u64,
        archived: bool,
    ) -> Result<dto::BriefFreshness, BriefError> {
        if archived {
            return Ok(dto::BriefFreshness::Archived);
        }
        if self.document.content.id != content.id().as_str()
            || self.document.content.revision_digest
                != content
                    .revision_digest()
                    .map_err(|_| BriefError::Encoding)?
                    .as_str()
        {
            return Ok(dto::BriefFreshness::ContentChanged);
        }
        if self.document.work.rights_digest != rights_digest(rights)? {
            return Ok(dto::BriefFreshness::RightsChanged);
        }
        if self.document.version != latest_version {
            return Ok(dto::BriefFreshness::GuidanceChanged);
        }
        Ok(dto::BriefFreshness::Current)
    }
}

fn validate_instructions(
    content: &ContentRevision,
    instructions: &dto::ProductionInstructions,
    guidance: &dto::BriefGuidance,
) -> Result<(), BriefError> {
    match (content.details().body(), instructions) {
        (ContentBody::Recording, dto::ProductionInstructions::Recording {})
            if guidance.editorial.mode == dto::ProductionMode::Recording => {}
        (ContentBody::Text { text, .. }, dto::ProductionInstructions::Text { text: actual })
            if actual == text.as_str()
                && guidance.editorial.mode != dto::ProductionMode::AudioDrama => {}
        (
            ContentBody::Source { source },
            dto::ProductionInstructions::Source {
                receipt,
                receipt_digest: digest,
            },
        ) => {
            if receipt.id != source.id()
                || receipt.sha256 != source.sha256()
                || !metadata_issues(&receipt.metadata).is_empty()
                || receipt_digest(&receipt.metadata, &receipt.outcome, &receipt.sha256)
                    .map_err(|_| BriefError::Encoding)?
                    != *digest
            {
                return Err(BriefError::InvalidInstructions);
            }
            match &receipt.outcome {
                dto::ImportOutcome::Parsed { extraction }
                    if !extraction.extractor_version.is_empty()
                        && !extraction.blocks.is_empty() => {}
                _ => return Err(BriefError::InvalidInstructions),
            }
            // Source labels and cues are source material, never silently interpreted as speech.
            if guidance.editorial.mode == dto::ProductionMode::AudioDrama {
                return Err(BriefError::InvalidInstructions);
            }
        }
        (ContentBody::Script { script, .. }, dto::ProductionInstructions::Script { revision }) => {
            let parsed = read_script(revision.script_json.as_bytes())
                .map_err(|_| BriefError::InvalidInstructions)?;
            if revision.script_id != script.id()
                || revision.revision != script.revision()
                || revision.export_digest != script.export_digest()
                || crate::contexts::editorial::domain::revisions::export_digest(
                    &parsed.export_bytes(),
                ) != revision.export_digest
                || parsed.content_digest().to_string() != revision.content_digest
                || !guidance
                    .required_capabilities
                    .contains(&dto::ProducerCapability::ScriptIr)
            {
                return Err(BriefError::InvalidInstructions);
            }
        }
        _ => return Err(BriefError::InvalidInstructions),
    }
    Ok(())
}

pub fn next_brief_version(current: u64, expected: u64) -> Result<u64, BriefError> {
    if current != expected || current >= i64::MAX as u64 {
        return Err(BriefError::InvalidVersion);
    }
    Ok(current + 1)
}

/// Narrow read witness, distinct from Studio access and future upload permission.
pub struct ProducerScope {
    grant: ProducerGrantId,
    work: WorkId,
    unit: crate::contexts::catalog::domain::ContentUnitId,
    brief: ProductionBriefId,
    digest: BriefDigest,
    expires_at: i64,
}
impl ProducerScope {
    pub fn admit(raw: &dto::ProducerGrantResponse) -> Result<Self, BriefError> {
        let unit = crate::contexts::catalog::domain::ContentUnitId::parse(&raw.unit_id)
            .map_err(|_| BriefError::InvalidId)?;
        if raw.permission != "read_instructions" || raw.expires_at < 0 {
            return Err(BriefError::ScopeMismatch);
        }
        Ok(Self {
            grant: ProducerGrantId::parse(&raw.id)?,
            work: WorkId::parse(&raw.work_id).map_err(|_| BriefError::InvalidId)?,
            unit,
            brief: ProductionBriefId::parse(&raw.brief_id)?,
            digest: BriefDigest::parse(&raw.brief_digest)?,
            expires_at: raw.expires_at,
        })
    }
    pub fn check(
        &self,
        brief: &ProductionBrief,
        now: i64,
        revoked: bool,
    ) -> Result<(), BriefError> {
        if revoked {
            return Err(BriefError::RevokedGrant);
        }
        if now < 0 || now >= self.expires_at {
            return Err(BriefError::ExpiredGrant);
        }
        let doc = brief.document();
        if self.work.as_str() != doc.work.id
            || self.unit.as_str() != doc.content.unit_id
            || self.brief != *brief.id()
            || self.digest != *brief.digest()
        {
            return Err(BriefError::ScopeMismatch);
        }
        Ok(())
    }
    pub fn grant_id(&self) -> &ProducerGrantId {
        &self.grant
    }
}
pub fn validate_grant_expiry(now: i64, expires: i64) -> Result<(), BriefError> {
    if now < 0 || expires <= now || expires.checked_sub(now).is_none_or(|d| d > MAX_GRANT_MS) {
        return Err(BriefError::ExpiredGrant);
    }
    Ok(())
}
