//! Immutable private storage identities and byte-read decisions. This module has no I/O,
//! provider, HTTP, database, credential or publication dependency. A valid reference identifies
//! expected bytes; it is not evidence that a store has actually persisted or verified them.

use sha2::{Digest, Sha256};

use crate::contexts::catalog::domain::OwnerId;

/// Existing private audio admission envelope. Other artifact classes need their own reviewed
/// bounds rather than silently admitting larger objects through this audio capability.
pub const MAX_OBJECT_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageCapability {
    ConditionalCreate,
    RangeRead,
    TemporaryRead,
}

/// Safe reason codes only: transport details, internal locations and credentials stay in the
/// concrete adapter. A timed-out write can have an unknown outcome; it is never proof of absence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    InvalidTarget,
    InvalidKey,
    InvalidIntegrity,
    DigestMismatch,
    LengthMismatch,
    Conflict,
    NotFound,
    Forbidden,
    Timeout,
    Cancelled,
    Unavailable,
    UnsupportedCapability(StorageCapability),
}

impl StorageError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidTarget => "invalid_storage_target",
            Self::InvalidKey => "invalid_object_key",
            Self::InvalidIntegrity => "invalid_object_integrity",
            Self::DigestMismatch => "object_digest_mismatch",
            Self::LengthMismatch => "object_length_mismatch",
            Self::Conflict => "immutable_object_conflict",
            Self::NotFound => "object_not_found",
            Self::Forbidden => "object_forbidden",
            Self::Timeout => "storage_timeout",
            Self::Cancelled => "storage_cancelled",
            Self::Unavailable => "storage_unavailable",
            Self::UnsupportedCapability(_) => "storage_capability_unsupported",
        }
    }
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for StorageError {}

/// Stable internal location identity, independent of public content identity and write default.
/// `legacy-filesystem` names historical filesystem records, not whichever target is active now.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StorageTargetId(String);

