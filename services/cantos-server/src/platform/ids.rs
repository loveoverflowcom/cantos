//! Canonical identifiers shared by every boundary. Pure: no I/O, randomness or clock.
use uuid::Uuid;

/// The canonical lowercase hyphenated form of `id`, or `None` if `id` is not already in it.
/// Accepting only canonical text keeps one spelling per identity in keys and digests.
pub fn canonical_uuid(id: &str) -> Option<String> {
    let canonical = Uuid::parse_str(id).ok()?.to_string();
    (id == canonical).then_some(canonical)
}

#[cfg(test)]
mod tests {
    use super::canonical_uuid;

    #[test]
    fn only_the_canonical_spelling_is_admitted() {
        let id = "5b7b7e4e-0a1d-4a0e-9d44-3f3f2d6d1a01";
        assert_eq!(canonical_uuid(id).as_deref(), Some(id));
        for other in [
            "5B7B7E4E-0A1D-4A0E-9D44-3F3F2D6D1A01",
            "5b7b7e4e0a1d4a0e9d443f3f2d6d1a01",
            "{5b7b7e4e-0a1d-4a0e-9d44-3f3f2d6d1a01}",
            "urn:uuid:5b7b7e4e-0a1d-4a0e-9d44-3f3f2d6d1a01",
            "",
        ] {
            assert_eq!(canonical_uuid(other), None, "{other}");
        }
    }
}
