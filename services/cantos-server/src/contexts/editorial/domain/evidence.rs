//! Evidence records a revision may cite: imported sources, rights claims and generation
//! bindings. A record is immutable once registered; citing it is checked on every save.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceKind {
    Source,
    Rights,
    Generation,
}

impl EvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceKind::Source => "source",
            EvidenceKind::Rights => "rights",
            EvidenceKind::Generation => "generation",
        }
    }
}