impl StorageTargetId {
    pub fn parse(value: &str) -> Result<Self, StorageError> {
        if !(1..=63).contains(&value.len())
            || !value.as_bytes()[0].is_ascii_lowercase()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            || value.ends_with('-')
        {
            return Err(StorageError::InvalidTarget);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The exact lowercase SHA-256 hex spelling is the only private object key. There are no path,
/// filename, title or URL construction paths. Adapters alone add their fixed private prefix.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKey(String);

impl ObjectKey {
    /// Restore a persisted internal identity. New writes derive their key from actual bytes.
    pub fn parse(value: &str) -> Result<Self, StorageError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StorageError::InvalidKey);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn for_bytes(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Valid expected integrity, not actual-object verification. Neither a remote ETag nor a HEAD
/// response can construct proof that stored bytes match this value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectIntegrity {
    sha256: ObjectKey,
    byte_length: u64,
}

impl ObjectIntegrity {
    pub fn new(sha256: &str, byte_length: u64) -> Result<Self, StorageError> {
        let sha256 = ObjectKey::parse(sha256).map_err(|_| StorageError::InvalidIntegrity)?;
        if !(1..=MAX_OBJECT_BYTES).contains(&byte_length) {
            return Err(StorageError::InvalidIntegrity);
        }
        Ok(Self {
            sha256,
            byte_length,
        })
    }

    pub fn for_bytes(bytes: &[u8]) -> Result<Self, StorageError> {
        let key = ObjectKey::for_bytes(bytes);
        Self::new(key.as_str(), bytes.len() as u64)
    }

    pub fn sha256(&self) -> &str {
        self.sha256.as_str()
    }

    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// Facts must come from hashing all actual bytes. Adapters own that effect and any bound on
    /// reading; metadata echoed by a provider is not an independent observation.
    pub fn verify_observed(&self, sha256: &str, byte_length: u64) -> Result<(), StorageError> {
        if byte_length != self.byte_length {
            return Err(StorageError::LengthMismatch);
        }
        let observed = ObjectKey::parse(sha256).map_err(|_| StorageError::InvalidIntegrity)?;
        if observed != self.sha256 {
            return Err(StorageError::DigestMismatch);
        }
        Ok(())
    }

    pub fn verify_bytes(&self, bytes: &[u8]) -> Result<(), StorageError> {
        let observed = ObjectKey::for_bytes(bytes);
        self.verify_observed(observed.as_str(), bytes.len() as u64)
    }
}

/// Private-only expected location. No conversion to a publication reference exists: accepted
/// private bytes still need the separate publication authority before listener delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateObjectRef {
    target: StorageTargetId,
    key: ObjectKey,
    integrity: ObjectIntegrity,
}

impl PrivateObjectRef {
    pub fn new(
        target: StorageTargetId,
        key: ObjectKey,
        integrity: ObjectIntegrity,
    ) -> Result<Self, StorageError> {
        if key.as_str() != integrity.sha256() {
            return Err(StorageError::DigestMismatch);
        }
        Ok(Self {
            target,
            key,
            integrity,
        })
    }

    pub fn target(&self) -> &StorageTargetId {
        &self.target
    }

    pub fn key(&self) -> &ObjectKey {
        &self.key
    }

    pub fn integrity(&self) -> &ObjectIntegrity {
        &self.integrity
    }
}

/// Application-supplied owner facts must be resolved for the exact requested resource/revision.
/// Equality authorizes only Studio's owner lane; it grants no Theatre or publication access.
pub fn decide_owner_read(actor: &OwnerId, owner: &OwnerId) -> Result<(), StorageError> {
    if actor != owner {
        return Err(StorageError::Forbidden);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeError {
    Invalid,
    MultipleRanges,
    Overflow,
    Unsatisfiable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RangeSpec {
    Closed { first: u64, last: u64 },
    Open { first: u64 },
    Suffix { length: u64 },
}

/// One parsed bytes range. Normalization needs the exact immutable object's actual length;
/// callers cannot forge a normalized interval with public fields or serde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteRange(RangeSpec);

impl ByteRange {
    pub fn parse(value: &str) -> Result<Self, RangeError> {
        let specification = value.strip_prefix("bytes=").ok_or(RangeError::Invalid)?;
        if specification.contains(',') {
            return Err(RangeError::MultipleRanges);
        }
        let (first, last) = specification.split_once('-').ok_or(RangeError::Invalid)?;
        let specification = match (first.is_empty(), last.is_empty()) {
            (true, true) => return Err(RangeError::Invalid),
            (true, false) => RangeSpec::Suffix {
                length: range_integer(last)?,
            },
            (false, true) => RangeSpec::Open {
                first: range_integer(first)?,
            },
            (false, false) => {
                let first = range_integer(first)?;
                let last = range_integer(last)?;
                if first > last {
                    return Err(RangeError::Invalid);
                }
                RangeSpec::Closed { first, last }
            }
        };
        Ok(Self(specification))
    }

    pub fn normalize(self, total_byte_length: u64) -> Result<NormalizedByteRange, RangeError> {
        let object_last = total_byte_length
            .checked_sub(1)
            .ok_or(RangeError::Unsatisfiable)?;
        let (first, last) = match self.0 {
            RangeSpec::Closed { first, last } => (first, last.min(object_last)),
            RangeSpec::Open { first } => (first, object_last),
            RangeSpec::Suffix { length } => {
                if length == 0 {
                    return Err(RangeError::Unsatisfiable);
                }
                (
                    total_byte_length - length.min(total_byte_length),
                    object_last,
                )
            }
        };
        if first > object_last {
            return Err(RangeError::Unsatisfiable);
        }
        Ok(NormalizedByteRange {
            first,
            last,
            total_byte_length,
        })
    }
}

fn range_integer(value: &str) -> Result<u64, RangeError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RangeError::Invalid);
    }
    value.parse().map_err(|_| RangeError::Overflow)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NormalizedByteRange {
    first: u64,
    last: u64,
    total_byte_length: u64,
}

impl NormalizedByteRange {
    pub fn full(total_byte_length: u64) -> Result<Self, RangeError> {
        ByteRange(RangeSpec::Open { first: 0 }).normalize(total_byte_length)
    }

    pub fn first(self) -> u64 {
        self.first
    }

    pub fn last(self) -> u64 {
        self.last
    }

    pub fn byte_length(self) -> u64 {
        // normalize guarantees 0 <= first <= last < total_byte_length <= u64::MAX.
        self.last - self.first + 1
    }

    pub fn total_byte_length(self) -> u64 {
        self.total_byte_length
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decide_owner_read, ByteRange, NormalizedByteRange, ObjectIntegrity, ObjectKey,
        PrivateObjectRef, RangeError, StorageCapability, StorageError, StorageTargetId,
        MAX_OBJECT_BYTES,
    };
    use crate::contexts::catalog::domain::OwnerId;
    use proptest::prelude::*;

    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn target_identity_is_stable_and_rejects_paths_unicode_and_noncanonical_spelling() {
        for value in ["local-dev", "legacy-filesystem", "r2", "a", &"a".repeat(63)] {
            assert_eq!(StorageTargetId::parse(value).unwrap().as_str(), value);
        }
        for value in [
            "",
            "../local",
            "a/b",
            "a\\b",
            "Local",
            "á",
            "a b",
            "a\n",
            "1",
            "-a",
            "a-",
            &"a".repeat(64),
        ] {
            assert_eq!(
                StorageTargetId::parse(value),
                Err(StorageError::InvalidTarget)
            );
        }
    }

    #[test]
    fn object_key_is_only_the_actual_sha256_not_a_title_filename_or_url() {
        assert_eq!(ObjectKey::for_bytes(b"abc").as_str(), ABC_SHA256);
        assert_eq!(
            ObjectKey::parse(ABC_SHA256).unwrap(),
            ObjectKey::for_bytes(b"abc")
        );
        for value in [
            "",
            "../abc",
            "story.ogg",
            "https://store/abc",
            "Ánh đèn cuối sân khấu",
            &"0".repeat(63),
            &"0".repeat(65),
            &"g".repeat(64),
            &"A".repeat(64),
            &format!("/{}", "0".repeat(63)),
        ] {
            assert_eq!(ObjectKey::parse(value), Err(StorageError::InvalidKey));
        }
    }

    #[test]
    fn integrity_has_the_existing_bounded_envelope_and_exact_error_partitions() {
        for length in [1, MAX_OBJECT_BYTES] {
            assert_eq!(
                ObjectIntegrity::new(ABC_SHA256, length)
                    .unwrap()
                    .byte_length(),
                length
            );
        }
        for length in [0, MAX_OBJECT_BYTES + 1, u64::MAX] {
            assert_eq!(
                ObjectIntegrity::new(ABC_SHA256, length),
                Err(StorageError::InvalidIntegrity)
            );
        }
        assert_eq!(
            ObjectIntegrity::new("not-a-digest", 3),
            Err(StorageError::InvalidIntegrity)
        );
        let integrity = ObjectIntegrity::for_bytes(b"abc").unwrap();
        assert_eq!(integrity.verify_bytes(b"abc"), Ok(()));
        assert_eq!(
            integrity.verify_bytes(b"ab"),
            Err(StorageError::LengthMismatch)
        );
        assert_eq!(
            integrity.verify_bytes(b""),
            Err(StorageError::LengthMismatch)
        );
        assert_eq!(
            integrity.verify_bytes(b"abd"),
            Err(StorageError::DigestMismatch)
        );
        assert_eq!(
            integrity.verify_observed("malformed", 3),
            Err(StorageError::InvalidIntegrity)
        );
        assert_eq!(
            integrity.verify_observed(ABC_SHA256, MAX_OBJECT_BYTES + 1),
            Err(StorageError::LengthMismatch)
        );
        assert_eq!(
            ObjectIntegrity::for_bytes(b""),
            Err(StorageError::InvalidIntegrity)
        );
    }

    #[test]
    fn private_reference_binds_one_exact_target_key_and_integrity() {
        let target = StorageTargetId::parse("legacy-filesystem").unwrap();
        let key = ObjectKey::for_bytes(b"abc");
        let integrity = ObjectIntegrity::for_bytes(b"abc").unwrap();
        let object = PrivateObjectRef::new(target.clone(), key.clone(), integrity.clone()).unwrap();
        assert_eq!(object.target(), &target);
        assert_eq!(object.key(), &key);
        assert_eq!(object.integrity(), &integrity);
        assert_eq!(
            PrivateObjectRef::new(target, ObjectKey::for_bytes(b"abd"), integrity),
            Err(StorageError::DigestMismatch)
        );
    }

    #[test]
    fn owner_read_decision_admits_only_the_exact_owner() {
        let owner = OwnerId::parse("creator-one").unwrap();
        let other = OwnerId::parse("creator-two").unwrap();
        assert_eq!(decide_owner_read(&owner, &owner), Ok(()));
        assert_eq!(
            decide_owner_read(&other, &owner),
            Err(StorageError::Forbidden)
        );
    }

    #[test]
    fn single_ranges_cover_first_middle_final_open_and_suffix_segments() {
        for (raw, first, last) in [
            ("bytes=0-2", 0, 2),
            ("bytes=3-5", 3, 5),
            ("bytes=7-9", 7, 9),
            ("bytes=7-", 7, 9),
            ("bytes=-3", 7, 9),
            ("bytes=7-99", 7, 9),
            ("bytes=-99", 0, 9),
            ("bytes=0-0", 0, 0),
        ] {
            let range = ByteRange::parse(raw).unwrap().normalize(10).unwrap();
            assert_eq!(
                (
                    range.first(),
                    range.last(),
                    range.byte_length(),
                    range.total_byte_length()
                ),
                (first, last, last - first + 1, 10)
            );
        }
        assert_eq!(NormalizedByteRange::full(10).unwrap().byte_length(), 10);
    }

    #[test]
    fn range_parser_rejects_invalid_multiple_overflow_and_unsatisfiable_inputs() {
        for raw in [
            "",
            "items=0-1",
            "Bytes=0-1",
            "bytes=-",
            "bytes=+1-2",
            "bytes=0--1",
            "bytes=2-1",
            "bytes=1 -2",
            "bytes=0-1\n",
        ] {
            assert_eq!(ByteRange::parse(raw), Err(RangeError::Invalid));
        }
        assert_eq!(
            ByteRange::parse("bytes=0-1,2-3"),
            Err(RangeError::MultipleRanges)
        );
        for raw in [
            "bytes=18446744073709551616-",
            "bytes=-18446744073709551616",
            "bytes=0-18446744073709551616",
        ] {
            assert_eq!(ByteRange::parse(raw), Err(RangeError::Overflow));
        }
        for raw in [
            "bytes=10-",
            "bytes=10-11",
            "bytes=-0",
            "bytes=18446744073709551615-",
        ] {
            assert_eq!(
                ByteRange::parse(raw).unwrap().normalize(10),
                Err(RangeError::Unsatisfiable)
            );
        }
        assert_eq!(
            ByteRange::parse("bytes=0-0").unwrap().normalize(0),
            Err(RangeError::Unsatisfiable)
        );
        assert_eq!(NormalizedByteRange::full(0), Err(RangeError::Unsatisfiable));
    }

    #[test]
    fn machine_width_boundary_is_safe_without_wrapping_range_length() {
        let total = u64::MAX;
        let range = ByteRange::parse("bytes=0-18446744073709551615")
            .unwrap()
            .normalize(total)
            .unwrap();
        assert_eq!(range.last(), total - 1);
        assert_eq!(range.byte_length(), total);
        let suffix = ByteRange::parse("bytes=-18446744073709551615")
            .unwrap()
            .normalize(total)
            .unwrap();
        assert_eq!(suffix, range);
    }

    #[test]
    fn bounded_range_exhaustion_agrees_with_independent_membership_oracle() {
        for total in 0_u64..=16 {
            for first in 0_u64..=20 {
                for last in first..=20 {
                    let result = ByteRange::parse(&format!("bytes={first}-{last}"))
                        .unwrap()
                        .normalize(total);
                    let expected: Vec<_> = (0..total)
                        .filter(|index| first <= *index && *index <= last)
                        .collect();
                    if expected.is_empty() {
                        assert_eq!(result, Err(RangeError::Unsatisfiable));
                    } else {
                        let observed = result.unwrap();
                        assert_eq!(
                            (observed.first()..=observed.last()).collect::<Vec<_>>(),
                            expected
                        );
                        assert_eq!(observed.byte_length(), expected.len() as u64);
                    }
                }
            }
            for suffix in 0_u64..=20 {
                let result = ByteRange::parse(&format!("bytes=-{suffix}"))
                    .unwrap()
                    .normalize(total);
                let expected: Vec<_> = (0..total)
                    .rev()
                    .take(suffix as usize)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                if expected.is_empty() {
                    assert_eq!(result, Err(RangeError::Unsatisfiable));
                } else {
                    let observed = result.unwrap();
                    assert_eq!(
                        (observed.first()..=observed.last()).collect::<Vec<_>>(),
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn storage_error_codes_are_total_and_disclose_no_transport_or_location_details() {
        for (error, code) in [
            (StorageError::InvalidTarget, "invalid_storage_target"),
            (StorageError::InvalidKey, "invalid_object_key"),
            (StorageError::InvalidIntegrity, "invalid_object_integrity"),
            (StorageError::DigestMismatch, "object_digest_mismatch"),
            (StorageError::LengthMismatch, "object_length_mismatch"),
            (StorageError::Conflict, "immutable_object_conflict"),
            (StorageError::NotFound, "object_not_found"),
            (StorageError::Forbidden, "object_forbidden"),
            (StorageError::Timeout, "storage_timeout"),
            (StorageError::Cancelled, "storage_cancelled"),
            (StorageError::Unavailable, "storage_unavailable"),
        ] {
            assert_eq!(error.code(), code);
        }
        for capability in [
            StorageCapability::ConditionalCreate,
            StorageCapability::RangeRead,
            StorageCapability::TemporaryRead,
        ] {
            assert_eq!(
                StorageError::UnsupportedCapability(capability).code(),
                "storage_capability_unsupported"
            );
        }
    }

    proptest! {
        #[test]
        fn actual_byte_key_and_integrity_are_stable(bytes in prop::collection::vec(any::<u8>(), 1..=4096)) {
            let key = ObjectKey::for_bytes(&bytes);
            prop_assert_eq!(ObjectKey::parse(key.as_str()).unwrap(), key.clone());
            let integrity = ObjectIntegrity::for_bytes(&bytes).unwrap();
            prop_assert_eq!(integrity.sha256(), key.as_str());
            prop_assert_eq!(integrity.byte_length(), bytes.len() as u64);
            prop_assert_eq!(integrity.verify_bytes(&bytes), Ok(()));
        }

        #[test]
        fn normalized_ranges_never_escape_exact_object(total in 1_u64..=u64::MAX, first in any::<u64>(), last in any::<u64>()) {
            let (first, last) = (first.min(last), first.max(last));
            let range = ByteRange::parse(&format!("bytes={first}-{last}")).unwrap().normalize(total);
            if first >= total {
                prop_assert_eq!(range, Err(RangeError::Unsatisfiable));
            } else {
                let range = range.unwrap();
                prop_assert!(range.first() <= range.last());
                prop_assert!(range.last() < total);
                prop_assert!(range.byte_length() > 0);
                prop_assert!(range.byte_length() <= total);
                prop_assert_eq!(range.first(), first);
            }
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::{ByteRange, RangeError, RangeSpec};
    /// Every parser-admissible closed/open/suffix specification and full u64 object length,
    /// no assumptions/stubs. Proves exact single-range normalization stays inside the exact
    /// immutable object; not header parsing, multi-range handling, storage reads or HTTP.
    #[kani::proof]
    #[kani::unwind(2)]
    fn byte_range_normalization_stays_inside_the_exact_object() {
        let total: u64 = kani::any();
        let a: u64 = kani::any();
        let b: u64 = kani::any();
        // `ByteRange::parse` rejects first > last, so closed specifications are ordered.
        let spec = match kani::any::<u8>() % 3 {
            0 => RangeSpec::Closed {
                first: a.min(b),
                last: a.max(b),
            },
            1 => RangeSpec::Open { first: a },
            _ => RangeSpec::Suffix { length: a },
        };
        let result = ByteRange(spec).normalize(total);
        let expected = if total == 0 {
            Err(RangeError::Unsatisfiable)
        } else {
            let object_last = total - 1;
            match spec {
                RangeSpec::Closed { first, last } if first < total => {
                    Ok((first, last.min(object_last)))
                }
                RangeSpec::Open { first } if first < total => Ok((first, object_last)),
                RangeSpec::Suffix { length } if length > 0 => {
                    Ok((total - length.min(total), object_last))
                }
                _ => Err(RangeError::Unsatisfiable),
            }
        };
        assert_eq!(result.map(|range| (range.first(), range.last())), expected);
        if let Ok(range) = result {
            assert!(range.first() <= range.last() && range.last() < total);
            assert_eq!(range.total_byte_length(), total);
            assert_eq!(
                u128::from(range.byte_length()),
                u128::from(range.last()) - u128::from(range.first()) + 1
            );
        }
        kani::cover!(
            matches!(spec, RangeSpec::Suffix { length } if length > total) && result.is_ok(),
            "suffix longer than the object selects all of it"
        );
        kani::cover!(
            matches!(spec, RangeSpec::Closed { last, .. } if last >= total) && result.is_ok(),
            "closed range clamped to the last byte"
        );
        kani::cover!(
            matches!(spec, RangeSpec::Open { first } if first == total)
                && result == Err(RangeError::Unsatisfiable),
            "first byte at the length is unsatisfiable"
        );
        kani::cover!(
            matches!(spec, RangeSpec::Suffix { length: 0 }) && total > 0,
            "empty suffix is unsatisfiable"
        );
        kani::cover!(
            total == 0 && result == Err(RangeError::Unsatisfiable),
            "empty object has no satisfiable range"
        );
        kani::cover!(
            total == u64::MAX && result.is_ok_and(|range| range.last() == u64::MAX - 1),
            "largest representable object reaches its last byte"
        );
    }
}
