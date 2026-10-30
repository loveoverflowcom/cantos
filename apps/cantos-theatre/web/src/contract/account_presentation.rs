//! Display-only data from the authenticated Server projection, never session authority.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AccountPresentation {
    pub version: String,
    pub alias: String,
    pub avatar_seed: String,
}

impl AccountPresentation {
    pub fn valid(&self) -> bool {
        self.version == "generated-alias-v1"
            && !self.alias.trim().is_empty()
            && self.alias.chars().count() <= 64
            && !self.alias.chars().any(char::is_control)
            && self.avatar_seed.len() == 64
            && self
                .avatar_seed
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_ui::{avatar_initials, avatar_tone, AvatarInitials};

    #[test]
    fn server_vectors_have_the_same_unicode_initials_and_palette_on_web() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/theatre/account-presentation/v1/fixtures/vectors.json")).unwrap();
        for vector in vectors.as_array().unwrap() {
            let p: AccountPresentation =
                serde_json::from_value(vector["presentation"].clone()).unwrap();
            assert!(p.valid());
            assert_eq!(
                avatar_initials(&p.alias, AvatarInitials::Pair),
                vector["initials"].as_str().unwrap()
            );
            assert_eq!(
                avatar_tone(&p.avatar_seed).as_str(),
                vector["tone"].as_str().unwrap()
            );
        }
    }

    #[test]
    fn malformed_presentation_is_not_adopted() {
        let p = AccountPresentation {
            version: "generated-alias-v1".into(),
            alias: "Mây Lam".into(),
            avatar_seed: "a".repeat(64),
        };
        for bad in [
            AccountPresentation {
                alias: "".into(),
                ..p.clone()
            },
            AccountPresentation {
                alias: "Mây\nLam".into(),
                ..p.clone()
            },
            AccountPresentation {
                version: "v2".into(),
                ..p.clone()
            },
            AccountPresentation {
                avatar_seed: "actor-id".into(),
                ..p.clone()
            },
        ] {
            assert!(!bad.valid());
        }
    }
}
