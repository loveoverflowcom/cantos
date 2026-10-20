//! Provider/HTTP/SQL-free lifetime and rotation decisions. Clock is an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Client {
    Studio,
    TheatreWeb,
    TheatreNative,
}
impl Client {
    pub fn name(self) -> &'static str {
        match self {
            Self::Studio => "cantos-studio-web-bff",
            Self::TheatreWeb => "cantos-theatre-web-bff",
            Self::TheatreNative => "cantos-theatre-native",
        }
    }
    pub fn surface(self) -> &'static str {
        match self {
            Self::Studio => "studio",
            Self::TheatreWeb | Self::TheatreNative => "theatre",
        }
    }
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Studio => "ca1_s_",
            Self::TheatreWeb => "ca1_t_",
            Self::TheatreNative => "ca1_n_",
        }
    }
    pub fn absolute_seconds(self) -> u64 {
        match self {
            Self::Studio => 600,
            Self::TheatreWeb | Self::TheatreNative => 900,
        }
    }
    pub fn idle_seconds(self) -> u64 {
        match self {
            Self::Studio => 300,
            Self::TheatreWeb | Self::TheatreNative => 600,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lifetime {
    pub absolute: u64,
    pub idle: u64,
    pub access: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Inactive,
    Expired,
}
pub fn issue(client: Client, now: u64, proof: u64, provider: u64) -> Result<Lifetime, Refusal> {
    let absolute = proof
        .min(provider)
        .min(now.saturating_add(client.absolute_seconds()));
    renew(client, now, absolute)
}
fn renew(client: Client, now: u64, absolute: u64) -> Result<Lifetime, Refusal> {
    if now >= absolute {
        return Err(Refusal::Expired);
    }
    Ok(Lifetime {
        absolute,
        idle: absolute.min(now.saturating_add(client.idle_seconds())),
        access: absolute.min(now.saturating_add(120)),
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    Refuse(Refusal),
    RevokeFamily,
    Rotate(Lifetime),
}
pub fn rotate(
    client: Client,
    now: u64,
    absolute: u64,
    idle: u64,
    active: bool,
    used: bool,
) -> Rotation {
    if !active {
        return Rotation::Refuse(Refusal::Inactive);
    }
    // A spent secret identifies compromise even after its access/idle lifetime. Never resurrect.
    if used {
        return Rotation::RevokeFamily;
    }
    if now >= idle {
        return Rotation::Refuse(Refusal::Expired);
    }
    match renew(client, now, absolute) {
        Ok(lifetime) => Rotation::Rotate(lifetime),
        Err(error) => Rotation::Refuse(error),
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
        .find(|client| value.strip_prefix(client.prefix()).is_some_and(secret))
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
    fn access_is_short_and_refresh_never_exceeds_initial_authority() {
        for client in [Client::Studio, Client::TheatreWeb, Client::TheatreNative] {
            for proof in [100, 101, 200, 2000] {
                for provider in [100, 101, 150, 2000] {
                    let result = issue(client, 100, proof, provider);
                    if proof == 100 || provider == 100 {
                        assert_eq!(result, Err(Refusal::Expired));
                    } else {
                        let first = result.unwrap();
                        assert_eq!(
                            first.absolute,
                            proof.min(provider).min(100 + client.absolute_seconds())
                        );
                        assert!(
                            first.access <= 220
                                && first.access <= first.idle
                                && first.idle <= first.absolute
                        );
                        if first.idle > 101 {
                            let Rotation::Rotate(next) =
                                rotate(client, 101, first.absolute, first.idle, true, false)
                            else {
                                panic!("renewal refused")
                            };
                            assert_eq!(next.absolute, first.absolute);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn replay_revokes_and_logout_expiry_cannot_be_renewed() {
        for client in [Client::Studio, Client::TheatreWeb, Client::TheatreNative] {
            assert_eq!(
                rotate(client, 100, 900, 600, true, true),
                Rotation::RevokeFamily
            );
            assert_eq!(
                rotate(client, 100, 900, 600, false, true),
                Rotation::Refuse(Refusal::Inactive)
            );
            assert_eq!(
                rotate(client, 600, 900, 600, true, false),
                Rotation::Refuse(Refusal::Expired)
            );
            assert_eq!(
                rotate(client, 900, 900, 900, true, false),
                Rotation::Refuse(Refusal::Expired)
            );
        }
    }
    #[test]
    fn opaque_tokens_are_exact_and_have_distinct_purposes() {
        for client in [Client::Studio, Client::TheatreWeb, Client::TheatreNative] {
            assert_eq!(
                access_client(&format!("{}{}", client.prefix(), "a".repeat(64))),
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
