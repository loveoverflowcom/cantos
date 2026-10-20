//! Content identity is distinct from catalog metadata, Script IR and media identity.
//! No SQL, HTTP, clock, random IDs or provider decisions belong here.
use std::collections::BTreeSet;

use serde::Serialize;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::{
    ids::token_id_valid, metadata::validate_language, revision::Revision, CatalogError,
    ContentUnitId, OwnerId, ScriptRef, SourceRef, WorkId, MAX_RIGHTS_REFS,
};
use crate::platform::ids::canonical_uuid;

pub const MAX_TEXT_BYTES: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentError {
    InvalidId,
    InvalidText,
    InvalidLanguage,
    InvalidRights,
    InvalidProvenance,
    InvalidRevision,
    InvalidTimestamp,
    Encoding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ContentRevisionId(String);

impl ContentRevisionId {
    pub fn parse(value: &str) -> Result<Self, ContentError> {
        let id = canonical_uuid(value).ok_or(ContentError::InvalidId)?;
        if id != value {
            return Err(ContentError::InvalidId);
        }
        Ok(Self(id))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Derived text is NFC with LF line endings; preserved ingestion bytes are never normalized.
/// Neither raw serde input nor a default can forge admitted text.
/// ```compile_fail,E0277
/// use cantos_server::contexts::catalog::domain::content::ManuscriptText;
/// let text = serde_json::from_str::<ManuscriptText>("\"\"");
/// ```
/// ```compile_fail,E0277
/// use cantos_server::contexts::catalog::domain::content::ManuscriptText;
/// let text: ManuscriptText = Default::default();
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ManuscriptText(String);

impl ManuscriptText {
    pub fn new(value: &str) -> Result<Self, ContentError> {
        // Bound before normalization, including hostile combining-character input.
        if value.len() > MAX_TEXT_BYTES {
            return Err(ContentError::InvalidText);
        }
        let text: String = value
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .nfc()
            .collect();
        if text.trim().is_empty()
            || text.len() > MAX_TEXT_BYTES
            || text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        {
            return Err(ContentError::InvalidText);
        }
        Ok(Self(text))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// References keep their existing authorities. A script is an exact export, never a head.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentBody {
    Recording,
    Text {
        text: ManuscriptText,
        source: Option<SourceRef>,
    },
    Source {
        source: SourceRef,
    },
    Script {
        script: ScriptRef,
        source: Option<SourceRef>,
    },
}
impl ContentBody {
    pub fn source(&self) -> Option<&SourceRef> {
        match self {
            Self::Source { source } => Some(source),
            Self::Text { source, .. } | Self::Script { source, .. } => source.as_ref(),
            Self::Recording => None,
        }
    }
    pub fn script(&self) -> Option<&ScriptRef> {
        match self {
            Self::Script { script, .. } => Some(script),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ContentDetails {
    language: String,
    body: ContentBody,
    rights_refs: Vec<String>,
    provenance_note: String,
}
impl ContentDetails {
    pub fn new(
        language: String,
        body: ContentBody,
        mut rights_refs: Vec<String>,
        provenance_note: String,
    ) -> Result<Self, ContentError> {
        validate_language(&language).map_err(|_| ContentError::InvalidLanguage)?;
        let distinct: BTreeSet<_> = rights_refs.iter().collect();
        if rights_refs.len() > MAX_RIGHTS_REFS
            || distinct.len() != rights_refs.len()
            || rights_refs.iter().any(|id| !token_id_valid(id))
        {
            return Err(ContentError::InvalidRights);
        }
        if provenance_note.trim().is_empty()
            || provenance_note.len() > 2048
            || provenance_note.chars().any(char::is_control)
        {
            return Err(ContentError::InvalidProvenance);
        }
        rights_refs.sort();
        let provenance_note = provenance_note.nfc().collect::<String>();
        if provenance_note.len() > 2048 {
            return Err(ContentError::InvalidProvenance);
        }
        Ok(Self {
            language,
            body,
            rights_refs,
            provenance_note,
        })
    }
    pub fn language(&self) -> &str {
        &self.language
    }
    pub fn body(&self) -> &ContentBody {
        &self.body
    }
    pub fn rights_refs(&self) -> &[String] {
        &self.rights_refs
    }
    pub fn provenance_note(&self) -> &str {
        &self.provenance_note
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ContentDigest(String);
impl ContentDigest {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ContentRevisionDigest(String);
impl ContentRevisionDigest {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Constructor facts are supplied by the shell. The unit/parent relationship is rechecked
/// against PostgreSQL identities when loading; UUIDs are never hashes or positions.
pub struct ContentRevisionFacts {
    pub id: ContentRevisionId,
    pub work_id: WorkId,
    pub unit_id: ContentUnitId,
    pub revision: u64,
    pub details: ContentDetails,
    pub recorded_by: OwnerId,
    pub recorded_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ContentRevision {
    id: ContentRevisionId,
    work_id: WorkId,
    unit_id: ContentUnitId,
    revision: Revision,
    details: ContentDetails,
    content_digest: ContentDigest,
    recorded_by: OwnerId,
    recorded_at: i64,
}

/// Canonical JSON sorts object keys recursively, preserves array order and uses no floats.
fn fingerprint<T: Serialize>(scheme: &str, value: &T) -> Result<String, ContentError> {
    let value = serde_json::to_value(value).map_err(|_| ContentError::Encoding)?;
    let bytes = serde_json::to_vec(&value).map_err(|_| ContentError::Encoding)?;
    let mut hash = Sha256::new();
    hash.update(format!("cantos/{scheme}\n"));
    hash.update(bytes);
    Ok(format!("{scheme}:sha256:{:x}", hash.finalize()))
}

impl ContentRevision {
    pub fn new(facts: ContentRevisionFacts) -> Result<Self, ContentError> {
        let revision = Revision::new(facts.revision).map_err(|_| ContentError::InvalidRevision)?;
        if facts.recorded_at < 0 {
            return Err(ContentError::InvalidTimestamp);
        }
        #[derive(Serialize)]
        struct EffectiveContent<'a> {
            language: &'a str,
            body: &'a ContentBody,
        }
        let content_digest = ContentDigest(fingerprint(
            "cc-c1",
            &EffectiveContent {
                language: facts.details.language(),
                body: facts.details.body(),
            },
        )?);
        Ok(Self {
            id: facts.id,
            work_id: facts.work_id,
            unit_id: facts.unit_id,
            revision,
            details: facts.details,
            content_digest,
            recorded_by: facts.recorded_by,
            recorded_at: facts.recorded_at,
        })
    }
    pub fn id(&self) -> &ContentRevisionId {
        &self.id
    }
    pub fn work_id(&self) -> &WorkId {
        &self.work_id
    }
    pub fn unit_id(&self) -> &ContentUnitId {
        &self.unit_id
    }
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }
    pub fn details(&self) -> &ContentDetails {
        &self.details
    }
    pub fn content_digest(&self) -> &ContentDigest {
        &self.content_digest
    }
    pub fn revision_digest(&self) -> Result<ContentRevisionDigest, ContentError> {
        fingerprint("cc-r1", self).map(ContentRevisionDigest)
    }
    pub fn recorded_by(&self) -> &OwnerId {
        &self.recorded_by
    }
    pub fn recorded_at(&self) -> i64 {
        self.recorded_at
    }
}

pub fn next_content_revision(current: u64, expected: u64) -> Result<u64, CatalogError> {
    if current != expected {
        return Err(CatalogError::RevisionConflict {
            expected_revision: expected,
            current_revision: current,
        });
    }
    if current >= i64::MAX as u64 {
        return Err(CatalogError::RevisionLimit);
    }
    Ok(current + 1)
}

/// Work metadata preserves submitted ordering; rights declarations are a set of references.
pub fn rights_change_requires_owner(previous: &[String], next: &[String]) -> bool {
    previous.iter().collect::<BTreeSet<_>>() != next.iter().collect::<BTreeSet<_>>()
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
