//! Order changes by stable ID.
//!
//! An entity moved when its parent changed, or when it is not part of the longest run of
//! siblings that kept their relative order. Inserting or deleting a sibling shifts indices but
//! moves nobody, so a moved line is never reported as a deletion plus an addition.

use std::collections::{BTreeMap, HashMap};

use super::rows::{Place, Row};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub from: Place,
    pub to: Place,
}

/// A surviving entity: its ID and where it stood before and after.
struct Survivor<'a> {
    id: &'a str,
    from: &'a Place,
    to: &'a Place,
}

impl Survivor<'_> {
    fn into_move(self) -> (String, Move) {
        let (from, to) = (self.from.clone(), self.to.clone());
        (self.id.to_owned(), Move { from, to })
    }
}

/// Moves among the rows of one entity kind, keyed by stable ID. Rows without a place never move.
///
/// When several sets of siblings are equally small, the one chosen is deterministic but
/// arbitrary: swapping two neighbours reports exactly one of them as moved.
pub fn moves(before: &[&Row], after: &[&Row]) -> BTreeMap<String, Move> {
    let earlier: HashMap<&str, &Place> = before
        .iter()
        .filter_map(|row| Some((row.id.as_str(), row.place.as_ref()?)))
        .collect();
    let mut moved = BTreeMap::new();
    let mut same_parent: BTreeMap<&str, Vec<Survivor>> = BTreeMap::new();
    for row in after {
        let (Some(to), Some(&from)) = (row.place.as_ref(), earlier.get(row.id.as_str())) else {
            continue;
        };
        let survivor = Survivor {
            id: &row.id,
            from,
            to,
        };
        if from.parent == to.parent {
            same_parent.entry(&to.parent).or_default().push(survivor);
        } else {
            moved.extend([survivor.into_move()]);
        }
    }
    for siblings in same_parent.into_values() {
        let earlier_indices: Vec<usize> = siblings.iter().map(|s| s.from.index).collect();
        let in_order = in_longest_increasing_run(&earlier_indices);
        let displaced = siblings.into_iter().zip(in_order);
        moved.extend(
            displaced
                .filter(|(_, kept)| !kept)
                .map(|(s, _)| s.into_move()),
        );
    }
    moved
}

/// Marks one longest strictly increasing subsequence of distinct values, in O(n log n).
fn in_longest_increasing_run(values: &[usize]) -> Vec<bool> {
    // run_ends[k] is the position in `values` of the smallest value that ends a run of length k + 1.
    let mut run_ends: Vec<usize> = Vec::new();
    let mut predecessor: Vec<Option<usize>> = vec![None; values.len()];
    for (position, &value) in values.iter().enumerate() {
        let length = run_ends.partition_point(|&end| values[end] < value);
        predecessor[position] = length.checked_sub(1).map(|shorter| run_ends[shorter]);
        match run_ends.get_mut(length) {
            Some(end) => *end = position,
            None => run_ends.push(position),
        }
    }
    let mut kept = vec![false; values.len()];
    let mut cursor = run_ends.last().copied();
    while let Some(position) = cursor {
        kept[position] = true;
        cursor = predecessor[position];
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::in_longest_increasing_run;

    fn kept_values(values: &[usize]) -> Vec<usize> {
        let kept = in_longest_increasing_run(values);
        values
            .iter()
            .zip(kept)
            .filter(|(_, kept)| *kept)
            .map(|(value, _)| *value)
            .collect()
    }

    #[test]
    fn already_ordered_siblings_are_all_kept() {
        assert_eq!(kept_values(&[0, 1, 2, 3]), [0, 1, 2, 3]);
        assert_eq!(kept_values(&[2, 5, 9]), [2, 5, 9]);
        assert_eq!(kept_values(&[]), Vec::<usize>::new());
    }

    #[test]
    fn one_displaced_sibling_is_the_only_one_dropped() {
        assert_eq!(kept_values(&[1, 2, 3, 0]), [1, 2, 3]);
        assert_eq!(kept_values(&[3, 0, 1, 2]), [0, 1, 2]);
        assert_eq!(kept_values(&[0, 3, 1, 2]), [0, 1, 2]);
    }

    #[test]
    fn the_kept_run_is_strictly_increasing() {
        assert_eq!(kept_values(&[1, 1, 1]), [1]);
        assert_eq!(kept_values(&[0, 1, 1, 2]), [0, 1, 2]);
    }

    #[test]
    fn a_reversed_run_keeps_a_single_sibling() {
        assert_eq!(kept_values(&[3, 2, 1, 0]).len(), 1);
    }
}
