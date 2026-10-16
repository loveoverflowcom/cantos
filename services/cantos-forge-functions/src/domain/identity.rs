//! Forge-local identities.
//!
//! A local identity names a record in one producer workspace. It is never a Cantos server
//! identity: server IDs exist only after the API issues or confirms them and will be separate
//! types introduced with the handoff that receives them.

use std::{cmp::Ordering, fmt, hash, marker::PhantomData};

/// The kind of a local identity. Sealed so that only this module can define kinds.
pub trait LocalKind: sealed::Sealed {
    /// Text prefix that keeps kinds distinct outside the type system, for example in a manifest.
    const PREFIX: &'static str;
}

mod sealed {
    pub trait Sealed {}
}

/// Kind marker for a registered artifact (manuscript, Script IR, audio, report, instructions).
#[derive(Debug)]
pub enum Artifact {}

/// Kind marker for an append-only evaluation evidence record.
#[derive(Debug)]
pub enum Evidence {}

impl sealed::Sealed for Artifact {}
impl LocalKind for Artifact {
    const PREFIX: &'static str = "art";
}

impl sealed::Sealed for Evidence {}
impl LocalKind for Evidence {
    const PREFIX: &'static str = "evd";
}

pub type ArtifactId = LocalId<Artifact>;
pub type EvidenceId = LocalId<Evidence>;

const VALUE_HEX_DIGITS: usize = 32;

/// A 128-bit local identity, written canonically as `<prefix>_<32 lowercase hex digits>`.
///
/// The value comes from the shell (a random source or a parsed manifest); this type never
/// generates one. Kinds cannot be mixed:
///
/// ```compile_fail,E0308
/// use cantos_forge::domain::identity::{ArtifactId, EvidenceId};
/// let artifact = ArtifactId::from_bits(7);
/// let evidence: EvidenceId = artifact;
/// ```
pub struct LocalId<K> {
    value: u128,
    kind: PhantomData<fn() -> K>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalIdError {
    /// The text does not start with this kind's prefix followed by `_`.
    WrongPrefix { expected: &'static str },
    /// The part after the prefix is not exactly 32 lowercase hexadecimal digits.
    MalformedValue,
}

impl<K: LocalKind> LocalId<K> {
    /// Wraps bits supplied by the shell's random source.
    pub fn from_bits(value: u128) -> Self {
        Self {
            value,
            kind: PhantomData,
        }
    }

    pub fn parse(text: &str) -> Result<Self, LocalIdError> {
        let value = text
            .strip_prefix(K::PREFIX)
            .and_then(|rest| rest.strip_prefix('_'))
            .ok_or(LocalIdError::WrongPrefix {
                expected: K::PREFIX,
            })?;
        let canonical = value.len() == VALUE_HEX_DIGITS
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if !canonical {
            return Err(LocalIdError::MalformedValue);
        }
        u128::from_str_radix(value, 16)
            .map(Self::from_bits)
            .map_err(|_| LocalIdError::MalformedValue)
    }
}

impl<K: LocalKind> fmt::Display for LocalId<K> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}_{:032x}", K::PREFIX, self.value)
    }
}

impl<K: LocalKind> fmt::Debug for LocalId<K> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, out)
    }
}

// Manual impls: derives would demand the same traits of the uninhabited kind marker.
impl<K> Clone for LocalId<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for LocalId<K> {}

impl<K> PartialEq for LocalId<K> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<K> Eq for LocalId<K> {}

impl<K> PartialOrd for LocalId<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K> Ord for LocalId<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<K> hash::Hash for LocalId<K> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn canonical_text_round_trips() {
        let text = "art_0123456789abcdef0123456789abcdef";
        let id = ArtifactId::parse(text).unwrap();
        assert_eq!(id.to_string(), text);
        assert_eq!(
            ArtifactId::from_bits(1).to_string(),
            "art_00000000000000000000000000000001"
        );
    }

    #[test]
    fn another_kinds_prefix_is_refused() {
        let evidence = EvidenceId::from_bits(42).to_string();
        assert_eq!(
            ArtifactId::parse(&evidence),
            Err(LocalIdError::WrongPrefix { expected: "art" })
        );
        assert_eq!(
            EvidenceId::parse("art_00000000000000000000000000000001"),
            Err(LocalIdError::WrongPrefix { expected: "evd" })
        );
    }

    #[test]
    fn non_canonical_values_are_refused() {
        let cases = [
            ("art", LocalIdError::WrongPrefix { expected: "art" }),
            (
                "art-0123456789abcdef0123456789abcdef",
                LocalIdError::WrongPrefix { expected: "art" },
            ),
            (
                "ART_0123456789abcdef0123456789abcdef",
                LocalIdError::WrongPrefix { expected: "art" },
            ),
            ("art_", LocalIdError::MalformedValue),
            (
                "art_0123456789ABCDEF0123456789abcdef",
                LocalIdError::MalformedValue,
            ),
            (
                "art_0123456789abcdef0123456789abcde",
                LocalIdError::MalformedValue,
            ),
            (
                "art_0123456789abcdef0123456789abcdef0",
                LocalIdError::MalformedValue,
            ),
            (
                "art_+123456789abcdef0123456789abcdef",
                LocalIdError::MalformedValue,
            ),
            (
                "art_0123456789abcdef 123456789abcdef",
                LocalIdError::MalformedValue,
            ),
            (
                "art_0123456789abcdef0123456789abcdeg",
                LocalIdError::MalformedValue,
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(ArtifactId::parse(text), Err(expected), "{text}");
        }
    }

    proptest! {
        #[test]
        fn display_then_parse_is_identity(bits in any::<u128>()) {
            let id = ArtifactId::from_bits(bits);
            prop_assert_eq!(ArtifactId::parse(&id.to_string()), Ok(id));
        }

        #[test]
        fn equality_and_order_follow_the_canonical_text(a in any::<u128>(), b in any::<u128>()) {
            let (left, right) = (ArtifactId::from_bits(a), ArtifactId::from_bits(b));
            prop_assert_eq!(left == right, left.to_string() == right.to_string());
            prop_assert_eq!(left.cmp(&right), left.to_string().cmp(&right.to_string()));
        }
    }
}
