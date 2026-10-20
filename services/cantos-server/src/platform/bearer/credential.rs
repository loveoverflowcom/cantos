//! Opaque credential routing and protocol syntax; tags never establish authority.
use super::client::Client;

pub fn access_prefix(client: Client) -> &'static str {
    match client {
        Client::Studio => "ca1_s_",
        Client::TheatreWeb => "ca1_t_",
        Client::TheatreNative => "ca1_n_",
    }
}

pub fn secret(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}
pub fn access_client(value: &str) -> Option<Client> {
    [Client::Studio, Client::TheatreWeb, Client::TheatreNative]
        .into_iter()
        .find(|client| {
            value
                .strip_prefix(access_prefix(*client))
                .is_some_and(secret)
        })
}
pub fn refresh_secret(value: &str) -> bool {
    value.strip_prefix("cr1_").is_some_and(secret)
}
pub fn protocol(value: &str) -> bool {
    (43..=128).contains(&value.len())
        && value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"-._~".contains(&v))
}
pub fn challenge(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"-_".contains(&v))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_tokens_are_exact_and_have_distinct_purposes() {
        for (client, prefix) in [
            (Client::Studio, "ca1_s_"),
            (Client::TheatreWeb, "ca1_t_"),
            (Client::TheatreNative, "ca1_n_"),
        ] {
            assert_eq!(access_prefix(client), prefix);
            assert_eq!(
                access_client(&format!("{prefix}{}", "a".repeat(64))),
                Some(client)
            );
        }
        for value in [
            "",
            "Bearer aaa",
            "fake.jwt.token",
            &"A".repeat(64),
            &format!("ca1_s_{}", "a".repeat(65)),
            &format!("cr1_{}", "a".repeat(64)),
        ] {
            assert_eq!(access_client(value), None);
        }
        assert!(refresh_secret(&format!("cr1_{}", "a".repeat(64))));
        assert!(!refresh_secret(&format!("ca1_s_{}", "a".repeat(64))));
        assert!(protocol(&"~".repeat(43)));
        assert!(!protocol(&" ".repeat(43)));
        assert!(!challenge(&"~".repeat(43)));
        assert!(challenge(&"_".repeat(43)));
    }
}
