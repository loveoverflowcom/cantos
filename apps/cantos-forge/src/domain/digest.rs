//! Content digests of exact bytes.

use std::fmt;

use sha2::{Digest as _, Sha256};

const SCHEME: &str = "sha256:";
const HEX_DIGITS: usize = 64;

/// SHA-256 of exact bytes, written canonically as `sha256:<64 lowercase hex digits>`.
///
/// A digest identifies bytes, not their meaning, rights or quality.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha256Digest([u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DigestError {
    /// The text does not start with `sha256:`.
    MissingScheme,
    /// The hexadecimal part is not exactly 64 lowercase digits.
    MalformedHex,
}

impl Sha256Digest {
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub fn parse(text: &str) -> Result<Self, DigestError> {
        let hex = text
            .strip_prefix(SCHEME)
            .ok_or(DigestError::MissingScheme)?;
        if hex.len() != HEX_DIGITS {
            return Err(DigestError::MalformedHex);
        }
        let mut bytes = [0_u8; 32];
        for (byte, pair) in bytes
            .iter_mut()
            .zip(hex.as_bytes().as_chunks::<2>().0.iter())
        {
            let &[high, low] = pair;
            *byte = (lowercase_nibble(high)? << 4) | lowercase_nibble(low)?;
        }
        Ok(Self(bytes))
    }
}

fn lowercase_nibble(digit: u8) -> Result<u8, DigestError> {
    match digit {
        b'0'..=b'9' => Ok(digit - b'0'),
        b'a'..=b'f' => Ok(digit - b'a' + 10),
        _ => Err(DigestError::MalformedHex),
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(SCHEME)?;
        for byte in self.0 {
            write!(out, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // FIPS 180-2 / NIST test vectors, independent of this implementation.
    #[test]
    fn matches_published_sha256_vectors() {
        assert_eq!(
            Sha256Digest::of(b"").to_string(),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            Sha256Digest::of(b"abc").to_string(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn digest_covers_exact_bytes_not_normalized_text() {
        // "Hà" precomposed (NFC) and decomposed (NFD): equal to a reader, different bytes.
        let composed = "H\u{e0}";
        let decomposed = "Ha\u{300}";
        assert_ne!(
            Sha256Digest::of(composed.as_bytes()),
            Sha256Digest::of(decomposed.as_bytes())
        );
    }

    #[test]
    fn non_canonical_text_is_refused() {
        let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let cases = [
            (hex.to_owned(), DigestError::MissingScheme),
            (format!("SHA256:{hex}"), DigestError::MissingScheme),
            (format!("sha512:{hex}"), DigestError::MissingScheme),
            (
                format!("sha256:{}", hex.to_uppercase()),
                DigestError::MalformedHex,
            ),
            (format!("sha256:{}", &hex[1..]), DigestError::MalformedHex),
            (format!("sha256:{hex}0"), DigestError::MalformedHex),
            (format!("sha256:{}g", &hex[1..]), DigestError::MalformedHex),
            ("sha256:".to_owned(), DigestError::MalformedHex),
        ];
        for (text, expected) in cases {
            assert_eq!(Sha256Digest::parse(&text), Err(expected), "{text}");
        }
    }

    proptest! {
        #[test]
        fn display_then_parse_is_identity(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let digest = Sha256Digest::of(&bytes);
            prop_assert_eq!(Sha256Digest::parse(&digest.to_string()), Ok(digest));
        }
    }
}
