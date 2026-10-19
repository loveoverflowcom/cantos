//! The release artifact a Pages deploy consumes: `release-build`'s `manifest.json` plus the Web
//! archive it lists. Nothing is rebuilt. This module decides, from the manifest text and the
//! actual bytes, whether the archive is the one the manifest describes, and extracts it into a
//! run-owned directory one entry at a time with the checks that make an archive a plain static
//! tree.
//!
//! A matching hash proves the bytes did not change after packaging; it does **not** prove where
//! the artifact came from. Source and provenance are carried into the receipt as recorded claims;
//! promotion of a trusted CI artifact is owned by the artifact-contract work (#277).

use std::{
    collections::HashSet,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use serde_json::Value;

use super::super::{bundle, host};
use super::request::Request;

pub const MANIFEST_FILE: &str = "manifest.json";
const MAX_DEPTH: usize = 8;
const MAX_PATH_BYTES: usize = 200;

/// What the manifest claims about the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub component: String,
    pub source_sha: String,
    pub api_origin: String,
    pub archive: ArchiveClaim,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveClaim {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactError {
    Manifest(String),
    ArchiveMismatch(String),
    Unsafe(String),
    Limit(String),
    Io(String),
}

impl std::fmt::Display for ArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manifest(why) => write!(f, "manifest.json refused: {why}"),
            Self::ArchiveMismatch(why) => write!(f, "archive does not match its manifest: {why}"),
            Self::Unsafe(why) => write!(f, "archive entry refused: {why}"),
            Self::Limit(why) => write!(f, "archive exceeds a bound: {why}"),
            Self::Io(why) => write!(f, "{why}"),
        }
    }
}

fn manifest_error(why: impl Into<String>) -> ArtifactError {
    ArtifactError::Manifest(why.into())
}

