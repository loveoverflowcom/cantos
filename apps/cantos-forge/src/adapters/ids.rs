//! Local identities from the operating system's random source.

use crate::{
    application::ports::{Ids, IdsUnavailable},
    domain::identity::{ArtifactId, EvidenceId},
};

#[derive(Clone, Copy, Debug, Default)]
pub struct OsIds;

fn random_bits() -> Result<u128, IdsUnavailable> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| IdsUnavailable)?;
    Ok(u128::from_be_bytes(bytes))
}

impl Ids for OsIds {
    fn artifact_id(&self) -> Result<ArtifactId, IdsUnavailable> {
        random_bits().map(ArtifactId::from_bits)
    }

    fn evidence_id(&self) -> Result<EvidenceId, IdsUnavailable> {
        random_bits().map(EvidenceId::from_bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_are_canonical_and_do_not_repeat() {
        let ids = OsIds;
        let first = ids.artifact_id().unwrap();
        let second = ids.artifact_id().unwrap();
        assert_ne!(first, second);
        assert_eq!(ArtifactId::parse(&first.to_string()), Ok(first));
        let evidence = ids.evidence_id().unwrap();
        assert!(evidence.to_string().starts_with("evd_"));
    }
}
