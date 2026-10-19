//! Validation of the exact public bytes a deploy would upload, as pure functions over a [`Scan`]
//! the host produced. Nothing here touches the file system: the host walks the run-owned `dist`
//! once (never following a symlink), hashes every file, keeps the first bytes of each and the
//! whole `index.html`; this module decides whether those facts describe a deployable bundle of the
//! selected app and derives a manifest and digest from them.
//!
//! What "deployable" means here, each point a refusal rather than a warning:
//! - the bundle is **only** regular files of an allow-listed static type, none hidden, none named
//!   like a credential, receipt or Firebase configuration, within local size bounds;
//! - `index.html` is for the **selected app** (its title and its `*.js`/`*_bg.wasm` names), and no
//!   artifact of the other app is present;
//! - every local reference in `index.html` (attributes and inline module imports) exists, stays
//!   inside the bundle, and where it carries `integrity="sha384-..."` matches the actual bytes;
//! - no `.js`/`.css`/`.wasm`/data file is really an HTML page, and every `.wasm` has the WebAssembly
//!   magic. A path that answers `200 index.html` is exactly how a missing asset hides.

#[cfg(test)]
use sha2::Sha384;
use sha2::{Digest, Sha256};

use crate::{config::App, run_files::hex};

// Local sanity bounds, not Hosting service limits.
// ACCEPTANCE(NOT_RUN): the Hosting service's own per-version file and size limits were not reached
// by a real upload. Owner: the operator of the first live preview deploy.
pub const MAX_FILES: usize = 1_000;
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
pub const INDEX_LIMIT: u64 = 256 * 1024;
/// How many leading bytes of each file the scan keeps for type checks.
pub const HEAD_BYTES: usize = 64;

const ALLOWED_EXTENSIONS: [&str; 15] = [
    "html", "js", "wasm", "css", "json", "svg", "png", "webp", "jpg", "jpeg", "gif", "ico", "woff",
    "woff2", "txt",
];
/// Names that are valid JSON but never public content of a Cantos web bundle.
const PRIVATE_NAMES: [&str; 5] = [
    "firebase.json",
    "receipt.json",
    "intent.json",
    "package.json",
    "package-lock.json",
];

/// Which provider the validated bytes are for. It decides only whether the provider's own
/// root-level policy files may be part of the inventory; every other rule is shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Firebase,
    /// Cloudflare Pages reads `_headers`, `_redirects` and `404.html` from the root of the
    /// uploaded directory. Only those exact root-level names are allowed, never other
    /// extensionless files.
    Pages,
}

/// Root-level files that configure Pages delivery rather than being product assets.
pub const PAGES_POLICY_FILES: [&str; 3] = ["_headers", "_redirects", "404.html"];

/// Names Pages treats as server code or routing for Functions. A static Web release never
/// contains them, and uploading one could make Pages run code instead of serving files.
const PAGES_SERVER_ROOTS: [&str; 3] = ["_worker.js", "_routes.json", "functions"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    /// Base64 of the SHA-384 digest, the form `integrity="sha384-..."` uses.
    pub sha384: String,
    pub head: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Irregular {
    Symlink,
    /// A device, socket, FIFO or other non-regular entry.
    Special,
    Unreadable(String),
    NonUtf8Name,
    TooDeep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Irregularity {
    pub path: String,
    pub kind: Irregular,
}

/// What the host observed. `index_html` is `None` when absent or when it exceeded
/// [`INDEX_LIMIT`] (`index_oversized`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    pub files: Vec<FileFacts>,
    pub irregular: Vec<Irregularity>,
    pub index_html: Option<Vec<u8>>,
    pub index_oversized: bool,
    /// The walk stopped after more than [`MAX_FILES`] entries.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    Irregular(Irregularity),
    TooManyFiles,
    TotalTooLarge(u64),
    FileTooLarge(String),
    EmptyFile(String),
    HiddenFile(String),
    PrivateName(String),
    UnexpectedType(String),
    NoIndex,
    IndexOversized,
    IndexNotUtf8,
    /// The `<title>` of the selected app is absent: the output may belong to another app.
    WrongApp {
        expected: &'static str,
    },
    ForeignArtifact {
        path: String,
        app: &'static str,
    },
    UnsafeReference(String),
    MissingReference(String),
    IntegrityMismatch(String),
    /// `index.html` does not load a WebAssembly module and a script named for the selected app.
    NoAppEntry {
        missing: &'static str,
        prefix: &'static str,
    },
    /// A non-HTML file whose bytes are an HTML page.
    HtmlMasquerade(String),
    BadWasm(String),
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Finding::Irregular(item) => write!(f, "`{}` is {}", item.path, describe(&item.kind)),
            Finding::TooManyFiles => write!(f, "more than {MAX_FILES} entries"),
            Finding::TotalTooLarge(bytes) => {
                write!(f, "{bytes} bytes in total exceeds the {MAX_TOTAL_BYTES} byte bound")
            }
            Finding::FileTooLarge(path) => {
                write!(f, "`{path}` exceeds the {MAX_FILE_BYTES} byte file bound")
            }
            Finding::EmptyFile(path) => write!(f, "`{path}` is empty"),
            Finding::HiddenFile(path) => write!(f, "`{path}` is hidden (dot-files are never public)"),
            Finding::PrivateName(path) => write!(
                f,
                "`{path}` is named like configuration or a record, not public content"
            ),
            Finding::UnexpectedType(path) => write!(
                f,
                "`{path}` is not an allow-listed static web file type (maps, logs, keys, audio and unknown types are never uploaded)"
            ),
            Finding::NoIndex => f.write_str("no non-empty `index.html` at the bundle root"),
            Finding::IndexOversized => {
                write!(f, "`index.html` exceeds the {INDEX_LIMIT} byte bound")
            }
            Finding::IndexNotUtf8 => f.write_str("`index.html` is not UTF-8"),
            Finding::WrongApp { expected } => write!(
                f,
                "`index.html` lacks {expected}: the output may belong to another app"
            ),
            Finding::ForeignArtifact { path, app } => {
                write!(f, "`{path}` belongs to {app}, not the selected app")
            }
            Finding::UnsafeReference(reference) => write!(
                f,
                "`index.html` references `{reference}`, which escapes the bundle or is not a plain path"
            ),
            Finding::MissingReference(path) => {
                write!(f, "`index.html` references `{path}`, which is not in the bundle")
            }
            Finding::IntegrityMismatch(path) => write!(
                f,
                "`{path}` does not match the integrity digest `index.html` pins for it"
            ),
            Finding::NoAppEntry { missing, prefix } => write!(
                f,
                "`index.html` loads no {missing} named `{prefix}-*`: it is not this app's entry page"
            ),
            Finding::HtmlMasquerade(path) => {
                write!(f, "`{path}` is an HTML page, not the asset its name claims")
            }
            Finding::BadWasm(path) => {
                write!(f, "`{path}` does not start with the WebAssembly magic bytes")
            }
        }
    }
}

