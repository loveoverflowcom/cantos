use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::platform::ids::canonical_uuid;

use super::ids::token_id_valid;
use super::CatalogError;

pub const TITLE_MAX_BYTES: usize = 512;
pub const DESCRIPTION_MAX_BYTES: usize = 16_384;
pub const LANGUAGE_MAX_BYTES: usize = 63;
pub const MAX_RIGHTS_REFS: usize = 64;

/// The declared category is fixed for the lifetime of a work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    Novel,
    Story,
    Podcast,
}

impl WorkKind {
    pub fn parse(value: &str) -> Result<Self, CatalogError> {
        match value {
            "novel" => Ok(Self::Novel),
            "story" => Ok(Self::Story),
            "podcast" => Ok(Self::Podcast),
            _ => Err(CatalogError::UnknownWorkKind),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Novel => "novel",
            Self::Story => "story",
            Self::Podcast => "podcast",
        }
    }

    /// The current CMS uses chapters for prose works and episodes for podcasts. This does
    /// not introduce a cast, scene or Script IR requirement for either category.
    pub fn content_unit_kind(self) -> ContentUnitKind {
        match self {
            Self::Novel | Self::Story => ContentUnitKind::Chapter,
            Self::Podcast => ContentUnitKind::Episode,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentUnitKind {
    Chapter,
    Episode,
}

impl ContentUnitKind {
    pub fn parse(value: &str) -> Result<Self, CatalogError> {
        match value {
            "chapter" => Ok(Self::Chapter),
            "episode" => Ok(Self::Episode),
            _ => Err(CatalogError::UnknownContentUnitKind),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chapter => "chapter",
            Self::Episode => "episode",
        }
    }
}

/// Catalog visibility is descriptive metadata; it never authorizes public delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    Unlisted,
    Public,
}

impl Visibility {
    pub fn parse(value: &str) -> Result<Self, CatalogError> {
        match value {
            "private" => Ok(Self::Private),
            "unlisted" => Ok(Self::Unlisted),
            "public" => Ok(Self::Public),
            _ => Err(CatalogError::UnknownVisibility),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Unlisted => "unlisted",
            Self::Public => "public",
        }
    }
}

/// Catalog archival is separate from audio review or publication status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Draft,
    Archived,
}

impl Lifecycle {
    pub fn parse(value: &str) -> Result<Self, CatalogError> {
        match value {
            "draft" => Ok(Self::Draft),
            "archived" => Ok(Self::Archived),
            _ => Err(CatalogError::UnknownLifecycle),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Archived => "archived",
        }
    }
}

/// Unadmitted metadata. Work constructors validate every field before accepting it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkDetails {
    pub title: String,
    pub synopsis: String,
    pub language: String,
    pub visibility: Visibility,
    pub rights_refs: Vec<String>,
}

impl WorkDetails {
    pub(super) fn validate(&self) -> Result<(), CatalogError> {
        validate_text(&self.title, "title", TITLE_MAX_BYTES, true, false)?;
        validate_text(
            &self.synopsis,
            "synopsis",
            DESCRIPTION_MAX_BYTES,
            false,
            true,
        )?;
        validate_language(&self.language)?;
        if self.rights_refs.len() > MAX_RIGHTS_REFS {
            return Err(CatalogError::TooManyRightsRefs);
        }
        let mut refs = BTreeSet::new();
        for id in &self.rights_refs {
            if !token_id_valid(id) {
                return Err(CatalogError::InvalidRightsRef);
            }
            if !refs.insert(id) {
                return Err(CatalogError::DuplicateRightsRef);
            }
        }
        Ok(())
    }
}

/// A reference to immutable existing manuscript bytes. The shell verifies owner and digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceRef {
    id: String,
    sha256: String,
}