/// Parse `manifest.json` strictly for the requested app. Every field this deploy depends on must
/// be present and well-formed; a manifest for another component, an unbuilt or already-deployed
/// artifact, or one that lists anything but the single Web archive is refused.
pub fn parse_release(request: &Request, text: &str) -> Result<Release, ArtifactError> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| manifest_error(error.to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| manifest_error("not a JSON object"))?;
    if !matches!(
        object.get("schema_version").and_then(Value::as_u64),
        Some(1 | 2)
    ) {
        return Err(manifest_error("only schema_version 1 and 2 are supported"));
    }
    let component = request.component();
    if object.get("target").and_then(Value::as_str) != Some(component.as_str()) {
        return Err(manifest_error(format!(
            "target is not `{component}`; a manifest for another component is never reused"
        )));
    }
    let source_sha = object
        .get("source_sha")
        .and_then(Value::as_str)
        .filter(|sha| {
            sha.len() == 40 && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
        .ok_or_else(|| manifest_error("source_sha is not a full lowercase commit SHA"))?
        .to_owned();
    if object.get("deployed") != Some(&Value::Bool(false)) {
        return Err(manifest_error(
            "`deployed` must be false for an unpublished artifact",
        ));
    }
    let api_origin = object
        .get("api_origin")
        .and_then(Value::as_str)
        .ok_or_else(|| manifest_error("api_origin is missing"))?;
    let api_origin = host::checked_api_origin(Some(api_origin))
        .map_err(|why| manifest_error(format!("api_origin: {why}")))?;
    let artifacts = object
        .get("artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| manifest_error("artifacts is missing"))?;
    let [entry] = artifacts.as_slice() else {
        return Err(manifest_error(
            "a Web release lists exactly one artifact: its archive",
        ));
    };
    let expected_path = format!("{component}.tar.gz");
    let path = entry.get("path").and_then(Value::as_str);
    if path != Some(expected_path.as_str()) {
        return Err(manifest_error(format!(
            "the artifact is not `{expected_path}`"
        )));
    }
    let bytes = entry
        .get("bytes")
        .and_then(Value::as_u64)
        .filter(|bytes| *bytes > 0)
        .ok_or_else(|| manifest_error("artifact bytes must be a positive integer"))?;
    let sha256 = entry
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|sha| {
            sha.len() == 64 && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
        .ok_or_else(|| manifest_error("artifact sha256 is not 64 lowercase hex characters"))?
        .to_owned();
    Ok(Release {
        component,
        source_sha,
        api_origin,
        archive: ArchiveClaim {
            path: expected_path,
            bytes,
            sha256,
        },
    })
}

/// The actual archive bytes must be exactly what the manifest recorded.
pub fn verify_archive(
    claim: &ArchiveClaim,
    actual_sha256: &str,
    actual_bytes: u64,
) -> Result<(), ArtifactError> {
    if actual_bytes != claim.bytes {
        return Err(ArtifactError::ArchiveMismatch(format!(
            "{} bytes on disk, {} in the manifest",
            actual_bytes, claim.bytes
        )));
    }
    if actual_sha256 != claim.sha256 {
        return Err(ArtifactError::ArchiveMismatch(
            "SHA-256 differs from the manifest".to_owned(),
        ));
    }
    Ok(())
}

/// A normalized relative path made of safe segments, or the reason it is refused.
pub fn safe_relative(path: &Path) -> Result<String, String> {
    let mut segments = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => {
                let text = segment.to_str().ok_or("a non-UTF-8 name")?;
                let safe = !text.is_empty()
                    && !text.starts_with('.')
                    && text
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
                if !safe {
                    return Err(format!("`{text}` is not a plain file name"));
                }
                segments.push(text);
            }
            Component::CurDir => continue,
            Component::ParentDir => return Err("a `..` component".to_owned()),
            Component::RootDir | Component::Prefix(_) => return Err("an absolute path".to_owned()),
        }
    }
    if segments.is_empty() {
        return Err("an empty path".to_owned());
    }
    if segments.len() > MAX_DEPTH {
        return Err(format!("deeper than {MAX_DEPTH} levels"));
    }
    let joined = segments.join("/");
    if joined.len() > MAX_PATH_BYTES {
        return Err(format!("longer than {MAX_PATH_BYTES} bytes"));
    }
    Ok(joined)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    pub files: usize,
    pub bytes: u64,
}

/// Extract a gzip tar into `destination` (an existing, empty, run-owned directory).
///
/// Every entry must be a regular file with a safe, unique relative path; links, directories,
/// devices, extended-attribute headers and absolute or `..` paths are refused before anything is
/// written for them. The decompressed stream, the entry count and the total size are bounded, so
/// a small archive cannot expand without limit. The caller scans the result again afterwards.
pub fn extract(archive: impl Read, destination: &Path) -> Result<Extracted, ArtifactError> {
    let decoder = flate2::read::GzDecoder::new(archive);
    // One byte of slack lets a stream that exceeds the bound be detected rather than truncated.
    let bounded = decoder.take(bundle::MAX_TOTAL_BYTES * 2 + 1);
    let mut tar = tar::Archive::new(bounded);
    tar.set_unpack_xattrs(false);
    tar.set_preserve_permissions(false);
    let entries = tar
        .entries()
        .map_err(|error| ArtifactError::Io(format!("not a readable gzip tar: {error}")))?;
    let mut seen: HashSet<String> = HashSet::new();
    let mut total = 0u64;
    for entry in entries {
        let mut entry =
            entry.map_err(|error| ArtifactError::Io(format!("archive read failed: {error}")))?;
        let kind = entry.header().entry_type();
        if !kind.is_file() {
            return Err(ArtifactError::Unsafe(format!(
                "a {} entry (only regular files are accepted)",
                describe(kind)
            )));
        }
        let raw = entry
            .path()
            .map_err(|error| ArtifactError::Unsafe(error.to_string()))?
            .into_owned();
        let relative = safe_relative(&raw)
            .map_err(|why| ArtifactError::Unsafe(format!("{why} (`{}`)", sanitize(&raw))))?;
        if !seen.insert(relative.to_ascii_lowercase()) {
            return Err(ArtifactError::Unsafe(format!(
                "duplicate entry `{relative}`"
            )));
        }
        if seen.len() > bundle::MAX_FILES {
            return Err(ArtifactError::Limit(format!(
                "more than {} entries",
                bundle::MAX_FILES
            )));
        }
        let declared = entry
            .header()
            .size()
            .map_err(|error| ArtifactError::Unsafe(error.to_string()))?;
        if declared > bundle::MAX_FILE_BYTES {
            return Err(ArtifactError::Limit(format!(
                "`{relative}` is {declared} bytes"
            )));
        }
        total = total.saturating_add(declared);
        if total > bundle::MAX_TOTAL_BYTES {
            return Err(ArtifactError::Limit(format!(
                "more than {} bytes in total",
                bundle::MAX_TOTAL_BYTES
            )));
        }
        let target: PathBuf = destination.join(&relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| ArtifactError::Io(format!("{relative}: {}", error.kind())))?;
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|error| ArtifactError::Io(format!("{relative}: {}", error.kind())))?;
        let copied = std::io::copy(&mut (&mut entry).take(declared), &mut file)
            .map_err(|error| ArtifactError::Io(format!("{relative}: {error}")))?;
        file.flush()
            .map_err(|error| ArtifactError::Io(format!("{relative}: {}", error.kind())))?;
        if copied != declared {
            return Err(ArtifactError::Unsafe(format!(
                "`{relative}` is shorter than its header says"
            )));
        }
    }
    if seen.is_empty() {
        return Err(ArtifactError::Unsafe(
            "the archive has no entries".to_owned(),
        ));
    }
    Ok(Extracted {
        files: seen.len(),
        bytes: total,
    })
}

