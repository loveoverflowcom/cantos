//! Paths that stay inside a producer workspace.

use std::{
    fmt,
    path::{Path, PathBuf},
};

const MAX_PATH_BYTES: usize = 1024;
const MAX_COMPONENT_BYTES: usize = 255;

/// A `/`-separated path relative to a workspace root that cannot lexically name anything
/// outside it: no absolute or drive-qualified form, no `.`/`..`, no empty component, no
/// backslash, colon or control character. The accepted text is already canonical.
///
/// This is a lexical barrier only. Symbolic links, hard links and case- or
/// normalization-insensitive file systems are the filesystem adapter's responsibility.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspacePath(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspacePathError {
    Empty,
    TooLong {
        max_bytes: usize,
    },
    Absolute,
    /// `\` is refused so that one text means the same path on every platform.
    Backslash,
    /// `:` is refused: it introduces Windows drives and alternate data streams.
    Colon,
    ControlCharacter,
    /// Leading, trailing or doubled `/`.
    EmptyComponent,
    CurrentDirectory,
    ParentDirectory,
    ComponentTooLong {
        max_bytes: usize,
    },
}

impl WorkspacePath {
    pub fn parse(text: &str) -> Result<Self, WorkspacePathError> {
        if text.is_empty() {
            return Err(WorkspacePathError::Empty);
        }
        if text.len() > MAX_PATH_BYTES {
            return Err(WorkspacePathError::TooLong {
                max_bytes: MAX_PATH_BYTES,
            });
        }
        if text.starts_with('/') {
            return Err(WorkspacePathError::Absolute);
        }
        if text.contains('\\') {
            return Err(WorkspacePathError::Backslash);
        }
        if text.contains(':') {
            return Err(WorkspacePathError::Colon);
        }
        if text.chars().any(char::is_control) {
            return Err(WorkspacePathError::ControlCharacter);
        }
        for component in text.split('/') {
            match component {
                "" => return Err(WorkspacePathError::EmptyComponent),
                "." => return Err(WorkspacePathError::CurrentDirectory),
                ".." => return Err(WorkspacePathError::ParentDirectory),
                _ if component.len() > MAX_COMPONENT_BYTES => {
                    return Err(WorkspacePathError::ComponentTooLong {
                        max_bytes: MAX_COMPONENT_BYTES,
                    })
                }
                _ => {}
            }
        }
        Ok(Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The native path below `root`. Lexically it never leaves `root`.
    pub fn under(&self, root: &Path) -> PathBuf {
        self.0
            .split('/')
            .fold(root.to_path_buf(), |path, component| path.join(component))
    }
}

impl fmt::Display for WorkspacePath {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl fmt::Debug for WorkspacePath {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "WorkspacePath({:?})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::Component;

    #[test]
    fn accepts_ordinary_relative_paths_including_vietnamese_names() {
        for text in [
            "raw/chuong-01.md",
            "score/Mot loi hen/v2.json",
            "bản thảo/chương một.md",
            ".hidden/notes.txt",
            "a..b/c...d",
        ] {
            assert_eq!(WorkspacePath::parse(text).unwrap().as_str(), text);
        }
    }

    #[test]
    fn refuses_every_escape_and_ambiguous_form_with_its_exact_reason() {
        let long_component = "a".repeat(256);
        let long_path = ["a"; 513].join("/");
        let cases = [
            ("", WorkspacePathError::Empty),
            (
                long_path.as_str(),
                WorkspacePathError::TooLong { max_bytes: 1024 },
            ),
            ("/etc/passwd", WorkspacePathError::Absolute),
            ("//server/share", WorkspacePathError::Absolute),
            ("..\\secrets", WorkspacePathError::Backslash),
            ("raw\\file.md", WorkspacePathError::Backslash),
            ("C:/Windows", WorkspacePathError::Colon),
            ("file.md:stream", WorkspacePathError::Colon),
            ("raw/\u{0}file", WorkspacePathError::ControlCharacter),
            ("raw/\nfile", WorkspacePathError::ControlCharacter),
            ("raw/", WorkspacePathError::EmptyComponent),
            ("raw//file.md", WorkspacePathError::EmptyComponent),
            (".", WorkspacePathError::CurrentDirectory),
            ("./raw", WorkspacePathError::CurrentDirectory),
            ("..", WorkspacePathError::ParentDirectory),
            ("../outside", WorkspacePathError::ParentDirectory),
            ("raw/../../outside", WorkspacePathError::ParentDirectory),
            ("raw/..", WorkspacePathError::ParentDirectory),
            (
                long_component.as_str(),
                WorkspacePathError::ComponentTooLong { max_bytes: 255 },
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(WorkspacePath::parse(text), Err(expected), "{text:?}");
        }
    }

    #[test]
    fn limits_are_inclusive() {
        let component = "a".repeat(255);
        assert!(WorkspacePath::parse(&component).is_ok());
        // 255 + 1 + 255 + 1 + 255 + 1 + 254 + 1 + 1 = 1024 bytes.
        let path = [component.as_str(); 3].join("/") + "/" + &"a".repeat(254) + "/a";
        assert_eq!(path.len(), 1024);
        assert!(WorkspacePath::parse(&path).is_ok());
        assert_eq!(
            WorkspacePath::parse(&(path + "a")),
            Err(WorkspacePathError::TooLong { max_bytes: 1024 })
        );
    }

    fn path_text() -> impl Strategy<Value = String> {
        let component = prop_oneof![
            Just("..".to_owned()),
            Just(".".to_owned()),
            Just(String::new()),
            "[a-zA-Z0-9 ._:\\\\-]{1,12}",
            "\\PC{1,8}",
        ];
        (any::<bool>(), proptest::collection::vec(component, 1..6)).prop_map(|(absolute, parts)| {
            let joined = parts.join("/");
            if absolute {
                format!("/{joined}")
            } else {
                joined
            }
        })
    }

    proptest! {
        #[test]
        fn an_accepted_path_never_leaves_its_root(text in path_text()) {
            if let Ok(path) = WorkspacePath::parse(&text) {
                let root = Path::new("/workspace/root");
                let native = path.under(root);
                prop_assert!(native.starts_with(root));
                let below = native.strip_prefix(root).unwrap();
                prop_assert!(below.components().all(|part| matches!(part, Component::Normal(_))));
                prop_assert_eq!(below.components().count(), text.split('/').count());
            } else {
                // A refused text is one a reader could resolve outside or ambiguously.
                let unsafe_text = text.is_empty()
                    || text.starts_with('/')
                    || text.contains('\\')
                    || text.contains(':')
                    || text.chars().any(char::is_control)
                    || text.split('/').any(|part| matches!(part, "" | "." | ".."));
                prop_assert!(unsafe_text, "refused a safe path: {:?}", text);
            }
        }

        #[test]
        fn display_then_parse_is_identity(text in path_text()) {
            if let Ok(path) = WorkspacePath::parse(&text) {
                prop_assert_eq!(WorkspacePath::parse(&path.to_string()), Ok(path));
            }
        }
    }
}
