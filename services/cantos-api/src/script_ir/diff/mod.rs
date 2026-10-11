//! Review-oriented comparison of two admitted Script IR values, keyed by stable ID.
//!
//! [`diff_scripts`] is pure: no file system, clock, ID minting, provider or revision store. It
//! tells a reviewer what an AI or a human editor changed between two versions of one episode.
//! It decides nothing about audio reuse, approval or publication; the speech fingerprints of the
//! production pipeline stay the authority for what must be re-rendered.
//!
//! Entities are matched by `(kind, stable ID)`. Characters and provenance sources have no
//! position, because validation keeps them sorted by ID. An ID that changes kind between the
//! versions is a different entity: the old one is removed and the new one added.

mod order;
mod rows;

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fmt,
};

use serde::Serialize;

use self::rows::{Aspect, Entity, Field, FieldValue, Place, Row};
use super::model::ScriptContent;
use super::wire::WRITE_VERSION;

/// Names the report layout and its rules; it changes whenever a field or its meaning changes.
pub const DIFF_REPORT_VERSION: &str = "cantos-script-diff-1";

#[derive(Debug, Serialize)]
pub struct ScriptDiff {
    report_version: &'static str,
    /// The Script IR version both documents were read as.
    schema_version: &'static str,
    scope: Scope,
    before: Side,
    after: Side,
    /// The content digest ignores provenance and rights references, so equal digests with
    /// `identical: false` mean the versions differ only in that metadata.
    content_digest_equal: bool,
    /// `true` only when there is no change of any kind, metadata included.
    identical: bool,
    summary: Summary,
    changes: Vec<Change>,
}

#[derive(Debug, Serialize)]
struct Scope {
    work_id: String,
    adaptation_id: String,
    episode_id: String,
}

#[derive(Debug, Serialize)]
struct Side {
    content_digest: String,
}

#[derive(Debug, Serialize)]
struct Summary {
    total: usize,
    added: usize,
    removed: usize,
    modified: usize,
    moved: usize,
    /// Changes involving each aspect. One change can involve several; every aspect is listed.
    by_aspect: AspectCounts,
}

#[derive(Debug, Default, Serialize)]
struct AspectCounts {
    text: usize,
    speaker: usize,
    delivery: usize,
    pronunciation: usize,
    order: usize,
    cue: usize,
    character: usize,
    title: usize,
    language: usize,
    provenance: usize,
    rights: usize,
}

impl AspectCounts {
    fn count(&mut self, aspect: Aspect) {
        let slot = match aspect {
            Aspect::Text => &mut self.text,
            Aspect::Speaker => &mut self.speaker,
            Aspect::Delivery => &mut self.delivery,
            Aspect::Pronunciation => &mut self.pronunciation,
            Aspect::Order => &mut self.order,
            Aspect::Cue => &mut self.cue,
            Aspect::Character => &mut self.character,
            Aspect::Title => &mut self.title,
            Aspect::Language => &mut self.language,
            Aspect::Provenance => &mut self.provenance,
            Aspect::Rights => &mut self.rights,
        };
        *slot += 1;
    }
}

