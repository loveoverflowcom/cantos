//! Blocking Argon2id hash and verify. Callers run these only through [`super::KdfPool`]:
//! never inside a database transaction and never on an async executor thread.
//!
//! Stored hashes are untrusted input to the KDF. Algorithm, version, parameters and lengths
//! are checked against the policy floor and ceiling BEFORE the library allocates memory, so a
//! corrupted or planted row cannot cause unbounded allocation or a weaker comparison.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};

use crate::{
    domain::password::PasswordInput,
    persistence::PhcString,
    policy::{
        KDF_ITERATIONS, KDF_LANES, KDF_MAX_ITERATIONS, KDF_MAX_LANES, KDF_MAX_MEMORY_KIB,
        KDF_MEMORY_KIB, KDF_OUTPUT_BYTES, KDF_SALT_BYTES,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KdfFault {
    EntropyUnavailable,
    /// The library refused to hash. Never carries input or library text.
    HashFailed,
    /// The stored hash is not a policy-conforming Argon2id PHC string.
    UnsupportedHash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    Match,
    Mismatch,
}

fn policy_params() -> Result<Params, KdfFault> {
    Params::new(
        KDF_MEMORY_KIB,
        KDF_ITERATIONS,
        KDF_LANES,
        Some(KDF_OUTPUT_BYTES),
    )
    .map_err(|_| KdfFault::HashFailed)
}

/// Hashes with the policy parameters and a fresh OS-random salt.
pub fn hash_password(password: &PasswordInput) -> Result<PhcString, KdfFault> {
    let mut salt = [0_u8; KDF_SALT_BYTES];
    getrandom::fill(&mut salt).map_err(|_| KdfFault::EntropyUnavailable)?;
    let salt = SaltString::encode_b64(&salt).map_err(|_| KdfFault::HashFailed)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, policy_params()?);
    let hash = argon
        .hash_password(password.expose(), &salt)
        .map_err(|_| KdfFault::HashFailed)?;
    PhcString::new(hash.to_string()).map_err(|_| KdfFault::HashFailed)
}

/// Checks the stored hash's shape and parameters without touching the KDF.
fn admit(hash: &PasswordHash<'_>) -> Result<(), KdfFault> {
    if hash.algorithm.as_str() != "argon2id" || hash.version != Some(0x13) {
        return Err(KdfFault::UnsupportedHash);
    }
    let params = Params::try_from(hash).map_err(|_| KdfFault::UnsupportedHash)?;
    let within = (KDF_MEMORY_KIB..=KDF_MAX_MEMORY_KIB).contains(&params.m_cost())
        && (KDF_ITERATIONS..=KDF_MAX_ITERATIONS).contains(&params.t_cost())
        && (KDF_LANES..=KDF_MAX_LANES).contains(&params.p_cost());
    let salt_ok = hash
        .salt
        .is_some_and(|salt| salt.len() >= 22 && salt.len() <= 86);
    let output_ok = hash
        .hash
        .is_some_and(|output| output.len() == KDF_OUTPUT_BYTES);
    if within && salt_ok && output_ok && params.output_len() == Some(KDF_OUTPUT_BYTES) {
        Ok(())
    } else {
        Err(KdfFault::UnsupportedHash)
    }
}

/// Verifies with the library's constant-time comparison.
pub fn verify_password(
    password: &PasswordInput,
    stored: &PhcString,
) -> Result<Verification, KdfFault> {
    let parsed = PasswordHash::new(stored.as_str()).map_err(|_| KdfFault::UnsupportedHash)?;
    admit(&parsed)?;
    match Argon2::default().verify_password(password.expose(), &parsed) {
        Ok(()) => Ok(Verification::Match),
        Err(argon2::password_hash::Error::Password) => Ok(Verification::Mismatch),
        Err(_) => Err(KdfFault::UnsupportedHash),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn password(text: &str) -> PasswordInput {
        PasswordInput::new(text).unwrap()
    }

    #[test]
    fn a_fresh_hash_is_argon2id_with_policy_parameters_and_verifies() {
        let phc = hash_password(&password("correct horse battery")).unwrap();
        assert!(phc.as_str().starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert_eq!(
            verify_password(&password("correct horse battery"), &phc),
            Ok(Verification::Match)
        );
        assert_eq!(
            verify_password(&password("correct horse batterz"), &phc),
            Ok(Verification::Mismatch)
        );
        assert_eq!(
            verify_password(&password("Correct horse battery"), &phc),
            Ok(Verification::Mismatch)
        );
    }

    #[test]
    fn every_hash_has_its_own_salt() {
        let first = hash_password(&password("correct horse battery")).unwrap();
        let second = hash_password(&password("correct horse battery")).unwrap();
        assert_ne!(first, second);
        let salt = |phc: &PhcString| phc.as_str().split('$').nth(4).unwrap().to_owned();
        assert_ne!(salt(&first), salt(&second));
    }

    #[test]
    fn hashes_outside_the_policy_window_are_refused_before_the_kdf_runs() {
        let valid = hash_password(&password("correct horse battery")).unwrap();
        let tail = valid.as_str().split('$').nth(5).unwrap().to_owned();
        let salt = valid.as_str().split('$').nth(4).unwrap().to_owned();
        let with = |params: &str| {
            PhcString::new(format!("$argon2id$v=19${params}${salt}${tail}")).unwrap()
        };
        for params in [
            "m=8,t=2,p=1",          // far below the memory floor
            "m=19455,t=2,p=1",      // one below
            "m=19456,t=1,p=1",      // iterations below floor
            "m=19456,t=2,p=0",      // zero lanes
            "m=65537,t=2,p=1",      // one above the memory ceiling
            "m=4294967295,t=2,p=1", // allocation bomb
            "m=19456,t=7,p=1",      // iterations above ceiling
            "m=19456,t=2,p=5",      // lanes above ceiling
        ] {
            assert_eq!(
                verify_password(&password("correct horse battery"), &with(params)),
                Err(KdfFault::UnsupportedHash),
                "{params}"
            );
        }
        // The window edges themselves are admitted (they fail only on the digest).
        for params in ["m=65536,t=6,p=4", "m=19456,t=2,p=1"] {
            assert_ne!(
                verify_password(&password("correct horse battery"), &with(params)),
                Err(KdfFault::UnsupportedHash),
                "{params}"
            );
        }
    }

    #[test]
    fn other_algorithms_versions_and_lengths_are_refused() {
        let valid = hash_password(&password("correct horse battery")).unwrap();
        let text = valid.as_str();
        let swapped = [
            text.replace("$argon2id$", "$argon2i$"),
            text.replace("v=19", "v=16"),
        ];
        for candidate in swapped {
            // `PhcString` already refuses a non-Argon2id prefix at the storage boundary.
            if let Ok(phc) = PhcString::new(candidate) {
                assert_eq!(
                    verify_password(&password("correct horse battery"), &phc),
                    Err(KdfFault::UnsupportedHash)
                );
            }
        }
        let short_salt = format!(
            "$argon2id$v=19$m=19456,t=2,p=1$YWJj${}",
            text.rsplit('$').next().unwrap()
        );
        assert_eq!(
            verify_password(
                &password("correct horse battery"),
                &PhcString::new(short_salt).unwrap()
            ),
            Err(KdfFault::UnsupportedHash)
        );
        let short_output = format!(
            "{}$QUJDREVGR0hJSktMTU5PUA",
            text.rsplit_once('$').unwrap().0
        );
        assert_eq!(
            verify_password(
                &password("correct horse battery"),
                &PhcString::new(short_output).unwrap()
            ),
            Err(KdfFault::UnsupportedHash)
        );
    }

    #[test]
    fn the_library_reproduces_the_rfc_9106_argon2id_vector() {
        // RFC 9106 section 5.3 (Argon2id): password 32x01, salt 16x02, secret 8x03, associated
        // data 12x04, m=32 KiB, t=3, p=4, 32-byte tag. The tag is copied from the RFC text,
        // not computed by this crate. It validates the pinned library, not this service's
        // parameter policy (m=32 is far below the floor).
        use argon2::{AssociatedData, ParamsBuilder};
        let mut builder = ParamsBuilder::new();
        builder
            .m_cost(32)
            .t_cost(3)
            .p_cost(4)
            .output_len(32)
            .data(AssociatedData::new(&[4; 12]).unwrap());
        let argon = Argon2::new_with_secret(
            &[3; 8],
            Algorithm::Argon2id,
            Version::V0x13,
            builder.build().unwrap(),
        )
        .unwrap();
        let mut tag = [0_u8; 32];
        argon
            .hash_password_into(&[1; 32], &[2; 16], &mut tag)
            .unwrap();
        let expected: [u8; 32] = [
            0x0d, 0x64, 0x0d, 0xf5, 0x8d, 0x78, 0x76, 0x6c, 0x08, 0xc0, 0x37, 0xa3, 0x4a, 0x8b,
            0x53, 0xc9, 0xd0, 0x1e, 0xf0, 0x45, 0x2d, 0x75, 0xb6, 0x5e, 0xb5, 0x25, 0x20, 0xe9,
            0x6b, 0x01, 0xe6, 0x59,
        ];
        assert_eq!(tag, expected);
    }
}
