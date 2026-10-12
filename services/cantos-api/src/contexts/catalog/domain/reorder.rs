use std::collections::{BTreeMap, BTreeSet};

use super::{CatalogError, ContentUnit, ContentUnitId, Lifecycle, Work};

/// A complete accepted permutation. Execute every assignment in one application transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReorderPlan {
    assignments: Vec<OrderAssignment>,
}

impl ReorderPlan {
    pub fn assignments(&self) -> &[OrderAssignment] {
        &self.assignments
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderAssignment {
    unit_id: ContentUnitId,
    position: u32,
}

impl OrderAssignment {
    pub fn unit_id(&self) -> &ContentUnitId {
        &self.unit_id
    }
    pub fn position(&self) -> u32 {
        self.position
    }
}

/// Reorder is an exact permutation of all draft units. Archived units retain their historical
/// positions and cannot be silently restored, removed or rebound by a reorder request.
pub fn plan_reorder(
    work: &Work,
    units: &[ContentUnit],
    ordered: &[ContentUnitId],
) -> Result<ReorderPlan, CatalogError> {
    work.require_draft()?;
    let mut current = BTreeMap::new();
    let mut positions = BTreeSet::new();
    let mut active_positions = Vec::new();
    for unit in units {
        if unit.work_id() != work.id() {
            return Err(CatalogError::WrongWork {
                unit: unit.id().clone(),
            });
        }
        if current.insert(unit.id(), unit.lifecycle()).is_some() {
            return Err(CatalogError::DuplicateContentUnit {
                unit: unit.id().clone(),
            });
        }
        if !positions.insert(unit.position()) {
            return Err(CatalogError::DuplicatePosition {
                position: unit.position(),
            });
        }
        if unit.lifecycle() == Lifecycle::Draft {
            active_positions.push(unit.position());
        }
    }
    active_positions.sort_unstable();
    let mut seen = BTreeSet::new();
    let mut assignments = Vec::with_capacity(ordered.len());
    for id in ordered {
        if !seen.insert(id) {
            return Err(CatalogError::DuplicateReorderUnit { unit: id.clone() });
        }
        match current.get(id) {
            None => return Err(CatalogError::UnknownReorderUnit { unit: id.clone() }),
            Some(Lifecycle::Archived) => {
                return Err(CatalogError::ArchivedReorderUnit { unit: id.clone() })
            }
            Some(Lifecycle::Draft) => {}
        }
    }
    for (id, lifecycle) in current {
        if lifecycle == Lifecycle::Draft && !seen.contains(id) {
            return Err(CatalogError::MissingReorderUnit { unit: id.clone() });
        }
    }
    for (id, position) in ordered.iter().zip(active_positions) {
        assignments.push(OrderAssignment {
            unit_id: id.clone(),
            position,
        });
    }
    Ok(ReorderPlan { assignments })
}
