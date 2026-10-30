//! Pure metadata publication generations (ADR 0054 D01/D02/D05/D06,
//! `docs/decisions/0054-publication-generation-and-access.md`). Catalog keeps the only editing
//! head: a generation pins exact saved Work/Unit revisions in an explicit published order, and
//! one versioned pointer per Work carries the lifecycle for compare-and-set. An entry may also
//! pin one exact audio release (D06); that pin is a fact, never a playback right. The shell
//! supplies IDs, digests, rights facts, release pins and time; nothing here reads a current
//! head, a mutable Catalog position, an unselected Unit or a "latest" audio release.
use std::collections::BTreeSet;

use unicode_normalization::UnicodeNormalization;

use super::{ContentUnit, ContentUnitId, Lifecycle, Revision, Work, WorkId};

/// Catalog's per-Work Unit bound (tombstones included) also bounds one generation.
pub const MAX_PUBLISHED_UNITS: usize = 1000;

/// Every refusal names a class, never caller prose or a private field value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationError {
    /// Compare-and-set failed; `current` is the pointer version the caller must reread.
    StaleVersion {
        current: u64,
    },
    /// The action needs an ACTIVE pointer (per-Unit actions and withdrawal).
    NotActive,
    UnitNotPublished {
        unit: ContentUnitId,
    },
    /// The result would equal the active generation; history records only real changes.
    Unchanged,
    TooManyUnits,
    DuplicateUnit {
        unit: ContentUnitId,
    },
    InvalidPosition,
    WorkArchived,
    UnitArchived {
        unit: ContentUnitId,
    },
    WrongWork {
        unit: ContentUnitId,
    },
    /// The pinned Work revision cites no rights reference, or one no longer resolves.
    RightsUnavailable,
    /// The selected Work revision was never saved (shell fact).
    UnknownRevision,
    /// The selected Unit or its revision was never saved (shell fact).
    UnknownUnit {
        unit: ContentUnitId,
    },
    InvalidDigest,
    InvalidVersion,
    VersionLimit,
    InvalidTimestamp,
    /// The release pin is not a canonical release ID and release document digest.
    InvalidAudioRef,
    /// The named release is not the Unit's active audio release (no substitute, D06).
    ReleaseNotActive,
    /// The release differs from the entry beyond the display-only fields (D06a).
    AudioIncompatible,
    /// A display-only difference needs the owner's explicit confirmation (D06b).
    AudioConfirmationRequired,
    /// A stored generation or pointer contradicts the invariants below.
    Corrupt,
}

/// The pointer version compared by every command; zero is only "never published".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PublicationVersion(u64);