fn describe(kind: &Irregular) -> String {
    match kind {
        Irregular::Symlink => "a symbolic link (links are never followed or uploaded)".to_owned(),
        Irregular::Special => "not a regular file".to_owned(),
        Irregular::Unreadable(why) => format!("unreadable ({why})"),
        Irregular::NonUtf8Name => "not named in UTF-8".to_owned(),
        Irregular::TooDeep => "nested too deeply".to_owned(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// The identity of a validated bundle: every file's path, size and SHA-256 in path order, and one
/// digest over all of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub files: Vec<ManifestFile>,
    pub total_bytes: u64,
    pub digest: String,
    /// The script, WebAssembly and stylesheet files `index.html` loads, in path order.
    pub entry_assets: Vec<String>,
}

pub fn validate(app: App, scan: &Scan) -> Result<Manifest, Vec<Finding>> {
    validate_for(app, scan, Provider::Firebase)
}

pub fn validate_for(app: App, scan: &Scan, provider: Provider) -> Result<Manifest, Vec<Finding>> {
    let mut findings = Vec::new();
    findings.extend(scan.irregular.iter().cloned().map(Finding::Irregular));
    if scan.truncated || scan.files.len() > MAX_FILES {
        findings.push(Finding::TooManyFiles);
    }
    let total: u64 = scan.files.iter().map(|file| file.size).sum();
    if total > MAX_TOTAL_BYTES {
        findings.push(Finding::TotalTooLarge(total));
    }
    for file in &scan.files {
        check_file(app, provider, file, &mut findings);
    }
    let entry_assets = check_index(app, scan, &mut findings);
    if !findings.is_empty() {
        return Err(findings);
    }
    let mut files: Vec<ManifestFile> = scan
        .files
        .iter()
        .map(|file| ManifestFile {
            path: file.path.clone(),
            size: file.size,
            sha256: file.sha256.clone(),
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut digest = Sha256::new();
    for file in &files {
        digest.update(file.path.as_bytes());
        digest.update([0]);
        digest.update(file.size.to_string().as_bytes());
        digest.update([0]);
        digest.update(file.sha256.as_bytes());
        digest.update(b"\n");
    }
    Ok(Manifest {
        total_bytes: total,
        digest: hex(&digest.finalize()),
        files,
        entry_assets,
    })
}

fn check_file(app: App, provider: Provider, file: &FileFacts, findings: &mut Vec<Finding>) {
    let path = file.path.as_str();
    if provider == Provider::Pages && PAGES_POLICY_FILES.contains(&path) {
        if file.size == 0 {
            findings.push(Finding::EmptyFile(path.to_owned()));
        }
        if file.size > MAX_FILE_BYTES {
            findings.push(Finding::FileTooLarge(path.to_owned()));
        }
        return;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    if provider == Provider::Pages
        && path
            .split('/')
            .next()
            .is_some_and(|root| PAGES_SERVER_ROOTS.contains(&root))
    {
        findings.push(Finding::PrivateName(path.to_owned()));
        return;
    }
    if path.split('/').any(|segment| segment.starts_with('.')) {
        findings.push(Finding::HiddenFile(path.to_owned()));
        return;
    }
    if PRIVATE_NAMES.contains(&name.to_ascii_lowercase().as_str())
        || path.split('/').any(|segment| segment == "node_modules")
    {
        findings.push(Finding::PrivateName(path.to_owned()));
        return;
    }
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    let Some(extension) = extension.filter(|e| ALLOWED_EXTENSIONS.contains(&e.as_str())) else {
        findings.push(Finding::UnexpectedType(path.to_owned()));
        return;
    };
    if extension == "html" && path != "index.html" {
        findings.push(Finding::UnexpectedType(path.to_owned()));
        return;
    }
    if file.size == 0 {
        findings.push(Finding::EmptyFile(path.to_owned()));
    }
    if file.size > MAX_FILE_BYTES {
        findings.push(Finding::FileTooLarge(path.to_owned()));
    }
    if let Some(other) = foreign_app(app, name) {
        findings.push(Finding::ForeignArtifact {
            path: path.to_owned(),
            app: other,
        });
    }
    if extension == "wasm" && !file.head.starts_with(b"\0asm") {
        findings.push(Finding::BadWasm(path.to_owned()));
    }
    let may_be_markup = matches!(extension.as_str(), "html" | "svg");
    if !may_be_markup && looks_like_html(&file.head) {
        findings.push(Finding::HtmlMasquerade(path.to_owned()));
    }
}

/// The other app's Trunk artifact names (`cantos-<app>-<hash>.js` / `_bg.wasm`).
fn foreign_app(app: App, name: &str) -> Option<&'static str> {
    let other = App::ALL.into_iter().find(|candidate| *candidate != app)?;
    let prefix = format!("{}-", other.web_package());
    (name.starts_with(&prefix) && (name.ends_with(".js") || name.ends_with(".wasm")))
        .then_some(other.cli_name())
}

fn looks_like_html(head: &[u8]) -> bool {
    let head = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    let start = head
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(head.len());
    let lower: Vec<u8> = head[start..].iter().map(u8::to_ascii_lowercase).collect();
    lower.starts_with(b"<!doctype html") || lower.starts_with(b"<html")
}

fn check_index(app: App, scan: &Scan, findings: &mut Vec<Finding>) -> Vec<String> {
    if scan.index_oversized {
        findings.push(Finding::IndexOversized);
        return Vec::new();
    }
    let index = scan.files.iter().find(|file| file.path == "index.html");
    let (Some(index), Some(bytes)) = (index, scan.index_html.as_deref()) else {
        findings.push(Finding::NoIndex);
        return Vec::new();
    };
    if index.size == 0 {
        // Reported as `EmptyFile` by the per-file check; there is nothing to parse.
        findings.push(Finding::NoIndex);
        return Vec::new();
    }
    let Ok(html) = std::str::from_utf8(bytes) else {
        findings.push(Finding::IndexNotUtf8);
        return Vec::new();
    };
    if !html.contains(app.web_marker()) {
        findings.push(Finding::WrongApp {
            expected: app.web_marker(),
        });
    }
    let mut entry_assets = Vec::new();
    for reference in references(html) {
        let path = match normalize(&reference.target) {
            Normalized::Ignored => continue,
            Normalized::Unsafe => {
                findings.push(Finding::UnsafeReference(reference.target));
                continue;
            }
            Normalized::Local(path) => path,
        };
        let Some(file) = scan.files.iter().find(|file| file.path == path) else {
            findings.push(Finding::MissingReference(path));
            continue;
        };
        if let Some(pinned) = &reference.integrity {
            if !integrity_accepts(pinned, &file.sha384) {
                findings.push(Finding::IntegrityMismatch(path.clone()));
            }
        }
        if matches!(extension_of(&path), "js" | "wasm" | "css") && !entry_assets.contains(&path) {
            entry_assets.push(path);
        }
    }
    entry_assets.sort();
    let prefix = app.web_package();
    for (extension, missing) in [("wasm", "WebAssembly module"), ("js", "script")] {
        let loaded = entry_assets.iter().any(|path| {
            extension_of(path) == extension
                && path.rsplit('/').next().is_some_and(|name| {
                    name.strip_prefix(prefix)
                        .is_some_and(|rest| rest.starts_with('-'))
                })
        });
        if !loaded {
            findings.push(Finding::NoAppEntry { missing, prefix });
        }
    }
    entry_assets
}

fn extension_of(path: &str) -> &str {
    path.rsplit_once('.').map_or("", |(_, extension)| extension)
}

/// An `integrity` attribute lists space-separated `<algorithm>-<base64>[?options]` tokens. Only
/// SHA-384 (what Trunk emits) can be checked here; a token of another algorithm is left to the
/// browser. When a SHA-384 token is present at least one must equal the real digest.
fn integrity_accepts(attribute: &str, actual_sha384: &str) -> bool {
    let tokens: Vec<&str> = attribute
        .split_ascii_whitespace()
        .filter_map(|token| token.strip_prefix("sha384-"))
        .map(|token| token.split('?').next().unwrap_or(token))
        .collect();
    tokens.is_empty() || tokens.contains(&actual_sha384)
}

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                text.push(ALPHABET[((group >> shift) & 63) as usize] as char);
            } else {
                text.push('=');
            }
        }
    }
    text
}

