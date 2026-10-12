//! Bounded session queue. The application supplies only selectable catalog sources.
use crate::core::{ids::EpisodeId, playback::Source};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueueState {
    pub items: Vec<EpisodeId>,
}
impl QueueState {
    pub fn enqueue(&self, source: Source, current: Option<EpisodeId>) -> Self {
        let id = source.episode();
        let mut next = self.clone();
        if current != Some(id) && !next.items.contains(&id) {
            next.items.push(id);
        }
        next
    }
    pub fn remove(&self, id: EpisodeId) -> Self {
        Self {
            items: self
                .items
                .iter()
                .copied()
                .filter(|item| *item != id)
                .collect(),
        }
    }
    pub fn pop(&self) -> (Self, Option<EpisodeId>) {
        match self.items.split_first() {
            Some((first, rest)) => (
                Self {
                    items: rest.to_vec(),
                },
                Some(*first),
            ),
            None => (self.clone(), None),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ids::WorkId;
    #[test]
    fn duplicate_current_and_empty_queue_actions_are_safe() {
        let source = Source::new(
            EpisodeId {
                work: WorkId::Mountain,
                index: 0,
            },
            1000,
        )
        .unwrap();
        let empty = QueueState::default();
        assert_eq!(empty.enqueue(source, Some(source.episode())), empty);
        let queued = empty.enqueue(source, None);
        assert_eq!(queued.enqueue(source, None), queued);
        let (empty, id) = queued.pop();
        assert_eq!(id, Some(source.episode()));
        assert_eq!(empty.pop(), (QueueState::default(), None));
    }
}
