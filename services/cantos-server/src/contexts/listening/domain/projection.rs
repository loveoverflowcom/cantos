//! Continue is derived from progress, with completion tested only after choosing the newest Work state.
use super::ProgressRecord;
use crate::contract::theatre::library::Completion;
use std::collections::BTreeMap;

pub(crate) fn continue_records(
    records: impl IntoIterator<Item = ProgressRecord>,
) -> Vec<ProgressRecord> {
    let mut latest: BTreeMap<String, ProgressRecord> = BTreeMap::new();
    for record in records {
        let raw = record.raw();
        let replace = latest.get(&raw.audio.work_id).is_none_or(|old| {
            raw.listener_order > old.raw().listener_order
                || (raw.listener_order == old.raw().listener_order
                    && raw.audio.publication_id < old.raw().audio.publication_id)
        });
        if replace {
            latest.insert(raw.audio.work_id.clone(), record);
        }
    }
    let mut records: Vec<_> = latest
        .into_values()
        .filter(|r| r.raw().completion == Completion::InProgress)
        .collect();
    records.sort_by(|a, b| {
        b.raw()
            .listener_order
            .cmp(&a.raw().listener_order)
            .then_with(|| a.raw().audio.work_id.cmp(&b.raw().audio.work_id))
    });
    records
}