fn describe(kind: tar::EntryType) -> &'static str {
    if kind.is_dir() {
        "directory"
    } else if kind.is_symlink() {
        "symbolic link"
    } else if kind.is_hard_link() {
        "hard link"
    } else if kind.is_pax_global_extensions() || kind.is_pax_local_extensions() {
        "extended header"
    } else {
        "special"
    }
}

/// A path as text for a message, with control characters removed and a length bound.
fn sanitize(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::{cloudflare::request, request::HostedApp};
    use flate2::{write::GzEncoder, Compression};

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
    const DIGEST: &str = "a3f1c3a2b0c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e";

    fn req() -> Request {
        request::resolve(&request::Raw {
            app: HostedApp::CantosTheatre,
            surface: Some(crate::build::Surface::Web),
            account: "8339afe44b4e8cd2b633a38a77f1db41".into(),
            project: "cantos-theatre".into(),
            branch: "main".into(),
            environment: "production".into(),
            artifact: "a".into(),
            dry_run: true,
        })
        .unwrap()
    }

    fn manifest(mutate: impl FnOnce(&mut Value)) -> String {
        let mut value = serde_json::json!({
            "schema_version": 1, "target": "theatre-web", "source_sha": SHA,
            "api_origin": "https://api.cantos.cc", "deployed": false,
            "artifacts": [{"path": "theatre-web.tar.gz", "bytes": 10, "sha256": DIGEST}]
        });
        mutate(&mut value);
        value.to_string()
    }

    #[test]
    fn a_strict_manifest_for_the_requested_app_is_accepted() {
        let release = parse_release(&req(), &manifest(|_| {})).unwrap();
        assert_eq!(release.component, "theatre-web");
        assert_eq!(release.source_sha, SHA);
        assert_eq!(release.api_origin, "https://api.cantos.cc");
        assert_eq!(release.archive.bytes, 10);
    }

    #[test]
    fn version_two_packager_output_preserves_the_pages_archive_contract() {
        let release = parse_release(
            &req(),
            &manifest(|v| {
                v["schema_version"] = 2.into();
                v["configuration_sha256"] = DIGEST.into();
                v["checkout_sha"] = SHA.into();
            }),
        )
        .unwrap();
        assert_eq!(release, parse_release(&req(), &manifest(|_| {})).unwrap());
        let invalid = manifest(|v| {
            v["schema_version"] = 2.into();
            v["artifacts"][0]["path"] = "studio-web.tar.gz".into();
        });
        assert!(matches!(
            parse_release(&req(), &invalid),
            Err(ArtifactError::Manifest(_))
        ));
    }

    #[test]
    fn every_manifest_field_the_deploy_depends_on_is_enforced() {
        type Mutation = Box<dyn FnOnce(&mut Value)>;
        let cases: Vec<(&str, Mutation)> = vec![
            ("schema", Box::new(|v| v["schema_version"] = 3.into())),
            (
                "another component",
                Box::new(|v| v["target"] = "studio-web".into()),
            ),
            ("short sha", Box::new(|v| v["source_sha"] = "abc".into())),
            (
                "upper sha",
                Box::new(|v| v["source_sha"] = SHA.to_uppercase().into()),
            ),
            ("deployed", Box::new(|v| v["deployed"] = true.into())),
            (
                "no origin",
                Box::new(|v| {
                    v.as_object_mut().unwrap().remove("api_origin");
                }),
            ),
            (
                "http origin",
                Box::new(|v| v["api_origin"] = "http://api.cantos.cc".into()),
            ),
            (
                "origin path",
                Box::new(|v| v["api_origin"] = "https://api.cantos.cc/x".into()),
            ),
            (
                "two artifacts",
                Box::new(|v| {
                    let a = v["artifacts"][0].clone();
                    v["artifacts"].as_array_mut().unwrap().push(a);
                }),
            ),
            (
                "other artifact",
                Box::new(|v| v["artifacts"][0]["path"] = "studio-web.tar.gz".into()),
            ),
            (
                "zero bytes",
                Box::new(|v| v["artifacts"][0]["bytes"] = 0.into()),
            ),
            (
                "bad digest",
                Box::new(|v| v["artifacts"][0]["sha256"] = "xyz".into()),
            ),
        ];
        for (name, mutate) in cases {
            let result = parse_release(&req(), &manifest(mutate));
            assert!(
                matches!(result, Err(ArtifactError::Manifest(_))),
                "{name}: {result:?}"
            );
        }
        assert!(parse_release(&req(), "[]").is_err());
        assert!(parse_release(&req(), "not json").is_err());
    }

    #[test]
    fn archive_bytes_must_equal_the_manifest_size_and_digest() {
        let claim = ArchiveClaim {
            path: "theatre-web.tar.gz".into(),
            bytes: 10,
            sha256: DIGEST.into(),
        };
        assert_eq!(verify_archive(&claim, DIGEST, 10), Ok(()));
        assert!(matches!(
            verify_archive(&claim, DIGEST, 11),
            Err(ArtifactError::ArchiveMismatch(_))
        ));
        assert!(matches!(
            verify_archive(&claim, "b", 10),
            Err(ArtifactError::ArchiveMismatch(_))
        ));
    }

    #[test]
    fn relative_paths_refuse_escape_hidden_names_and_odd_characters() {
        assert_eq!(
            safe_relative(Path::new("assets/a-1.js")).unwrap(),
            "assets/a-1.js"
        );
        assert_eq!(
            safe_relative(Path::new("./index.html")).unwrap(),
            "index.html"
        );
        for bad in [
            "/etc/passwd",
            "../x",
            "a/../../x",
            ".env",
            "a/.hidden",
            "a b",
            "a\nb",
            "é.js",
            "",
            "a;b",
        ] {
            assert!(safe_relative(Path::new(bad)).is_err(), "{bad:?}");
        }
        assert!(safe_relative(Path::new("a/b/c/d/e/f/g/h/i.js")).is_err());
        assert!(safe_relative(Path::new(&"a".repeat(MAX_PATH_BYTES + 1))).is_err());
    }

    fn gz(build: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        build(&mut builder);
        let tarball = builder.into_inner().unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tarball).unwrap();
        encoder.finish().unwrap()
    }

    fn file(builder: &mut tar::Builder<Vec<u8>>, name: &str, body: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, body).unwrap();
    }

    /// A header whose name bypasses the builder's own refusal of `..` and absolute paths.
    fn raw_entry(
        builder: &mut tar::Builder<Vec<u8>>,
        name: &str,
        kind: tar::EntryType,
        link: Option<&str>,
    ) {
        let mut header = tar::Header::new_old();
        header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        if let Some(link) = link {
            header.as_old_mut().linkname[..link.len()].copy_from_slice(link.as_bytes());
        }
        header.set_entry_type(kind);
        header.set_size(0);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, std::io::empty()).unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("cantos-cf-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_plain_static_tree_is_extracted_byte_for_byte() {
        let archive = gz(|b| {
            file(b, "index.html", b"<html></html>");
            file(b, "assets/app.js", b"js");
        });
        let dir = scratch("ok");
        let done = extract(archive.as_slice(), &dir).unwrap();
        assert_eq!(
            done,
            Extracted {
                files: 2,
                bytes: 15
            }
        );
        assert_eq!(std::fs::read(dir.join("assets/app.js")).unwrap(), b"js");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn traversal_absolute_links_directories_and_duplicates_write_nothing_outside_or_at_all() {
        let escape = scratch("escape-parent");
        let cases: Vec<(&str, Vec<u8>)> = vec![
            (
                "dotdot",
                gz(|b| raw_entry(b, "../evil.js", tar::EntryType::Regular, None)),
            ),
            (
                "absolute",
                gz(|b| raw_entry(b, "/tmp/evil.js", tar::EntryType::Regular, None)),
            ),
            (
                "symlink",
                gz(|b| raw_entry(b, "link.js", tar::EntryType::Symlink, Some("/etc/passwd"))),
            ),
            (
                "hardlink",
                gz(|b| {
                    file(b, "a.js", b"x");
                    raw_entry(b, "b.js", tar::EntryType::Link, Some("a.js"))
                }),
            ),
            (
                "directory",
                gz(|b| raw_entry(b, "assets/", tar::EntryType::Directory, None)),
            ),
            (
                "duplicate",
                gz(|b| {
                    file(b, "a.js", b"x");
                    file(b, "a.js", b"y")
                }),
            ),
            (
                "case duplicate",
                gz(|b| {
                    file(b, "a.js", b"x");
                    file(b, "A.js", b"y")
                }),
            ),
            ("hidden", gz(|b| file(b, ".env", b"x"))),
            ("empty archive", gz(|_| {})),
        ];
        for (name, archive) in cases {
            let dir = scratch(name.replace(' ', "-").as_str());
            let result = extract(archive.as_slice(), &dir);
            assert!(
                matches!(result, Err(ArtifactError::Unsafe(_))),
                "{name}: {result:?}"
            );
            assert!(!escape.join("evil.js").exists());
            assert!(!std::path::Path::new("/tmp/evil.js").exists(), "{name}");
            std::fs::remove_dir_all(dir).unwrap();
        }
        std::fs::remove_dir_all(escape).unwrap();
    }

    #[test]
    fn an_existing_file_is_never_overwritten_and_a_non_archive_is_refused() {
        let dir = scratch("existing");
        std::fs::write(dir.join("index.html"), "keep").unwrap();
        let archive = gz(|b| file(b, "index.html", b"new"));
        assert!(matches!(
            extract(archive.as_slice(), &dir),
            Err(ArtifactError::Io(_))
        ));
        assert_eq!(std::fs::read(dir.join("index.html")).unwrap(), b"keep");
        assert!(extract(&b"not gzip"[..], &dir).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