#[cfg(test)]
pub fn sha384_base64(bytes: &[u8]) -> String {
    base64(&Sha384::digest(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub target: String,
    pub integrity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Normalized {
    /// Empty, a fragment, or a URL with a scheme or authority: not served from the bundle.
    Ignored,
    Local(String),
    Unsafe,
}

/// A bundle-relative path for a reference, or why it cannot be one.
pub fn normalize(reference: &str) -> Normalized {
    let reference = reference.trim();
    if reference.is_empty() || reference.starts_with('#') || reference.starts_with("//") {
        return Normalized::Ignored;
    }
    let scheme_end = reference.find(|c: char| !(c.is_ascii_alphanumeric() || "+-.".contains(c)));
    if let Some(end) = scheme_end {
        if end > 0 && reference[end..].starts_with(':') {
            return Normalized::Ignored;
        }
    }
    let path = reference
        .split(['?', '#'])
        .next()
        .unwrap_or(reference)
        .trim_start_matches('/');
    if path.contains(['\\', '\0', '%']) {
        return Normalized::Unsafe;
    }
    let mut segments = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => continue,
            ".." => return Normalized::Unsafe,
            other => segments.push(other),
        }
    }
    if segments.is_empty() {
        return Normalized::Ignored;
    }
    Normalized::Local(segments.join("/"))
}

const WATCHED_TAGS: [&str; 8] = [
    "link", "script", "img", "source", "audio", "video", "iframe", "embed",
];
const INLINE_ASSET_EXTENSIONS: [&str; 5] = ["js", "wasm", "css", "json", "woff2"];

/// Every local asset reference of an HTML document: the `href`/`src` of resource-loading tags,
/// with the tag's `integrity`, and root-relative asset strings inside inline `<script>` bodies
/// (Trunk's module bootstrap imports its script and WebAssembly module that way).
pub fn references(html: &str) -> Vec<Reference> {
    let bytes = html.as_bytes();
    let mut found: Vec<Reference> = Vec::new();
    let mut position = 0;
    while let Some(offset) = html[position..].find('<') {
        let open = position + offset;
        if html[open..].starts_with("<!--") {
            position = html[open..]
                .find("-->")
                .map_or(html.len(), |end| open + end + 3);
            continue;
        }
        let Some(close) = tag_end(bytes, open) else {
            break;
        };
        let tag = &html[open + 1..close];
        let name_end = tag
            .find(|c: char| c.is_ascii_whitespace() || c == '/')
            .unwrap_or(tag.len());
        let name = tag[..name_end].to_ascii_lowercase();
        position = close + 1;
        if WATCHED_TAGS.contains(&name.as_str()) {
            let attributes = attributes(&tag[name_end..]);
            let integrity = attributes
                .iter()
                .find(|(key, _)| key == "integrity")
                .map(|(_, value)| value.clone());
            for (key, value) in &attributes {
                if key == "href" || key == "src" {
                    push(&mut found, value.clone(), integrity.clone());
                }
            }
        }
        if name == "script" {
            let end = html[position..]
                .to_ascii_lowercase()
                .find("</script")
                .map_or(html.len(), |end| position + end);
            for value in quoted_strings(&html[position..end]) {
                if inline_asset(&value) {
                    push(&mut found, value, None);
                }
            }
            position = end;
        }
    }
    found
}

fn push(found: &mut Vec<Reference>, target: String, integrity: Option<String>) {
    if let Some(existing) = found.iter_mut().find(|r| r.target == target) {
        if integrity.is_none() {
            return;
        }
        if existing.integrity.is_none() {
            existing.integrity = integrity;
            return;
        }
    }
    let reference = Reference { target, integrity };
    // Several tags can load the same URL with different integrity attributes. Each pin must
    // agree with its bytes, even if a preceding tag already supplied a valid pin for this URL.
    if !found.contains(&reference) {
        found.push(reference);
    }
}

/// Index of the `>` that closes the tag opened at `open`, ignoring a `>` inside a quoted value.
fn tag_end(bytes: &[u8], open: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (offset, byte) in bytes[open + 1..].iter().enumerate() {
        match (quote, *byte) {
            (Some(q), b) if b == q => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(*byte),
            (None, b'>') => return Some(open + 1 + offset),
            _ => {}
        }
    }
    None
}

fn attributes(text: &str) -> Vec<(String, String)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        let start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'/'
        {
            i += 1;
        }
        if start == i {
            break;
        }
        let key = text[start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let value = match bytes.get(i) {
                Some(q @ (b'"' | b'\'')) => {
                    let begin = i + 1;
                    let end = text[begin..]
                        .find(*q as char)
                        .map_or(text.len(), |offset| begin + offset);
                    i = (end + 1).min(text.len());
                    text[begin..end].to_owned()
                }
                _ => {
                    let begin = i;
                    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    text[begin..i].to_owned()
                }
            };
            out.push((key, value));
        } else {
            out.push((key, String::new()));
        }
    }
    out
}