impl PublicationVersion {
    pub const fn new(value: u64) -> Result<Self, PublicationError> {
        if value == 0 || value > i64::MAX as u64 {
            return Err(PublicationError::InvalidVersion);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    const fn after(current: u64) -> Result<Self, PublicationError> {
        if current >= i64::MAX as u64 {
            return Err(PublicationError::VersionLimit);
        }
        Ok(Self(current + 1))
    }
}

/// One Work's generation sequence number, starting at 1 and never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GenerationNumber(u64);

impl GenerationNumber {
    pub const fn new(value: u64) -> Result<Self, PublicationError> {
        if value == 0 || value > i64::MAX as u64 {
            return Err(PublicationError::InvalidVersion);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    const fn next(self) -> Result<Self, PublicationError> {
        if self.0 == i64::MAX as u64 {
            return Err(PublicationError::VersionLimit);
        }
        Ok(Self(self.0 + 1))
    }
}

/// An exact saved metadata revision and the SHA-256 of its stored snapshot bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    revision: Revision,
    digest: String,
}

impl Pin {
    pub fn new(revision: Revision, digest: &str) -> Result<Self, PublicationError> {
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(PublicationError::InvalidDigest);
        }
        Ok(Self {
            revision,
            digest: digest.to_owned(),
        })
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// One settled audio release as Media identifies it: the release ID and the digest of the
/// immutable release document. It names exact bytes, approval and rights evidence; it never
/// resolves to a newer release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioRef {
    release: String,
    digest: String,
}

impl AudioRef {
    pub fn new(release: &str, digest: &str) -> Result<Self, PublicationError> {
        let hex = digest
            .strip_prefix("rl-r1:sha256:")
            .ok_or(PublicationError::InvalidAudioRef)?;
        if crate::platform::ids::canonical_uuid(release).as_deref() != Some(release)
            || hex.len() != 64
            || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(PublicationError::InvalidAudioRef);
        }
        Ok(Self {
            release: release.to_owned(),
            digest: digest.to_owned(),
        })
    }

    pub fn release(&self) -> &str {
        &self.release
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// The exact metadata revisions a release was approved against, as Media's manifest pins them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleasePin {
    pub release: AudioRef,
    pub work_revision: Revision,
    pub unit_revision: Revision,
}

/// D06 outcome of comparing a release's pinned metadata with a generation entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioCompatibility {
    /// The release pins exactly the entry's Unit revision (D06, first rule).
    Exact,
    /// Only display-only fields differ: Work title/synopsis, Unit title/description (D06a).
    DisplayOnly,
    /// Content, language, kind or rights differ; the entry stays metadata-only.
    Incompatible,
}

/// The D06 compatibility matrix. `released_*` are the snapshots at the revisions the release
/// manifest pins; `work`/`unit` are the generation's pins. Work `visibility` is editorial
/// metadata and never a listener gate (D05b), so it takes no part. Rights references are
/// compared as sets, languages as case-insensitive BCP-47 tags.
pub fn audio_compatibility(
    released_work: &Work,
    released_unit: &ContentUnit,
    work: &Work,
    unit: &ContentUnit,
) -> AudioCompatibility {
    fn rights(work: &Work) -> BTreeSet<&String> {
        work.details().rights_refs.iter().collect()
    }
    let (released, entry) = (released_unit.details(), unit.details());
    let same = released_work.id() == work.id()
        && released_work.owner_id() == work.owner_id()
        && released_work.kind() == work.kind()
        && released_unit.id() == unit.id()
        && released_unit.work_id() == work.id()
        && unit.work_id() == work.id()
        && released_unit.kind() == unit.kind()
        && released_work
            .details()
            .language
            .eq_ignore_ascii_case(&work.details().language)
        && rights(released_work) == rights(work)
        && released.source == entry.source
        && released.script == entry.script
        && released.content_digest == entry.content_digest;
    if !same {
        AudioCompatibility::Incompatible
    } else if released_unit.revision() == unit.revision() {
        AudioCompatibility::Exact
    } else {
        AudioCompatibility::DisplayOnly
    }
}

/// How an association was admitted; a display-only one always carries the owner's receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioBasis {
    Exact,
    DisplayOnly,
}

impl AudioBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::DisplayOnly => "display_only",
        }
    }

    pub fn parse(stored: &str) -> Result<Self, PublicationError> {
        match stored {
            "exact" => Ok(Self::Exact),
            "display_only" => Ok(Self::DisplayOnly),
            _ => Err(PublicationError::Corrupt),
        }
    }
}

/// An entry's audio association. Only `admit` creates a new one, from a compatibility result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioAssociation {
    release: AudioRef,
    basis: AudioBasis,
}

impl AudioAssociation {
    /// Exact pins associate on their own; a display-only drift needs `owner_confirmed`;
    /// anything else is refused, so the entry stays metadata-only.
    pub fn admit(
        release: AudioRef,
        compatibility: AudioCompatibility,
        owner_confirmed: bool,
    ) -> Result<Self, PublicationError> {
        let basis = match compatibility {
            AudioCompatibility::Exact => AudioBasis::Exact,
            AudioCompatibility::DisplayOnly if owner_confirmed => AudioBasis::DisplayOnly,
            AudioCompatibility::DisplayOnly => {
                return Err(PublicationError::AudioConfirmationRequired)
            }
            AudioCompatibility::Incompatible => return Err(PublicationError::AudioIncompatible),
        };
        Ok(Self { release, basis })
    }

    pub fn restore(release: AudioRef, basis: AudioBasis) -> Self {
        Self { release, basis }
    }

    pub fn release(&self) -> &AudioRef {
        &self.release
    }

    pub fn basis(&self) -> AudioBasis {
        self.basis
    }
}

/// One published Unit. Its published position is its index in the generation, never the
/// Catalog `position` stored in the Unit snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedEntry {
    unit: ContentUnitId,
    pin: Pin,
    audio: Option<AudioAssociation>,
}

impl PublishedEntry {
    pub fn unit(&self) -> &ContentUnitId {
        &self.unit
    }

    pub fn pin(&self) -> &Pin {
        &self.pin
    }

    /// `None` is metadata-only (`playback: null`).
    pub fn audio(&self) -> Option<&AudioAssociation> {
        self.audio.as_ref()
    }

    /// The same entry with an exact association offered by the shell for its pin.
    pub fn with_audio(self, audio: Option<AudioAssociation>) -> Self {
        Self { audio, ..self }
    }
}

