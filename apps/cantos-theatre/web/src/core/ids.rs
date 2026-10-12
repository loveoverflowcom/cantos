//! Typed keys for synthetic catalog fixtures; never publication authority.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkId {
    Mountain,
    River,
    Dream,
}
impl WorkId {
    pub const ALL: [Self; 3] = [Self::Mountain, Self::River, Self::Dream];
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Mountain => "son-ha-ky",
            Self::River => "den-khuya",
            Self::Dream => "mong-ho-diep",
        }
    }
    pub fn from_slug(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|work| work.slug() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EpisodeId {
    pub work: WorkId,
    pub index: usize,
}
