//! Bounded human-readable labels.
//!
//! Each label is its own type so that, for example, a model label cannot be recorded where a
//! named human reviewer is required. A label is a declared claim, never an authenticated
//! identity.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelError {
    Empty,
    TooLong {
        max_chars: usize,
    },
    /// Leading or trailing whitespace would make two spellings of one label look identical.
    SurroundingWhitespace,
    ControlCharacter,
}

fn validate(text: &str, max_chars: usize) -> Result<(), LabelError> {
    if text.is_empty() {
        return Err(LabelError::Empty);
    }
    if text.chars().count() > max_chars {
        return Err(LabelError::TooLong { max_chars });
    }
    if text.trim() != text {
        return Err(LabelError::SurroundingWhitespace);
    }
    if text.chars().any(char::is_control) {
        return Err(LabelError::ControlCharacter);
    }
    Ok(())
}

macro_rules! label {
    ($(#[$meta:meta])* $name:ident, max_chars = $max:expr) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub const MAX_CHARS: usize = $max;

            pub fn parse(text: &str) -> Result<Self, LabelError> {
                validate(text, Self::MAX_CHARS).map(|()| Self(text.to_owned()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                out.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(out, "{}({:?})", stringify!($name), self.0)
            }
        }
    };
}

label!(
    /// Who or what produced an artifact through Forge: an AI host session or a person.
    ProducerLabel,
    max_chars = 128
);
label!(
    /// A reference supporting a rights claim (licence, permission note). Not verified rights.
    ClaimReference,
    max_chars = 512
);
label!(
    /// The deterministic check or measurement that produced evidence.
    CheckName,
    max_chars = 128
);
label!(
    /// The model and configuration that produced an AI review. An assessment, never approval.
    ModelLabel,
    max_chars = 128
);
label!(
    /// The named person who read or listened.
    ReviewerName,
    max_chars = 128
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_vietnamese_names_up_to_the_character_limit() {
        assert_eq!(
            ReviewerName::parse("Nguyễn Thị Hồng Nhung")
                .unwrap()
                .as_str(),
            "Nguyễn Thị Hồng Nhung"
        );
        // 128 multi-byte characters are within a 128-character limit.
        let at_limit = "ệ".repeat(128);
        assert!(ProducerLabel::parse(&at_limit).is_ok());
    }

    #[test]
    fn refuses_each_malformed_label_with_its_exact_reason() {
        let cases = [
            (String::new(), LabelError::Empty),
            ("ệ".repeat(129), LabelError::TooLong { max_chars: 128 }),
            (" Lan".to_owned(), LabelError::SurroundingWhitespace),
            ("Lan\u{a0}".to_owned(), LabelError::SurroundingWhitespace),
            ("Lan\tAnh".to_owned(), LabelError::ControlCharacter),
            ("Lan\u{1b}[31m".to_owned(), LabelError::ControlCharacter),
        ];
        for (text, expected) in cases {
            assert_eq!(ReviewerName::parse(&text), Err(expected), "{text:?}");
        }
    }

    #[test]
    fn each_label_has_its_own_limit() {
        let text = "x".repeat(129);
        assert_eq!(
            CheckName::parse(&text),
            Err(LabelError::TooLong { max_chars: 128 })
        );
        assert!(ClaimReference::parse(&text).is_ok());
        assert_eq!(
            ClaimReference::parse(&"x".repeat(513)),
            Err(LabelError::TooLong { max_chars: 512 })
        );
    }
}