/// Admit the Work revision a generation will pin. The revision is the exact saved snapshot the
/// owner chose; neither it nor the current Work may be archived. Rights follow the audio
/// release precedent: at least one reference, and every one resolves (`rights_resolved`, a
/// shell fact read in the settling transaction).
pub fn admit_work(
    pinned: &Work,
    current: &Work,
    digest: &str,
    rights_resolved: bool,
) -> Result<Pin, PublicationError> {
    if pinned.id() != current.id() || pinned.owner_id() != current.owner_id() {
        return Err(PublicationError::Corrupt);
    }
    if pinned.lifecycle() == Lifecycle::Archived || current.lifecycle() == Lifecycle::Archived {
        return Err(PublicationError::WorkArchived);
    }
    if pinned.details().rights_refs.is_empty() || !rights_resolved {
        return Err(PublicationError::RightsUnavailable);
    }
    let revision = Revision::new(pinned.revision()).map_err(|_| PublicationError::Corrupt)?;
    Pin::new(revision, digest)
}

/// Admit one selected Unit revision of `work`. A Unit of another Work, an archived snapshot or
/// a currently archived Unit is refused, so the generation can never mix Work ownership.
pub fn admit_unit(
    work: &WorkId,
    pinned: &ContentUnit,
    current: &ContentUnit,
    digest: &str,
) -> Result<PublishedEntry, PublicationError> {
    if pinned.id() != current.id() {
        return Err(PublicationError::Corrupt);
    }
    if pinned.work_id() != work || current.work_id() != work {
        return Err(PublicationError::WrongWork {
            unit: pinned.id().clone(),
        });
    }
    if pinned.lifecycle() == Lifecycle::Archived || current.lifecycle() == Lifecycle::Archived {
        return Err(PublicationError::UnitArchived {
            unit: pinned.id().clone(),
        });
    }
    let revision = Revision::new(pinned.revision()).map_err(|_| PublicationError::Corrupt)?;
    Ok(PublishedEntry {
        unit: pinned.id().clone(),
        pin: Pin::new(revision, digest)?,
        audio: None,
    })
}

/// An immutable full-copy generation: the Work pin and the ordered, duplicate-free entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generation {
    number: GenerationNumber,
    work: Pin,
    entries: Vec<PublishedEntry>,
}

fn check_entries(entries: &[PublishedEntry]) -> Result<(), PublicationError> {
    if entries.len() > MAX_PUBLISHED_UNITS {
        return Err(PublicationError::TooManyUnits);
    }
    let mut seen = BTreeSet::new();
    for entry in entries {
        if !seen.insert(entry.unit.as_str()) {
            return Err(PublicationError::DuplicateUnit {
                unit: entry.unit.clone(),
            });
        }
    }
    Ok(())
}

impl Generation {
    /// Rebuild a stored generation; a stored contradiction is corruption, not a caller error.
    pub fn restore(
        number: GenerationNumber,
        work: Pin,
        entries: Vec<(ContentUnitId, Pin, Option<AudioAssociation>)>,
    ) -> Result<Self, PublicationError> {
        let entries: Vec<_> = entries
            .into_iter()
            .map(|(unit, pin, audio)| PublishedEntry { unit, pin, audio })
            .collect();
        check_entries(&entries).map_err(|_| PublicationError::Corrupt)?;
        Ok(Self {
            number,
            work,
            entries,
        })
    }

    pub fn number(&self) -> GenerationNumber {
        self.number
    }

    /// Generations of one Work are consecutive, so the parent is always the previous number.
    pub fn parent(&self) -> Option<GenerationNumber> {
        GenerationNumber::new(self.number.0 - 1).ok()
    }

    pub fn work(&self) -> &Pin {
        &self.work
    }

    /// Entries in published order; the one-based published position is the index plus one.
    pub fn entries(&self) -> &[PublishedEntry] {
        &self.entries
    }

    fn contains(&self, unit: &ContentUnitId) -> bool {
        self.entries.iter().any(|entry| &entry.unit == unit)
    }

    /// The entry of `unit`, if this generation publishes it.
    pub fn entry(&self, unit: &ContentUnitId) -> Option<&PublishedEntry> {
        self.entries.iter().find(|entry| &entry.unit == unit)
    }
}

/// The pointer's live lifecycle once a Work has been published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Live {
    Active,
    Withdrawn,
}

impl Live {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Withdrawn => "withdrawn",
        }
    }

    pub fn parse(stored: &str) -> Result<Self, PublicationError> {
        match stored {
            "active" => Ok(Self::Active),
            "withdrawn" => Ok(Self::Withdrawn),
            _ => Err(PublicationError::Corrupt),
        }
    }
}

