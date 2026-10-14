use serde::Serialize;

use super::{ScriptRef, SourceRef, UnitDetails, Visibility, WorkDetails};

/// Optional work metadata changes. Omitted fields retain their current values; the
/// resulting details still pass through the Work transition's normal validation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct WorkPatch {
    pub title: Option<String>,
    pub synopsis: Option<String>,
    pub language: Option<String>,
    pub visibility: Option<Visibility>,
    pub rights_refs: Option<Vec<String>>,
}

impl WorkPatch {
    pub fn apply(self, current: &WorkDetails) -> WorkDetails {
        WorkDetails {
            title: self.title.unwrap_or_else(|| current.title.clone()),
            synopsis: self.synopsis.unwrap_or_else(|| current.synopsis.clone()),
            language: self.language.unwrap_or_else(|| current.language.clone()),
            visibility: self.visibility.unwrap_or(current.visibility),
            rights_refs: self
                .rights_refs
                .unwrap_or_else(|| current.rights_refs.clone()),
        }
    }
}

/// An optional binding distinguishes retaining the current reference from explicitly
/// removing it. The HTTP boundary decides which variant an omitted or null field means.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "action", content = "value", rename_all = "snake_case")]
pub enum LinkPatch<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}

impl<T: Clone> LinkPatch<T> {
    fn apply(self, current: &Option<T>) -> Option<T> {
        match self {
            Self::Keep => current.clone(),
            Self::Set(value) => Some(value),
            Self::Clear => None,
        }
    }
}

/// Unadmitted unit changes. Pinned source/script identities remain intact when omitted;
/// applying this value creates details for the existing validated ContentUnit transition.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct UnitPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub source: LinkPatch<SourceRef>,
    pub script: LinkPatch<ScriptRef>,
    pub content_digest: LinkPatch<String>,
}

impl UnitPatch {
    pub fn apply(self, current: &UnitDetails) -> UnitDetails {
        UnitDetails {
            title: self.title.unwrap_or_else(|| current.title.clone()),
            description: self
                .description
                .unwrap_or_else(|| current.description.clone()),
            source: self.source.apply(&current.source),
            script: self.script.apply(&current.script),
            content_digest: self.content_digest.apply(&current.content_digest),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LinkPatch;

    #[test]
    fn default_keeps_the_binding_without_requiring_a_default_payload() {
        struct NoDefault;
        assert!(matches!(LinkPatch::<NoDefault>::default(), LinkPatch::Keep));
    }
}
