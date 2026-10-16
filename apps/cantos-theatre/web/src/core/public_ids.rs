//! Canonical public catalog identity issued by the Theatre API. It is never a title, slug,
//! display position or mock fixture key, and holding one grants no access by itself.
use std::fmt;

/// Only the canonical lower-case hyphenated UUID form is admitted, so one Work has one route.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicWorkId(uuid::Uuid);

impl PublicWorkId {
    pub fn parse(value: &str) -> Option<Self> {
        canonical_uuid(value).map(Self)
    }
}

impl fmt::Display for PublicWorkId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(formatter)
    }
}

/// Braced, URN, simple or upper-case spellings are rejected: the contract issues one form.
pub fn canonical_uuid(value: &str) -> Option<uuid::Uuid> {
    let id = uuid::Uuid::try_parse(value).ok()?;
    (id.hyphenated().to_string() == value).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_canonical_lower_case_form_is_a_public_work_identity() {
        let canonical = "10000000-0000-4000-8000-00000000000a";
        let id = PublicWorkId::parse(canonical).unwrap();
        assert_eq!(id.to_string(), canonical);
        for rejected in [
            "10000000-0000-4000-8000-00000000000A",
            "{10000000-0000-4000-8000-00000000000a}",
            "urn:uuid:10000000-0000-4000-8000-00000000000a",
            "1000000000004000800000000000000a",
            " 10000000-0000-4000-8000-00000000000a",
            "son-ha-ky",
            "",
        ] {
            assert_eq!(PublicWorkId::parse(rejected), None, "{rejected}");
        }
    }
}