/// The Studio-visible lifecycle. A failed or unread state is an error, never `NeverPublished`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationLifecycle {
    NeverPublished,
    Active,
    Withdrawn,
}

impl PublicationLifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeverPublished => "never_published",
            Self::Active => "active",
            Self::Withdrawn => "withdrawn",
        }
    }
}

/// A published Work's pointer: version, lifecycle, the generation it names and the immutable
/// recency anchor set by the first ACTIVE generation (D02c).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
    version: PublicationVersion,
    lifecycle: Live,
    generation: Generation,
    first_visible_at: i64,
}

impl Published {
    pub fn restore(
        version: PublicationVersion,
        lifecycle: Live,
        generation: Generation,
        first_visible_at: i64,
    ) -> Result<Self, PublicationError> {
        if first_visible_at < 0 || generation.number.0 > version.0 {
            return Err(PublicationError::Corrupt);
        }
        Ok(Self {
            version,
            lifecycle,
            generation,
            first_visible_at,
        })
    }

    pub fn version(&self) -> PublicationVersion {
        self.version
    }

    pub fn lifecycle(&self) -> Live {
        self.lifecycle
    }

    pub fn generation(&self) -> &Generation {
        &self.generation
    }

    pub fn first_visible_at(&self) -> i64 {
        self.first_visible_at
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationState {
    NeverPublished,
    Published(Published),
}

impl PublicationState {
    /// The value a caller's `expected_version` is compared with.
    pub fn version(&self) -> u64 {
        match self {
            Self::NeverPublished => 0,
            Self::Published(published) => published.version.0,
        }
    }

    pub fn lifecycle(&self) -> PublicationLifecycle {
        match self {
            Self::NeverPublished => PublicationLifecycle::NeverPublished,
            Self::Published(Published {
                lifecycle: Live::Active,
                ..
            }) => PublicationLifecycle::Active,
            Self::Published(Published {
                lifecycle: Live::Withdrawn,
                ..
            }) => PublicationLifecycle::Withdrawn,
        }
    }

    /// The ACTIVE pointer, or `NotActive` (per-Unit actions, withdrawal, association).
    pub fn active(&self) -> Result<&Published, PublicationError> {
        match self {
            Self::Published(published) if published.lifecycle == Live::Active => Ok(published),
            _ => Err(PublicationError::NotActive),
        }
    }
}

/// An owner command after admission: every pin was resolved by the shell for this Work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationChange {
    /// The complete explicit selection in published order (first publish, republish, reorder).
    Publish {
        work: Pin,
        entries: Vec<PublishedEntry>,
    },
    /// Add or replace one Unit at a one-based published position; everything else is copied.
    PublishUnit {
        entry: PublishedEntry,
        position: u32,
    },
    WithdrawUnit {
        unit: ContentUnitId,
    },
    /// Withdraw the Work: the pointer turns WITHDRAWN; generations and history stay.
    Withdraw,
    /// Associate one published entry with an admitted release (D06b owner command, or D06c
    /// activation); every other entry is copied.
    AssociateAudio {
        unit: ContentUnitId,
        audio: AudioAssociation,
    },
}

/// An entry keeps the association of the ACTIVE generation's entry with the same Unit pin
/// under the same Work pin, unless the shell offers a new exact one. A changed Unit or Work pin
/// never inherits an association (the Work's language or rights may have changed), and a
/// publication from NEVER_PUBLISHED or WITHDRAWN revives no earlier owner decision.
fn carry_audio(active: Option<&Published>, work: &Pin, entry: PublishedEntry) -> PublishedEntry {
    if entry.audio.is_some() {
        return entry;
    }
    let prior = active
        .filter(|published| &published.generation.work == work)
        .and_then(|published| published.generation.entry(&entry.unit))
        .filter(|prior| prior.pin == entry.pin)
        .and_then(|prior| prior.audio.clone());
    entry.with_audio(prior)
}

/// The settled pointer after one command. `created` says whether `generation` is new and must
/// be written; a withdrawal keeps naming the last generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settled {
    pub published: Published,
    pub created: bool,
}

