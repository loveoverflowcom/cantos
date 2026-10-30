//! Versioned display pseudonyms, never authentication facts or public account identifiers.
use sha2::{Digest, Sha256};

pub const VERSION: &str = "generated-alias-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedAlias {
    pub alias: String,
    pub avatar_seed: String,
}

// V1 is immutable: changing the ordering or spelling would rename existing listeners.
const FIRST: [&str; 16] = [
    "Mây", "Trăng", "Lá", "Gió", "Sao", "Sóng", "Suối", "Nắng", "Mưa", "Sương", "Rừng", "Đồi",
    "Biển", "Cát", "Hoa", "Tre",
];
const LAST: [&str; 16] = [
    "Lam", "Non", "Sớm", "Êm", "Nhẹ", "Trong", "Xanh", "Vàng", "Hiền", "Mát", "Xuân", "Thu",
    "Hồng", "Mai", "Bạc", "Thanh",
];

/// No storage or signup side effect: the same durable actor survives replay and restart.
pub fn for_actor(actor: &str) -> GeneratedAlias {
    let digest = Sha256::digest([b"cantos:avatar-alias:v1".as_slice(), actor.as_bytes()].concat());
    let seed = Sha256::digest([b"cantos:avatar-seed:v1".as_slice(), actor.as_bytes()].concat());
    GeneratedAlias {
        alias: format!(
            "{} {}",
            FIRST[usize::from(digest[0]) % 16],
            LAST[usize::from(digest[1]) % 16]
        ),
        avatar_seed: seed.iter().map(|byte| format!("{byte:02x}")).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::identity::domain::ActorId;

    #[test]
    fn generated_identity_matches_independent_golden_vectors_and_replays() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/theatre/account-presentation/v1/fixtures/vectors.json"
        ))
        .unwrap();
        for vector in vectors.as_array().unwrap() {
            let actor = ActorId::new(vector["actor_id"].as_str().unwrap()).unwrap();
            let expected = GeneratedAlias {
                alias: vector["presentation"]["alias"].as_str().unwrap().to_owned(),
                avatar_seed: vector["presentation"]["avatar_seed"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            };
            assert_eq!(VERSION, vector["presentation"]["version"].as_str().unwrap());
            assert_eq!(for_actor(actor.as_str()), expected);
            assert_eq!(
                for_actor(ActorId::new(actor.as_str()).unwrap().as_str()),
                expected
            );
            assert!(!expected.alias.contains(actor.as_str()));
            assert!(!expected.avatar_seed.contains(actor.as_str()));
        }
    }
}
