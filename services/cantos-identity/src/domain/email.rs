//! Checked mailbox address with one canonical spelling.
//!
//! Policy (see the Identity README decision table): ASCII only, no trimming, no quoted local
//! parts and no internationalized addresses. The domain is lowercased; the local part is
//! preserved exactly. There is deliberately no provider-specific rule: no dot removal, no
//! `+tag` stripping and no whole-address lowercasing. Two spellings that differ in local-part
//! case are two distinct addresses, each needing its own mailbox proof. Parsing proves syntax
//! only: it says nothing about deliverability or ownership.

use std::fmt;

pub const MAX_EMAIL_BYTES: usize = 254;
const MAX_LOCAL_BYTES: usize = 64;
const MAX_DOMAIN_BYTES: usize = 253;
const MAX_LABEL_BYTES: usize = 63;

/// Why an address was refused. Carries no input text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmailRejected {
    Length,
    NotAscii,
    Shape,
    LocalPart,
    Domain,
}

impl fmt::Display for EmailRejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Length => "email address length is outside the accepted range",
            Self::NotAscii => "email address must be ASCII",
            Self::Shape => "email address must have exactly one local part and one domain",
            Self::LocalPart => "email local part is not accepted",
            Self::Domain => "email domain is not accepted",
        })
    }
}

impl std::error::Error for EmailRejected {}

/// Canonical address. The inner string is private so every value passed the checks below.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EmailAddress(String);