/// One difference. A line that was both edited and moved yields a `modified` and a `moved`.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "change", rename_all = "snake_case")]
enum Change {
    /// Present only in `after`; `values` are its set fields.
    Added {
        entity: Entity,
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        at: Option<Place>,
        values: Vec<Field>,
    },
    /// Present only in `before`; `values` are its set fields.
    Removed {
        entity: Entity,
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        at: Option<Place>,
        values: Vec<Field>,
    },
    /// Present in both with at least one differing field.
    Modified {
        entity: Entity,
        id: String,
        fields: Vec<FieldChange>,
    },
    /// Present in both with the same fields but a different parent or relative order.
    /// Indices count every sibling of that version, added and removed ones included.
    Moved {
        entity: Entity,
        id: String,
        from: Place,
        to: Place,
    },
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct FieldChange {
    field: &'static str,
    aspect: Aspect,
    before: FieldValue,
    after: FieldValue,
}

impl Change {
    fn aspects(&self) -> BTreeSet<Aspect> {
        match self {
            Self::Added { values, .. } | Self::Removed { values, .. } => {
                values.iter().map(|field| field.aspect).collect()
            }
            Self::Modified { fields, .. } => fields.iter().map(|field| field.aspect).collect(),
            Self::Moved { .. } => BTreeSet::from([Aspect::Order]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeLevel {
    Work,
    Adaptation,
    Episode,
}

impl fmt::Display for ScopeLevel {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Work => "work",
            Self::Adaptation => "adaptation",
            Self::Episode => "episode",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeDifference {
    pub level: ScopeLevel,
    pub before: String,
    pub after: String,
}

/// The two scripts are not versions of one work, adaptation and episode.
#[derive(Debug, PartialEq, Eq)]
pub struct ScopeMismatch {
    differences: Vec<ScopeDifference>,
}

impl ScopeMismatch {
    pub fn differences(&self) -> &[ScopeDifference] {
        &self.differences
    }
}

impl fmt::Display for ScopeMismatch {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("not versions of one episode")?;
        for (position, difference) in self.differences.iter().enumerate() {
            out.write_str(if position == 0 { ": " } else { "; " })?;
            let ScopeDifference {
                level,
                before,
                after,
            } = difference;
            write!(out, "{level} id `{before}` vs `{after}`")?;
        }
        Ok(())
    }
}

/// Compare two versions of the same work, adaptation and episode.
///
/// Both values were admitted by the Script IR validator, so IDs are unique within each version
/// and every cue anchors to a dialogue of its own scene.
pub fn diff_scripts(
    before: &ScriptContent,
    after: &ScriptContent,
) -> Result<ScriptDiff, ScopeMismatch> {
    let scope = shared_scope(before, after)?;
    let (earlier, later) = (rows::rows(before), rows::rows(after));
    let changes: Vec<Change> = Entity::ALL
        .into_iter()
        .flat_map(|entity| entity_changes(entity, &earlier, &later))
        .collect();
    let before_digest = before.content_digest();
    let after_digest = after.content_digest();
    Ok(ScriptDiff {
        report_version: DIFF_REPORT_VERSION,
        schema_version: WRITE_VERSION,
        scope,
        content_digest_equal: before_digest == after_digest,
        before: Side {
            content_digest: before_digest.to_string(),
        },
        after: Side {
            content_digest: after_digest.to_string(),
        },
        identical: changes.is_empty(),
        summary: summarize(&changes),
        changes,
    })
}

fn scope_ids(script: &ScriptContent) -> [(ScopeLevel, &String); 3] {
    [
        (ScopeLevel::Work, &script.work.id.0),
        (ScopeLevel::Adaptation, &script.adaptation.id.0),
        (ScopeLevel::Episode, &script.episode.id.0),
    ]
}

fn shared_scope(before: &ScriptContent, after: &ScriptContent) -> Result<Scope, ScopeMismatch> {
    let differences: Vec<ScopeDifference> = scope_ids(before)
        .into_iter()
        .zip(scope_ids(after))
        .filter(|((_, old), (_, new))| old != new)
        .map(|((level, old), (_, new))| ScopeDifference {
            level,
            before: old.clone(),
            after: new.clone(),
        })
        .collect();
    if !differences.is_empty() {
        return Err(ScopeMismatch { differences });
    }
    Ok(Scope {
        work_id: before.work.id.0.clone(),
        adaptation_id: before.adaptation.id.0.clone(),
        episode_id: before.episode.id.0.clone(),
    })
}

fn of_kind(rows: &[Row], entity: Entity) -> Vec<&Row> {
    rows.iter().filter(|row| row.entity == entity).collect()
}

/// Changes of one entity kind: added, modified and moved in `after` order, then removed in
/// `before` order.
fn entity_changes(entity: Entity, earlier: &[Row], later: &[Row]) -> Vec<Change> {
    let (before, after) = (of_kind(earlier, entity), of_kind(later, entity));
    let before_by_id: HashMap<&str, &Row> = before.iter().map(|r| (r.id.as_str(), *r)).collect();
    let mut moved = order::moves(&before, &after);
    let mut changes = Vec::new();
    for row in &after {
        let Some(previous) = before_by_id.get(row.id.as_str()) else {
            changes.push(Change::Added {
                entity,
                id: row.id.clone(),
                at: row.place.clone(),
                values: set_fields(row),
            });
            continue;
        };
        let fields = field_changes(previous, row);
        if !fields.is_empty() {
            changes.push(Change::Modified {
                entity,
                id: row.id.clone(),
                fields,
            });
        }
        if let Some(order::Move { from, to }) = moved.remove(&row.id) {
            changes.push(Change::Moved {
                entity,
                id: row.id.clone(),
                from,
                to,
            });
        }
    }
    let kept: HashSet<&str> = after.iter().map(|r| r.id.as_str()).collect();
    for row in before.iter().filter(|r| !kept.contains(r.id.as_str())) {
        changes.push(Change::Removed {
            entity,
            id: row.id.clone(),
            at: row.place.clone(),
            values: set_fields(row),
        });
    }
    changes
}

fn set_fields(row: &Row) -> Vec<Field> {
    let set = row.fields.iter().filter(|field| !field.value.is_unset());
    set.cloned().collect()
}

/// Rows of one kind share their fields and order, so fields compare pairwise.
fn field_changes(before: &Row, after: &Row) -> Vec<FieldChange> {
    let pairs = before.fields.iter().zip(&after.fields);
    pairs
        .filter(|(old, new)| old.value != new.value)
        .map(|(old, new)| FieldChange {
            field: new.field,
            aspect: new.aspect,
            before: old.value.clone(),
            after: new.value.clone(),
        })
        .collect()
}

fn summarize(changes: &[Change]) -> Summary {
    let mut summary = Summary {
        total: changes.len(),
        added: 0,
        removed: 0,
        modified: 0,
        moved: 0,
        by_aspect: AspectCounts::default(),
    };
    for change in changes {
        match change {
            Change::Added { .. } => summary.added += 1,
            Change::Removed { .. } => summary.removed += 1,
            Change::Modified { .. } => summary.modified += 1,
            Change::Moved { .. } => summary.moved += 1,
        }
        for aspect in change.aspects() {
            summary.by_aspect.count(aspect);
        }
    }
    summary
}