impl SourceRef {
    pub fn new(id: String, sha256: String) -> Result<Self, CatalogError> {
        if !token_id_valid(&id) {
            return Err(CatalogError::InvalidSourceId);
        }
        if !sha256_valid(&sha256) {
            return Err(CatalogError::InvalidSourceDigest);
        }
        Ok(Self { id, sha256 })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Exact accepted Script IR export binding. No latest-revision pointer is representable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ScriptRef {
    id: String,
    revision: u64,
    export_digest: String,
}

impl ScriptRef {
    pub fn new(id: String, revision: u64, export_digest: String) -> Result<Self, CatalogError> {
        if canonical_uuid(&id).is_none() {
            return Err(CatalogError::InvalidScriptId);
        }
        validate_revision(revision)?;
        if !export_digest
            .strip_prefix("sir-e1:sha256:")
            .is_some_and(sha256_valid)
        {
            return Err(CatalogError::InvalidScriptDigest);
        }
        Ok(Self {
            id,
            revision,
            export_digest,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn export_digest(&self) -> &str {
        &self.export_digest
    }
}

/// Unadmitted unit metadata. Validated links are optional for every work category.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UnitDetails {
    pub title: String,
    pub description: String,
    pub source: Option<SourceRef>,
    pub script: Option<ScriptRef>,
    /// Caller-declared content checksum. Admission of this string is not a byte-integrity
    /// check; the shell checks it against a pinned source or Script IR revision when linked.
    pub content_digest: Option<String>,
}

impl UnitDetails {
    pub(super) fn validate(&self) -> Result<(), CatalogError> {
        validate_text(&self.title, "title", TITLE_MAX_BYTES, true, false)?;
        validate_text(
            &self.description,
            "description",
            DESCRIPTION_MAX_BYTES,
            false,
            true,
        )?;
        if let Some(digest) = &self.content_digest {
            let value = digest.strip_prefix("sir-c1:sha256:").unwrap_or(digest);
            if !sha256_valid(value) {
                return Err(CatalogError::InvalidContentDigest);
            }
        }
        Ok(())
    }
}

fn validate_text(
    value: &str,
    field: &'static str,
    max_bytes: usize,
    required: bool,
    multiline: bool,
) -> Result<(), CatalogError> {
    if required && value.trim().is_empty() {
        return Err(CatalogError::BlankField { field });
    }
    if value.len() > max_bytes {
        return Err(CatalogError::FieldTooLong { field, max_bytes });
    }
    if value
        .chars()
        .any(|ch| ch.is_control() && !(multiline && matches!(ch, '\n' | '\r' | '\t')))
    {
        return Err(CatalogError::ControlCharacter { field });
    }
    Ok(())
}

fn validate_language(value: &str) -> Result<(), CatalogError> {
    // Accept a bounded common BCP-47 form without silently changing its spelling. This is
    // a language tag, not an assertion that a specific narrator or asset supports it.
    if value.len() > LANGUAGE_MAX_BYTES {
        return Err(CatalogError::InvalidLanguage);
    }
    let mut subtags = value.split('-');
    let Some(primary) = subtags.next() else {
        return Err(CatalogError::InvalidLanguage);
    };
    if !(2..=8).contains(&primary.len()) || !primary.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return Err(CatalogError::InvalidLanguage);
    }
    if subtags.any(|part| {
        !(1..=8).contains(&part.len()) || !part.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }) {
        return Err(CatalogError::InvalidLanguage);
    }
    Ok(())
}

fn sha256_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn validate_revision(revision: u64) -> Result<(), CatalogError> {
    if revision == 0 || revision > i64::MAX as u64 {
        return Err(CatalogError::InvalidRevision);
    }
    Ok(())
}

pub(super) fn validate_timestamps(created_at: i64, updated_at: i64) -> Result<(), CatalogError> {
    if created_at < 0 || updated_at < 0 {
        return Err(CatalogError::InvalidTimestamp);
    }
    if updated_at < created_at {
        return Err(CatalogError::TimestampRegression);
    }
    Ok(())
}

pub(super) fn next_revision(
    current: u64,
    expected: u64,
    updated_at: i64,
    at: i64,
) -> Result<u64, CatalogError> {
    if current != expected {
        return Err(CatalogError::RevisionConflict {
            current_revision: current,
            expected_revision: expected,
        });
    }
    validate_timestamps(updated_at, at)?;
    if current == i64::MAX as u64 {
        return Err(CatalogError::RevisionLimit);
    }
    Ok(current + 1)
}

pub(super) fn validate_unit_kind(
    work: WorkKind,
    unit: ContentUnitKind,
) -> Result<(), CatalogError> {
    if work.content_unit_kind() != unit {
        return Err(CatalogError::WrongContentUnitKind { work, unit });
    }
    Ok(())
}

pub(super) fn validate_position(position: u32) -> Result<(), CatalogError> {
    if position > i32::MAX as u32 {
        return Err(CatalogError::InvalidOrder);
    }
    Ok(())
}