impl EmailAddress {
    pub fn parse(input: &str) -> Result<Self, EmailRejected> {
        if input.len() < 3 || input.len() > MAX_EMAIL_BYTES {
            return Err(EmailRejected::Length);
        }
        if !input.is_ascii() {
            return Err(EmailRejected::NotAscii);
        }
        let (local, domain) = input.split_once('@').ok_or(EmailRejected::Shape)?;
        if domain.contains('@') {
            return Err(EmailRejected::Shape);
        }
        if local.is_empty()
            || local.len() > MAX_LOCAL_BYTES
            || local.starts_with('.')
            || local.ends_with('.')
            || local.contains("..")
            || !local.bytes().all(local_byte)
        {
            return Err(EmailRejected::LocalPart);
        }
        if domain.len() > MAX_DOMAIN_BYTES || !valid_domain(domain) {
            return Err(EmailRejected::Domain);
        }
        Ok(Self(format!("{local}@{}", domain.to_ascii_lowercase())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn domain(&self) -> &str {
        self.0.rsplit_once('@').map_or("", |(_, domain)| domain)
    }
}

fn local_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-/=?^_`{|}~.".contains(&byte)
}

fn valid_domain(domain: &str) -> bool {
    let mut labels = domain.split('.').peekable();
    let mut count = 0;
    while let Some(label) = labels.next() {
        count += 1;
        let ok = !label.is_empty()
            && label.len() <= MAX_LABEL_BYTES
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if !ok {
            return false;
        }
        // An all-numeric last label would make `1.2.3.4` read as an address literal.
        if labels.peek().is_none() && label.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    count >= 2
}

impl fmt::Debug for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EmailAddress([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_form_lowercases_only_the_domain_and_never_rewrites_the_local_part() {
        for (input, canonical) in [
            ("User@Example.COM", "User@example.com"),
            (
                "first.last+tag@Mail.Example.org",
                "first.last+tag@mail.example.org",
            ),
            ("a.b.c@x1.vn", "a.b.c@x1.vn"),
            ("UPPER@x.io", "UPPER@x.io"),
            ("o'brien@example.com", "o'brien@example.com"),
            ("a@xn--bcher-kva.example", "a@xn--bcher-kva.example"),
        ] {
            let parsed = EmailAddress::parse(input).unwrap();
            assert_eq!(parsed.as_str(), canonical, "{input}");
            assert_eq!(EmailAddress::parse(canonical).unwrap(), parsed);
        }
        assert_ne!(
            EmailAddress::parse("A@x.io").unwrap(),
            EmailAddress::parse("a@x.io").unwrap()
        );
        assert_ne!(
            EmailAddress::parse("a.b@x.io").unwrap(),
            EmailAddress::parse("ab@x.io").unwrap()
        );
        assert_ne!(
            EmailAddress::parse("a+x@x.io").unwrap(),
            EmailAddress::parse("a@x.io").unwrap()
        );
    }

    #[test]
    fn ambiguous_or_injectable_addresses_are_refused_without_trimming() {
        for (input, reason) in [
            ("", EmailRejected::Length),
            ("a@", EmailRejected::Length),
            ("@x.io", EmailRejected::LocalPart),
            ("noat.example.com", EmailRejected::Shape),
            ("a@b@x.io", EmailRejected::Shape),
            (" a@x.io", EmailRejected::LocalPart),
            ("a@x.io ", EmailRejected::Domain),
            ("a@x.io\r\nBcc: v@x.io", EmailRejected::Shape),
            ("a@x.io\r\nBcc: v", EmailRejected::Domain),
            ("a\r\n@x.io", EmailRejected::LocalPart),
            (".a@x.io", EmailRejected::LocalPart),
            ("a.@x.io", EmailRejected::LocalPart),
            ("a..b@x.io", EmailRejected::LocalPart),
            ("\"quoted\"@x.io", EmailRejected::LocalPart),
            ("a b@x.io", EmailRejected::LocalPart),
            ("a(c)@x.io", EmailRejected::LocalPart),
            ("a@localhost", EmailRejected::Domain),
            ("a@-x.io", EmailRejected::Domain),
            ("a@x-.io", EmailRejected::Domain),
            ("a@x..io", EmailRejected::Domain),
            ("a@.x.io", EmailRejected::Domain),
            ("a@x.io.", EmailRejected::Domain),
            ("a@x_y.io", EmailRejected::Domain),
            ("a@1.2.3.4", EmailRejected::Domain),
            ("a@[127.0.0.1]", EmailRejected::Domain),
            ("ngườidùng@x.io", EmailRejected::NotAscii),
            ("a@máy.vn", EmailRejected::NotAscii),
        ] {
            assert_eq!(EmailAddress::parse(input), Err(reason), "{input:?}");
        }
    }

    #[test]
    fn length_limits_are_exact() {
        let local_64 = "a".repeat(64);
        assert!(EmailAddress::parse(&format!("{local_64}@x.io")).is_ok());
        assert_eq!(
            EmailAddress::parse(&format!("{}@x.io", "a".repeat(65))),
            Err(EmailRejected::LocalPart)
        );
        let label_63 = "b".repeat(63);
        assert!(EmailAddress::parse(&format!("a@{label_63}.io")).is_ok());
        assert_eq!(
            EmailAddress::parse(&format!("a@{}.io", "b".repeat(64))),
            Err(EmailRejected::Domain)
        );
        let long = format!(
            "a@{}.{}.{}.io",
            "c".repeat(63),
            "d".repeat(63),
            "e".repeat(63)
        );
        assert!(long.len() < MAX_EMAIL_BYTES && EmailAddress::parse(&long).is_ok());
        let too_long = format!(
            "{local_64}@{}.{}.{}.io",
            "c".repeat(63),
            "d".repeat(63),
            "e".repeat(58)
        );
        assert_eq!(too_long.len(), 254);
        assert!(EmailAddress::parse(&too_long).is_ok());
        assert_eq!(
            EmailAddress::parse(&format!("{too_long}x")),
            Err(EmailRejected::Length)
        );
    }

    #[test]
    fn debug_and_errors_never_echo_the_address() {
        let parsed = EmailAddress::parse("secret.person@example.com").unwrap();
        assert_eq!(format!("{parsed:?}"), "EmailAddress([REDACTED])");
        let error = EmailAddress::parse("secret person@example.com").unwrap_err();
        assert!(!format!("{error:?} {error}").contains("secret"));
        assert_eq!(parsed.domain(), "example.com");
    }
}
