//! Bounded password input and the stored-credential (PHC) text boundary.
//!
//! A password is never trimmed, truncated, case-folded or Unicode-normalized: the exact bytes
//! the user typed are what the KDF consumes. Only length is checked, before any KDF work.

use std::fmt;

use zeroize::Zeroizing;

pub const MIN_PASSWORD_CHARS: usize = 8;
pub const MAX_PASSWORD_CHARS: usize = 128;
pub const MAX_PASSWORD_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordRejected {
    TooShort,
    TooLong,
}

impl fmt::Display for PasswordRejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooShort => "password is shorter than the minimum length",
            Self::TooLong => "password is longer than the maximum length",
        })
    }
}

impl std::error::Error for PasswordRejected {}

/// A candidate password. The buffer is zeroized on drop, never printed and not cloneable.
/// Copies made inside the KDF library are outside this type's control.
///
/// ```compile_fail
/// use cantos_identity::domain::password::PasswordInput;
/// let input = PasswordInput::new("a long enough one").unwrap();
/// let copy = input.clone();
/// ```
pub struct PasswordInput(Zeroizing<String>);

impl PasswordInput {
    /// `Err` for a length outside policy. The policy is intentionally length-only: composition
    /// rules and password-list screening are separate owner decisions.
    pub fn new(text: &str) -> Result<Self, PasswordRejected> {
        if text.len() > MAX_PASSWORD_BYTES {
            return Err(PasswordRejected::TooLong);
        }
        let scalars = text.chars().count();
        if scalars < MIN_PASSWORD_CHARS {
            return Err(PasswordRejected::TooShort);
        }
        if scalars > MAX_PASSWORD_CHARS {
            return Err(PasswordRejected::TooLong);
        }
        Ok(Self(Zeroizing::new(text.to_owned())))
    }

    /// Login input is bounded above only: an existing account may predate a stricter minimum,
    /// and a short guess must not exit before the KDF runs. It never creates a credential.
    pub fn for_verification(text: &str) -> Result<Self, PasswordRejected> {
        if text.is_empty() || text.len() > MAX_PASSWORD_BYTES {
            return Err(if text.is_empty() {
                PasswordRejected::TooShort
            } else {
                PasswordRejected::TooLong
            });
        }
        if text.chars().count() > MAX_PASSWORD_CHARS {
            return Err(PasswordRejected::TooLong);
        }
        Ok(Self(Zeroizing::new(text.to_owned())))
    }

    pub(crate) fn expose(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for PasswordInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PasswordInput([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_is_counted_in_scalars_and_bytes_with_exact_boundaries() {
        assert_eq!(
            PasswordInput::new("1234567").err(),
            Some(PasswordRejected::TooShort)
        );
        assert!(PasswordInput::new("12345678").is_ok());
        assert!(PasswordInput::new(&"x".repeat(128)).is_ok());
        assert_eq!(
            PasswordInput::new(&"x".repeat(129)).err(),
            Some(PasswordRejected::TooLong)
        );
        // 8 scalars of 3 bytes each: counted as characters, not bytes.
        assert!(PasswordInput::new("ềềềềềềềề").is_ok());
        // 128 four-byte scalars is exactly 512 bytes: both ceilings are reached together.
        assert!(PasswordInput::new(&"😀".repeat(128)).is_ok());
        assert_eq!(
            PasswordInput::new(&"😀".repeat(129)).err(),
            Some(PasswordRejected::TooLong)
        );
    }

    #[test]
    fn spaces_case_and_unicode_form_are_preserved_exactly() {
        let composed = PasswordInput::new("  Mật khẩu  ").unwrap();
        assert_eq!(composed.expose(), "  Mật khẩu  ".as_bytes());
        let decomposed = PasswordInput::new("Ma\u{323}\u{302}t khau\u{309}  ").unwrap();
        assert_ne!(
            PasswordInput::new("Mật khẩu  ").unwrap().expose(),
            decomposed.expose()
        );
    }

    #[test]
    fn verification_input_has_no_minimum_but_keeps_the_ceiling() {
        assert!(PasswordInput::for_verification("x").is_ok());
        assert_eq!(
            PasswordInput::for_verification("").err(),
            Some(PasswordRejected::TooShort)
        );
        assert_eq!(
            PasswordInput::for_verification(&"y".repeat(513)).err(),
            Some(PasswordRejected::TooLong)
        );
    }

    #[test]
    fn debug_is_redacted() {
        let input = PasswordInput::new("a very secret one").unwrap();
        assert_eq!(format!("{input:?}"), "PasswordInput([REDACTED])");
    }
}