fn quoted_strings(script: &str) -> Vec<String> {
    let bytes = script.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if matches!(bytes[i], b'"' | b'\'') {
            let quote = bytes[i];
            let begin = i + 1;
            let mut end = begin;
            while end < bytes.len() && bytes[end] != quote && bytes[end] != b'\n' {
                end += 1;
            }
            if end < bytes.len() && bytes[end] == quote {
                out.push(script[begin..end].to_owned());
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// A root-relative path to a script, WebAssembly, stylesheet, JSON or font file.
fn inline_asset(value: &str) -> bool {
    if !(value.starts_with('/') && !value.starts_with("//")) && !value.starts_with("./") {
        return false;
    }
    let path = value.split(['?', '#']).next().unwrap_or(value);
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| INLINE_ASSET_EXTENSIONS.contains(&extension))
}

/// Scan builders shared by this module's tests and the coordinator's.
#[cfg(test)]
pub mod testing {
    use super::*;

    pub const WASM: &[u8] = b"\0asm\x01\0\0\0body";
    pub const JS: &[u8] = b"export default function init(){}";
    pub const CSS: &[u8] = b"body{margin:0}";

    pub fn file(path: &str, bytes: &[u8]) -> FileFacts {
        FileFacts {
            path: path.to_owned(),
            size: bytes.len() as u64,
            sha256: hex(&Sha256::digest(bytes)),
            sha384: sha384_base64(bytes),
            head: bytes.iter().take(HEAD_BYTES).copied().collect(),
        }
    }

    /// A well-formed bundle of `app` whose index pins every asset by integrity.
    pub fn good_scan(app: App) -> Scan {
        let package = app.web_package();
        let js = file(&format!("{package}-aaaa.js"), JS);
        let wasm = file(&format!("{package}-aaaa_bg.wasm"), WASM);
        let css = file("tokens-bbbb.css", CSS);
        let index = format!(
            r#"<!doctype html><html><head>{}
            <link rel="stylesheet" href="/tokens-bbbb.css" integrity="sha384-{}"/>
            <script type="module">
            import init, * as bindings from '/{package}-aaaa.js';
            const wasm = await init({{ module_or_path: '/{package}-aaaa_bg.wasm' }});
            </script>
            <link rel="modulepreload" href="/{package}-aaaa.js" integrity="sha384-{}">
            <link rel="preload" href="/{package}-aaaa_bg.wasm" integrity="sha384-{}" as="fetch" type="application/wasm">
            </head><body></body></html>"#,
            app.web_marker(),
            css.sha384,
            js.sha384,
            wasm.sha384
        );
        Scan {
            files: vec![
                file("index.html", index.as_bytes()),
                js,
                wasm,
                css,
                file(
                    "assets/icons/home.svg",
                    b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
                ),
            ],
            index_html: Some(index.into_bytes()),
            ..Scan::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    const REAL_THEATRE_INDEX: &str =
        include_str!("../../fixtures/hosting-bundles/theatre-index.html");
    const REAL_STUDIO_INDEX: &str =
        include_str!("../../fixtures/hosting-bundles/studio-index.html");

    fn good_theatre() -> Scan {
        good_scan(App::Theatre)
    }

    fn replace(scan: &mut Scan, path: &str, bytes: &[u8]) {
        let index = scan.files.iter().position(|f| f.path == path).unwrap();
        scan.files[index] = file(path, bytes);
        if path == "index.html" {
            scan.index_html = Some(bytes.to_vec());
        }
    }

    fn findings(app: App, scan: &Scan) -> Vec<Finding> {
        validate(app, scan).unwrap_err()
    }

    #[test]
    fn a_well_formed_bundle_yields_a_stable_path_ordered_manifest() {
        let scan = good_theatre();
        let manifest = validate(App::Theatre, &scan).unwrap();
        let paths: Vec<_> = manifest.files.iter().map(|f| f.path.as_str()).collect();
        let mut sorted = paths.clone();
        sorted.sort_unstable();
        assert_eq!(paths, sorted);
        assert_eq!(
            manifest.total_bytes,
            scan.files.iter().map(|f| f.size).sum::<u64>()
        );
        assert_eq!(
            manifest.entry_assets,
            [
                "cantos-theatre-aaaa.js",
                "cantos-theatre-aaaa_bg.wasm",
                "tokens-bbbb.css"
            ]
        );
        // The digest depends on the files and not on the order the walk produced them.
        let mut shuffled = scan.clone();
        shuffled.files.reverse();
        assert_eq!(
            validate(App::Theatre, &shuffled).unwrap().digest,
            manifest.digest
        );
        // ...and changes when any byte does.
        let mut changed = scan;
        replace(&mut changed, "assets/icons/home.svg", b"<svg/>");
        assert_ne!(
            validate(App::Theatre, &changed).unwrap().digest,
            manifest.digest
        );
    }

    #[test]
    fn the_bundle_must_belong_to_the_selected_app() {
        let scan = good_theatre();
        let found = findings(App::Studio, &scan);
        assert!(found.contains(&Finding::WrongApp {
            expected: "<title>Cantos Studio</title>"
        }));
        assert!(found.contains(&Finding::ForeignArtifact {
            path: "cantos-theatre-aaaa.js".into(),
            app: "cantos-theatre"
        }));
        assert!(found.iter().any(|f| matches!(
            f,
            Finding::NoAppEntry {
                prefix: "cantos-studio",
                ..
            }
        )));
        // A leftover artifact of the other app inside the right app's bundle is refused too.
        let mut mixed = good_theatre();
        mixed.files.push(file("cantos-studio-cccc_bg.wasm", WASM));
        assert!(
            findings(App::Theatre, &mixed).contains(&Finding::ForeignArtifact {
                path: "cantos-studio-cccc_bg.wasm".into(),
                app: "cantos-studio"
            })
        );
    }

    #[test]
    fn a_missing_or_empty_index_is_refused() {
        let mut scan = good_theatre();
        scan.files.retain(|f| f.path != "index.html");
        scan.index_html = None;
        assert!(findings(App::Theatre, &scan).contains(&Finding::NoIndex));
        let mut empty = good_theatre();
        replace(&mut empty, "index.html", b"");
        let found = findings(App::Theatre, &empty);
        assert!(found.contains(&Finding::EmptyFile("index.html".into())));
        assert!(found.contains(&Finding::NoIndex));
        let mut oversized = good_theatre();
        oversized.index_oversized = true;
        oversized.index_html = None;
        assert!(findings(App::Theatre, &oversized).contains(&Finding::IndexOversized));
        let mut binary = good_theatre();
        replace(&mut binary, "index.html", &[0xFF, 0xFE, 0xFD]);
        assert!(findings(App::Theatre, &binary).contains(&Finding::IndexNotUtf8));
    }

    #[test]
    fn every_local_reference_must_exist_and_stay_inside_the_bundle() {
        let mut scan = good_theatre();
        scan.files.retain(|f| f.path != "tokens-bbbb.css");
        assert!(findings(App::Theatre, &scan)
            .contains(&Finding::MissingReference("tokens-bbbb.css".into())));
        for hostile in [
            "/../secret.js",
            "../secret.js",
            "/a/../../b.css",
            "/a%2e%2e/b.css",
            "/a\\b.css",
        ] {
            let mut scan = good_theatre();
            let html = format!(
                "<!doctype html><title>Cantos</title><link rel=stylesheet href=\"{hostile}\"><script>import '/cantos-theatre-aaaa.js';'/cantos-theatre-aaaa_bg.wasm'</script>"
            );
            replace(&mut scan, "index.html", html.as_bytes());
            assert!(
                findings(App::Theatre, &scan).contains(&Finding::UnsafeReference(hostile.into())),
                "{hostile}"
            );
        }
    }

    #[test]
    fn an_integrity_pin_that_disagrees_with_the_bytes_is_refused() {
        let mut scan = good_theatre();
        replace(&mut scan, "tokens-bbbb.css", b"body{margin:1px}");
        assert_eq!(
            findings(App::Theatre, &scan),
            [Finding::IntegrityMismatch("tokens-bbbb.css".into())]
        );
        // The same file may be pinned by several tags; one wrong pin is enough to refuse.
        let mut js = good_theatre();
        replace(
            &mut js,
            "cantos-theatre-aaaa.js",
            b"export default function init(){ /* x */ }",
        );
        assert_eq!(
            findings(App::Theatre, &js),
            [Finding::IntegrityMismatch("cantos-theatre-aaaa.js".into())]
        );
    }

    #[test]
    fn a_valid_first_integrity_pin_cannot_hide_a_later_invalid_pin_for_the_same_url() {
        for invalid_first in [false, true] {
            let mut scan = good_theatre();
            let original = String::from_utf8(scan.index_html.clone().unwrap()).unwrap();
            let bad =
                "<script src=\"/cantos-theatre-aaaa.js\" integrity=\"sha384-invalid\"></script>";
            let html = if invalid_first {
                format!("{bad}{original}")
            } else {
                format!("{original}{bad}")
            };
            replace(&mut scan, "index.html", html.as_bytes());
            assert_eq!(
                findings(App::Theatre, &scan),
                [Finding::IntegrityMismatch("cantos-theatre-aaaa.js".into())],
                "invalid_first={invalid_first}"
            );
        }
    }

    #[test]
    fn html_cannot_masquerade_as_an_asset_or_wasm() {
        for (name, body) in [
            ("a.js", &b"<!DOCTYPE html><html>"[..]),
            ("b.css", b"  \n<html lang=en>"),
            ("c.json", b"\xEF\xBB\xBF<!doctype html>"),
            ("d.woff2", b"<HTML>"),
        ] {
            let mut scan = good_theatre();
            scan.files.push(file(name, body));
            assert!(
                findings(App::Theatre, &scan).contains(&Finding::HtmlMasquerade(name.into())),
                "{name}"
            );
        }
        let mut wasm = good_theatre();
        wasm.files.push(file("other.wasm", b"<!doctype html>"));
        let found = findings(App::Theatre, &wasm);
        assert!(found.contains(&Finding::BadWasm("other.wasm".into())));
        assert!(found.contains(&Finding::HtmlMasquerade("other.wasm".into())));
        // SVG is markup by nature and is not an HTML page.
        assert!(validate(App::Theatre, &good_theatre()).is_ok());
    }

    #[test]
    fn only_regular_allow_listed_visible_non_private_files_are_public() {
        for (name, finding) in [
            (".env", Finding::HiddenFile(".env".into())),
            (
                "assets/.DS_Store",
                Finding::HiddenFile("assets/.DS_Store".into()),
            ),
            (".git/config", Finding::HiddenFile(".git/config".into())),
            (
                "firebase.json",
                Finding::PrivateName("firebase.json".into()),
            ),
            ("receipt.json", Finding::PrivateName("receipt.json".into())),
            (
                "node_modules/lib/app.js",
                Finding::PrivateName("node_modules/lib/app.js".into()),
            ),
            (
                "assets/node_modules/app.js",
                Finding::PrivateName("assets/node_modules/app.js".into()),
            ),
            (
                "sub/package.json",
                Finding::PrivateName("sub/package.json".into()),
            ),
            (
                "Firebase.JSON",
                Finding::PrivateName("Firebase.JSON".into()),
            ),
            ("app.js.map", Finding::UnexpectedType("app.js.map".into())),
            ("debug.log", Finding::UnexpectedType("debug.log".into())),
            ("key.pem", Finding::UnexpectedType("key.pem".into())),
            ("take1.wav", Finding::UnexpectedType("take1.wav".into())),
            (
                "episode.opus",
                Finding::UnexpectedType("episode.opus".into()),
            ),
            ("script.sh", Finding::UnexpectedType("script.sh".into())),
            ("noextension", Finding::UnexpectedType("noextension".into())),
            ("other.html", Finding::UnexpectedType("other.html".into())),
        ] {
            let mut scan = good_theatre();
            scan.files.push(file(name, b"x"));
            assert!(findings(App::Theatre, &scan).contains(&finding), "{name}");
        }
        // Known static types stay, in any case of extension.
        let mut scan = good_theatre();
        scan.files.push(file("assets/Font.WOFF2", b"wOF2"));
        scan.files.push(file("assets/OFL.txt", b"licence"));
        assert!(validate(App::Theatre, &scan).is_ok());
    }

    #[test]
    fn symlinks_special_files_unreadable_entries_and_oversized_trees_are_refused() {
        for kind in [
            Irregular::Symlink,
            Irregular::Special,
            Irregular::Unreadable("PermissionDenied".into()),
            Irregular::NonUtf8Name,
            Irregular::TooDeep,
        ] {
            let mut scan = good_theatre();
            let item = Irregularity {
                path: "assets/link".into(),
                kind,
            };
            scan.irregular.push(item.clone());
            assert!(findings(App::Theatre, &scan).contains(&Finding::Irregular(item)));
        }
        let mut many = good_theatre();
        many.truncated = true;
        assert!(findings(App::Theatre, &many).contains(&Finding::TooManyFiles));
        let mut over = good_theatre();
        for index in 0..MAX_FILES {
            over.files.push(file(&format!("a/{index}.txt"), b"x"));
        }
        assert!(findings(App::Theatre, &over).contains(&Finding::TooManyFiles));
        let mut big = good_theatre();
        big.files.push(FileFacts {
            size: MAX_FILE_BYTES + 1,
            ..file("big.png", b"\x89PNG")
        });
        let found = findings(App::Theatre, &big);
        assert!(found.contains(&Finding::FileTooLarge("big.png".into())));
        let mut total = good_theatre();
        for index in 0..5 {
            total.files.push(FileFacts {
                size: MAX_FILE_BYTES,
                ..file(&format!("part{index}.png"), b"\x89PNG")
            });
        }
        assert!(findings(App::Theatre, &total)
            .iter()
            .any(|f| matches!(f, Finding::TotalTooLarge(_))));
        let mut empty = good_theatre();
        empty.files.push(file("zero.png", b""));
        assert!(findings(App::Theatre, &empty).contains(&Finding::EmptyFile("zero.png".into())));
    }

    #[test]
    fn findings_name_their_path_and_never_echo_file_content() {
        let mut scan = good_theatre();
        scan.files.push(file(".env", b"SECRET_TOKEN=hunter2"));
        let text: Vec<String> = findings(App::Theatre, &scan)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(text.iter().any(|line| line.contains(".env")));
        assert!(text.iter().all(|line| !line.contains("hunter2")));
    }

    #[test]
    fn references_are_read_from_attributes_and_inline_module_imports() {
        let found = references(REAL_THEATRE_INDEX);
        let targets: Vec<&str> = found.iter().map(|r| r.target.as_str()).collect();
        for expected in [
            "/tokens-3708958437b0fef6.css",
            "/foundation-951b963d9814b6c.css",
            "/theatre-ca2c9510ca313bc0.css",
            "/cantos-theatre-a34b29a2a671bedf.js",
            "/cantos-theatre-a34b29a2a671bedf_bg.wasm",
        ] {
            assert!(targets.contains(&expected), "{expected} in {targets:?}");
        }
        assert_eq!(targets.len(), 5, "{targets:?}");
        // Pinned by integrity on the tags; the inline imports are the same files.
        let wasm = found
            .iter()
            .find(|r| r.target.ends_with("_bg.wasm"))
            .unwrap();
        assert!(wasm.integrity.as_deref().unwrap().starts_with("sha384-"));
        let studio: Vec<_> = references(REAL_STUDIO_INDEX)
            .into_iter()
            .map(|r| r.target)
            .collect();
        assert!(studio.contains(&"/cantos-studio-11df3230f3449e82.js".to_owned()));
        assert!(studio.contains(&"/artwork-1bff59dd25ac9b52.css".to_owned()));
        assert_eq!(studio.len(), 6, "{studio:?}");
    }

    #[test]
    fn comments_external_urls_and_non_loading_tags_are_not_bundle_references() {
        let html = r#"<!-- <link href="/commented.css"> -->
            <a href="/api/v1/x">api</a>
            <link rel=stylesheet href="https://cdn.example/x.css">
            <link rel=stylesheet href="//cdn.example/y.css">
            <img src="data:image/png;base64,AAAA">
            <script>const t = "text/css"; const u = "/api/v1/theatre/works?limit=20"; const w = '/real.js';</script>"#;
        let kept: Vec<_> = references(html)
            .into_iter()
            .filter(|r| !matches!(normalize(&r.target), Normalized::Ignored))
            .map(|r| r.target)
            .collect();
        assert_eq!(kept, ["/real.js"]);
    }

    #[test]
    fn reference_paths_normalize_to_bundle_relative_files() {
        assert_eq!(normalize("/a.js"), Normalized::Local("a.js".into()));
        assert_eq!(
            normalize("./a/b.css?v=1#x"),
            Normalized::Local("a/b.css".into())
        );
        assert_eq!(
            normalize("a//b/./c.js"),
            Normalized::Local("a/b/c.js".into())
        );
        for ignored in [
            "",
            "#top",
            "https://x/y.js",
            "data:text/plain,x",
            "//cdn/x.js",
            "mailto:a@b.c",
        ] {
            assert_eq!(normalize(ignored), Normalized::Ignored, "{ignored}");
        }
        for hostile in ["..", "/../x", "a/../../x", "a%2fb", "a\\b", "a\0b"] {
            assert_eq!(normalize(hostile), Normalized::Unsafe, "{hostile:?}");
        }
    }

    #[test]
    fn base64_and_sha384_match_the_published_vectors() {
        for (input, expected) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input), expected);
        }
        assert_eq!(
            sha384_base64(b"abc"),
            "ywB1P0WjXou1oD1pmsZQBycsMqsO3tFjGotgWkP/W+2AhgcroefMI1i67KE0yCWn"
        );
        assert_eq!(
            sha384_base64(b"hello"),
            "WeF0h3dEjGnea4ANejO7+5/xtGPkQ1TDVTvNucZm+pASWjx5+QOXvfX2oT3oKGhP"
        );
    }

    #[test]
    fn integrity_accepts_any_matching_sha384_token_and_defers_other_algorithms() {
        assert!(integrity_accepts("sha384-AAA sha384-BBB", "BBB"));
        assert!(!integrity_accepts("sha384-AAA", "BBB"));
        assert!(integrity_accepts("sha512-xxxx", "BBB"));
        assert!(integrity_accepts("", "BBB"));
        assert!(integrity_accepts("sha384-BBB?ext", "BBB"));
    }
}
