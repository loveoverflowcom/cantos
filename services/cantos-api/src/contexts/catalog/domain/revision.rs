//! Accepted Catalog/Content sequences. Zero is only a raw pre-first-content head,
//! never the sequence of an admitted snapshot or exact ScriptRef.
use serde::Serialize;

use super::CatalogError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(super) struct Revision(u64);

impl Revision {
    pub(super) const fn new(value: u64) -> Result<Self, CatalogError> {
        if value == 0 || value > i64::MAX as u64 {
            return Err(CatalogError::InvalidRevision);
        }
        Ok(Self(value))
    }

    pub(super) const fn get(self) -> u64 {
        self.0
    }

    pub(super) const fn next(self) -> Result<Self, CatalogError> {
        if self.0 == i64::MAX as u64 {
            return Err(CatalogError::RevisionLimit);
        }
        // The private constructor proves 1 <= self.0 < i64::MAX here.
        Ok(Self(self.0 + 1))
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{CatalogError, Revision};

    #[test]
    fn admission_and_successor_pin_exact_storage_edges() {
        for value in [0, i64::MAX as u64 + 1, u64::MAX] {
            assert_eq!(Revision::new(value), Err(CatalogError::InvalidRevision));
        }
        assert_eq!(Revision::new(1).unwrap().next().unwrap().get(), 2);
        assert_eq!(
            Revision::new(i64::MAX as u64).unwrap().next(),
            Err(CatalogError::RevisionLimit)
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(20_261_013), ..ProptestConfig::default() })]
        #[test]
        fn admitted_sequence_and_successor_match_a_wider_integer_oracle(value in any::<u64>()) {
            let admitted = Revision::new(value);
            if (1..=i64::MAX as u64).contains(&value) {
                let revision = admitted.unwrap();
                prop_assert_eq!(revision.get(), value);
                let next = u128::from(value) + 1;
                if next <= i64::MAX as u128 {
                    prop_assert_eq!(revision.next().unwrap().get() as u128, next);
                } else {
                    prop_assert_eq!(revision.next(), Err(CatalogError::RevisionLimit));
                }
                prop_assert_eq!(serde_json::to_value(revision).unwrap(), serde_json::json!(value));
            } else {
                prop_assert_eq!(admitted, Err(CatalogError::InvalidRevision));
            }
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::{CatalogError, Revision};

    /// Proposition: admission is exactly 1..=i64::MAX; every admitted non-maximal revision
    /// has its exact storage-bounded successor, and the maximal one fails without wrapping.
    /// Domain: the full symbolic u64. Assumptions/stubs: none. Unwind: 2; no business loop.
    /// Calls the production Revision::{new,get,next}, not an arithmetic substitute.
    #[kani::proof]
    #[kani::unwind(2)]
    fn catalog_revision_admission_and_successor() {
        let value: u64 = kani::any();
        match Revision::new(value) {
            Ok(revision) => {
                assert!(value > 0 && value <= i64::MAX as u64);
                assert_eq!(revision.get(), value);
                match revision.next() {
                    Ok(next) => {
                        assert!(value < i64::MAX as u64);
                        assert_eq!(u128::from(next.get()), u128::from(value) + 1);
                        assert!(next.get() > 0 && next.get() <= i64::MAX as u64);
                    }
                    Err(CatalogError::RevisionLimit) => assert_eq!(value, i64::MAX as u64),
                    Err(_) => assert!(false, "unexpected successor error"),
                }
            }
            Err(CatalogError::InvalidRevision) => {
                assert!(value == 0 || value > i64::MAX as u64);
            }
            Err(_) => assert!(false, "unexpected admission error"),
        }
        kani::cover!(value == 0, "zero rejected");
        kani::cover!(value == 1, "first accepted revision");
        kani::cover!(value == i64::MAX as u64 - 1, "last successful successor");
        kani::cover!(value == i64::MAX as u64, "storage limit rejected successor");
        kani::cover!(
            value == i64::MAX as u64 + 1,
            "above storage domain rejected"
        );
        kani::cover!(value == u64::MAX, "full machine-width maximum rejected");
    }
}