/// The one compare-and-set decision for every metadata publication command (D02a full copy).
pub fn decide(
    state: &PublicationState,
    expected_version: u64,
    change: PublicationChange,
    now: i64,
) -> Result<Settled, PublicationError> {
    if now < 0 {
        return Err(PublicationError::InvalidTimestamp);
    }
    if expected_version != state.version() {
        return Err(PublicationError::StaleVersion {
            current: state.version(),
        });
    }
    let version = PublicationVersion::after(state.version())?;
    let (lifecycle, work, entries) = match change {
        PublicationChange::Publish { work, entries } => {
            check_entries(&entries)?;
            let entries: Vec<_> = entries
                .into_iter()
                .map(|entry| carry_audio(state.active().ok(), &work, entry))
                .collect();
            if let Ok(active) = state.active() {
                let same = active.generation.work == work && active.generation.entries == entries;
                if same {
                    return Err(PublicationError::Unchanged);
                }
            }
            (Live::Active, work, entries)
        }
        PublicationChange::PublishUnit { entry, position } => {
            let active = state.active()?;
            let entry = carry_audio(Some(active), &active.generation.work, entry);
            let mut entries: Vec<_> = active
                .generation
                .entries
                .iter()
                .filter(|current| current.unit != entry.unit)
                .cloned()
                .collect();
            let index = usize::try_from(position)
                .ok()
                .and_then(|position| position.checked_sub(1))
                .filter(|index| *index <= entries.len())
                .ok_or(PublicationError::InvalidPosition)?;
            entries.insert(index, entry);
            check_entries(&entries)?;
            if entries == active.generation.entries {
                return Err(PublicationError::Unchanged);
            }
            (Live::Active, active.generation.work.clone(), entries)
        }
        PublicationChange::WithdrawUnit { unit } => {
            let active = state.active()?;
            if !active.generation.contains(&unit) {
                return Err(PublicationError::UnitNotPublished { unit });
            }
            let entries = active
                .generation
                .entries
                .iter()
                .filter(|current| current.unit != unit)
                .cloned()
                .collect();
            (Live::Active, active.generation.work.clone(), entries)
        }
        PublicationChange::AssociateAudio { unit, audio } => {
            let active = state.active()?;
            if !active.generation.contains(&unit) {
                return Err(PublicationError::UnitNotPublished { unit });
            }
            let entries: Vec<_> = active
                .generation
                .entries
                .iter()
                .map(|current| {
                    let entry = current.clone();
                    if current.unit == unit {
                        entry.with_audio(Some(audio.clone()))
                    } else {
                        entry
                    }
                })
                .collect();
            if entries == active.generation.entries {
                return Err(PublicationError::Unchanged);
            }
            (Live::Active, active.generation.work.clone(), entries)
        }
        PublicationChange::Withdraw => {
            let active = state.active()?;
            return Ok(Settled {
                published: Published {
                    version,
                    lifecycle: Live::Withdrawn,
                    generation: active.generation.clone(),
                    first_visible_at: active.first_visible_at,
                },
                created: false,
            });
        }
    };
    let (number, first_visible_at) = match state {
        PublicationState::NeverPublished => (GenerationNumber(1), now),
        PublicationState::Published(published) => (
            published.generation.number.next()?,
            published.first_visible_at,
        ),
    };
    Ok(Settled {
        published: Published {
            version,
            lifecycle,
            generation: Generation {
                number,
                work,
                entries,
            },
            first_visible_at,
        },
        created: true,
    })
}

/// CanViewWork for the only v1 audience, PUBLIC (D05a; RESTRICTED arrives with S06): an ACTIVE
/// pointer, an active owner and a current Work that is not archived. Catalog `visibility` is
/// editorial metadata here and never a gate (D05b), so saving a private r4 leaves r3 shown.
pub fn work_visible(lifecycle: Live, owner_active: bool, current_work_archived: bool) -> bool {
    lifecycle == Live::Active && owner_active && !current_work_archived
}

/// One visible entry: its consecutive public rank and its published position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibleEntry<'a> {
    pub ordinal: u32,
    pub position: u32,
    pub entry: &'a PublishedEntry,
}

/// CanViewUnit within a visible Work: generation membership and a current Unit that is not
/// archived. A hidden Unit leaves no row, count or ordinal, so ordinals stay consecutive while
/// published positions (the keyset) stay those of the generation.
pub fn visible_entries<'a>(
    generation: &'a Generation,
    archived: impl Fn(&ContentUnitId) -> bool,
) -> Vec<VisibleEntry<'a>> {
    generation
        .entries
        .iter()
        .zip(1_u32..)
        .filter(|(entry, _)| !archived(&entry.unit))
        .zip(1_u32..)
        .map(|((entry, position), ordinal)| VisibleEntry {
            ordinal,
            position,
            entry,
        })
        .collect()
}

/// The text a public search matches, from the pinned Work revision only. Same NFC/lowercase
/// rule as Discovery's `normalize_search`, applied to the query and to stored text.
pub fn public_search_text(value: &str) -> String {
    value
        .nfc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .nfc()
        .collect()
}

#[cfg(test)]
#[path = "metadata_publication_tests.rs"]
mod tests;
