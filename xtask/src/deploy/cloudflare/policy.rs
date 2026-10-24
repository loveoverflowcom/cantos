//! The tracked Pages delivery policy (`infra/cloudflare/<target>/`) and the operator's private
//! account/project mapping, validated as pure functions over their text.
//!
//! The policy is three small files that Pages reads from the root of the uploaded directory:
//! `_headers` (security and cache headers), `404.html` and an optional `_redirects`. They are
//! owned here, validated before staging and become part of the final bundle inventory, so the
//! bytes that are hashed, smoked and uploaded are the bytes that were reviewed.
//!
//! Pages serves a single-page-application fallback (`index.html` with status 200 for every
//! unknown path) **unless a top-level `404.html` exists**. Cantos routes with URL fragments, so
//! no path needs a fallback; the mandatory `404.html` is what keeps `/api/*` and a missing
//! `.js`/`.wasm`/`.css` from answering `200 text/html`.
//!
//! Theatre may add its Android App Links and iOS Universal Links association files under
//! `.well-known/` (ADR 0048). Each claims exactly the release app and the signup email action
//! path `/verify-email`; a debug application, another path or another association is refused.

use std::collections::BTreeMap;

use serde::Deserialize;

use serde_json::Value;

use super::request::{AccountId, Destination};
use crate::{
    config::{App, THEATRE_VERIFY_EMAIL_PATH as VERIFY_EMAIL_PATH},
    run_files::sha256_hex,
};

pub const MAPPING_FILE: &str = "infra/cloudflare/projects.json";
pub const MAPPING_EXAMPLE: &str = "infra/cloudflare/projects.example.json";
pub const POLICY_DIR: &str = "infra/cloudflare";

const MAX_HEADER_RULES: usize = 100;
const MAX_REDIRECT_RULES: usize = 2_000;
const MAX_LINE: usize = 2_000;
const MAX_NOT_FOUND_BYTES: usize = 16 * 1024;
/// Apple's documented association file limit is 128 KB; both files are far smaller.
const MAX_ASSOCIATION_BYTES: usize = 128 * 1024;

pub const ASSET_LINKS: &str = ".well-known/assetlinks.json";
pub const APPLE_APP_SITE_ASSOCIATION: &str = ".well-known/apple-app-site-association";
/// The release application of Cantos Theatre. The debug `com.cantos.theatre.dev` is never
/// associated (owner decision recorded in ADR 0048).
const RELEASE_APPLICATION: &str = "com.cantos.theatre";
/// Android's at most two signing-certificate generations during a key rotation, plus slack.
const MAX_FINGERPRINTS: usize = 4;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingFile {
    schema_version: u32,
    accounts: BTreeMap<String, BTreeMap<String, Destination>>,
}

/// Resolve the operator's destination for `(account, target)`.
///
/// The mapping is private (ignored by Git): it names the real account and project and the
/// production branch. Tracked examples carry no allocated identifiers.
pub fn resolve_mapping(
    text: Option<&str>,
    account: &AccountId,
    target: &str,
) -> Result<Destination, String> {
    let Some(text) = text else {
        return Err(format!(
            "no {MAPPING_FILE}; copy {MAPPING_EXAMPLE} and fill your own account, projects and \
             production branches"
        ));
    };
    let file: MappingFile =
        serde_json::from_str(text).map_err(|error| format!("{MAPPING_FILE}: {error}"))?;
    if file.schema_version != 1 {
        return Err(format!("{MAPPING_FILE}: unsupported schema_version"));
    }
    for (id, targets) in &file.accounts {
        AccountId::parse(id).map_err(|why| format!("{MAPPING_FILE}: {why}"))?;
        if let Some(unknown) = targets
            .keys()
            .find(|key| !matches!(key.as_str(), "theatre" | "studio"))
        {
            return Err(format!(
                "{MAPPING_FILE}: unknown app `{unknown}` (theatre or studio)"
            ));
        }
        if let (Some(theatre), Some(studio)) = (targets.get("theatre"), targets.get("studio")) {
            if theatre.project == studio.project {
                return Err(format!(
                    "{MAPPING_FILE}: Theatre and Studio must be separate Pages projects"
                ));
            }
        }
    }
    file.accounts
        .get(account.as_str())
        .ok_or_else(|| format!("account {} is not in {MAPPING_FILE}", account.as_str()))?
        .get(target)
        .cloned()
        .ok_or_else(|| format!("no `{target}` project for this account in {MAPPING_FILE}"))
}

/// The tracked policy files of one app, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyFiles {
    pub headers: String,
    pub not_found: String,
    pub redirects: Option<String>,
    /// Theatre only: `.well-known/assetlinks.json`.
    pub asset_links: Option<String>,
    /// Theatre only: `.well-known/apple-app-site-association`.
    pub apple_app_site_association: Option<String>,
}

impl PolicyFiles {
    /// Every problem, so one run reports the whole list.
    pub fn validate(&self, app: App) -> Result<(), Vec<String>> {
        let mut problems = Vec::new();
        validate_headers(&self.headers, &mut problems);
        validate_not_found(&self.not_found, &mut problems);
        if let Some(redirects) = &self.redirects {
            validate_redirects(redirects, &mut problems);
        }
        if app == App::Theatre {
            validate_verify_email_headers(&self.headers, &mut problems);
        }
        for (name, content) in [
            (ASSET_LINKS, &self.asset_links),
            (APPLE_APP_SITE_ASSOCIATION, &self.apple_app_site_association),
        ] {
            let Some(content) = content else { continue };
            if app != App::Theatre {
                problems.push(format!(
                    "{name}: only Theatre publishes app association files"
                ));
                continue;
            }
            if content.len() > MAX_ASSOCIATION_BYTES {
                problems.push(format!(
                    "{name} is larger than {MAX_ASSOCIATION_BYTES} bytes"
                ));
                continue;
            }
            let parsed = match serde_json::from_str::<Value>(content) {
                Ok(value) => value,
                Err(_) => {
                    problems.push(format!("{name} is not JSON"));
                    continue;
                }
            };
            let verdict = if name == ASSET_LINKS {
                validate_asset_links(&parsed)
            } else {
                validate_apple_association(&parsed)
            };
            if let Err(why) = verdict {
                problems.push(format!("{name}: {why}"));
            }
        }
        if self.apple_app_site_association.is_some() {
            validate_association_media_type(&self.headers, &mut problems);
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }

    /// One digest over the names and bytes of the files that will be added to the bundle.
    pub fn digest(&self) -> String {
        let mut bytes = Vec::new();
        for (name, content) in self.files() {
            bytes.extend_from_slice(name.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(sha256_hex(content.as_bytes()).as_bytes());
            bytes.push(b'\n');
        }
        sha256_hex(&bytes)
    }

    /// The files to add to the staged directory, in a fixed order.
    pub fn files(&self) -> Vec<(&'static str, &str)> {
        let mut files = vec![
            ("_headers", self.headers.as_str()),
            ("404.html", self.not_found.as_str()),
        ];
        if let Some(redirects) = &self.redirects {
            files.push(("_redirects", redirects.as_str()));
        }
        if let Some(links) = &self.asset_links {
            files.push((ASSET_LINKS, links.as_str()));
        }
        if let Some(association) = &self.apple_app_site_association {
            files.push((APPLE_APP_SITE_ASSOCIATION, association.as_str()));
        }
        files
    }
}

struct HeaderRule<'a> {
    path: &'a str,
    headers: Vec<(String, String)>,
}

fn parse_headers<'a>(text: &'a str, problems: &mut Vec<String>) -> Vec<HeaderRule<'a>> {
    let mut rules: Vec<HeaderRule<'_>> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        if line.len() > MAX_LINE {
            problems.push(format!(
                "_headers line {number} is longer than {MAX_LINE} characters"
            ));
            continue;
        }
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with([' ', '\t']) {
            let Some(rule) = rules.last_mut() else {
                problems.push(format!("_headers line {number}: a header before any path"));
                continue;
            };
            let content = line.trim();
            if content.starts_with('!') {
                problems.push(format!(
                    "_headers line {number}: detaching a header is not part of the reviewed policy"
                ));
                continue;
            }
            let Some((name, value)) = content.split_once(':') else {
                problems.push(format!("_headers line {number}: expected `Name: value`"));
                continue;
            };
            let name = name.trim();
            let token =
                !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
            if !token {
                problems.push(format!(
                    "_headers line {number}: `{name}` is not a header name"
                ));
                continue;
            }
            rule.headers
                .push((name.to_ascii_lowercase(), value.trim().to_owned()));
        } else {
            let path = line.trim();
            if !path.starts_with('/') || path.contains(char::is_whitespace) {
                problems.push(format!(
                    "_headers line {number}: a rule starts with an absolute path on this site"
                ));
                continue;
            }
            rules.push(HeaderRule {
                path,
                headers: Vec::new(),
            });
        }
    }
    rules
}

fn validate_headers(text: &str, problems: &mut Vec<String>) {
    let rules = parse_headers(text, problems);
    if rules.len() > MAX_HEADER_RULES {
        problems.push(format!("_headers has more than {MAX_HEADER_RULES} rules"));
    }
    for rule in &rules {
        for (name, value) in &rule.headers {
            let refused = name == "set-cookie"
                || name.starts_with("access-control-")
                || name == "link"
                || name == "refresh";
            if refused {
                problems.push(format!(
                    "_headers `{}`: `{name}` is not allowed (no cookies, CORS grants, preload links or refresh)",
                    rule.path
                ));
            }
            if value.is_empty() {
                problems.push(format!("_headers `{}`: `{name}` has no value", rule.path));
            }
        }
    }
    let values = |path: &str, name: &str| -> Vec<&str> {
        rules
            .iter()
            .filter(|rule| rule.path == path)
            .flat_map(|rule| rule.headers.iter())
            .filter(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
            .collect()
    };
    let require = |problems: &mut Vec<String>, path: &str, name: &str, wanted: &str| {
        let found = values(path, name);
        if !found
            .iter()
            .any(|value| value.to_ascii_lowercase().contains(wanted))
        {
            problems.push(format!(
                "_headers `{path}` must set `{name}` containing `{wanted}`"
            ));
        }
    };
    require(problems, "/*", "referrer-policy", "no-referrer");
    require(problems, "/*", "x-content-type-options", "nosniff");
    require(problems, "/*", "x-frame-options", "deny");
    require(
        problems,
        "/*",
        "content-security-policy",
        "frame-ancestors 'none'",
    );
    require(problems, "/*", "permissions-policy", "camera=()");
    // Pages merges matching rules instead of letting the later one win, so the HTML entry rules
    // are disjoint from `/*` and carry the no-store policy themselves.
    require(problems, "/", "cache-control", "no-store");
    require(problems, "/index.html", "cache-control", "no-store");
    if !values("/*", "cache-control").is_empty() {
        problems.push(
            "_headers `/*` must not set Cache-Control: it would be merged into every asset; \
             immutable caching needs a verified URL-to-bytes invariant first"
                .to_owned(),
        );
    }
}

/// Header values of every rule for exactly `path`.
fn rule_values(text: &str, path: &str, name: &str) -> Vec<String> {
    parse_headers(text, &mut Vec::new())
        .into_iter()
        .filter(|rule| rule.path == path)
        .flat_map(|rule| rule.headers)
        .filter(|(header, _)| header == name)
        .map(|(_, value)| value.to_ascii_lowercase())
        .collect()
}

/// Pages serves `verify-email.html` at `/verify-email` and redirects the `.html` spelling, so
/// both entry spellings carry the same no-store rule as `/` and `/index.html`.
fn validate_verify_email_headers(text: &str, problems: &mut Vec<String>) {
    for path in [VERIFY_EMAIL_PATH, "/verify-email.html"] {
        if !rule_values(text, path, "cache-control")
            .iter()
            .any(|value| value.contains("no-store"))
        {
            problems.push(format!(
                "_headers `{path}` must set `cache-control` containing `no-store`"
            ));
        }
    }
}

/// Apple fetches the extensionless file and requires JSON; Pages would otherwise guess the type.
fn validate_association_media_type(text: &str, problems: &mut Vec<String>) {
    let path = format!("/{APPLE_APP_SITE_ASSOCIATION}");
    let values = rule_values(text, &path, "content-type");
    if values.len() != 1 || values[0] != "application/json" {
        problems.push(format!(
            "_headers `{path}` must set exactly one `Content-Type: application/json`"
        ));
    }
}

fn object<'a>(
    value: &'a Value,
    keys: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, String> {
    let map = value
        .as_object()
        .ok_or_else(|| "expected a JSON object".to_owned())?;
    if let Some(unknown) = map.keys().find(|key| !keys.contains(&key.as_str())) {
        return Err(format!("unexpected field `{unknown}`"));
    }
    Ok(map)
}

fn single<'a>(value: Option<&'a Value>, what: &str) -> Result<&'a Value, String> {
    match value.and_then(Value::as_array).map(Vec::as_slice) {
        Some([only]) => Ok(only),
        _ => Err(format!("`{what}` must hold exactly one entry")),
    }
}

/// A SHA-256 certificate fingerprint in the `AB:CD:…` form Android verifies.
fn fingerprint(value: &Value) -> bool {
    value.as_str().is_some_and(|text| {
        text.len() == 95
            && text.split(':').count() == 32
            && text.split(':').all(|pair| {
                pair.len() == 2
                    && pair
                        .bytes()
                        .all(|b| b.is_ascii_digit() || matches!(b, b'A'..=b'F'))
            })
    })
}

/// Exactly one `handle_all_urls` statement for the release application.
fn validate_asset_links(value: &Value) -> Result<(), String> {
    let statement = match value.as_array().map(Vec::as_slice) {
        Some([only]) => only,
        _ => return Err("must hold exactly one statement".into()),
    };
    let statement = object(statement, &["relation", "target"])?;
    let relation = single(statement.get("relation"), "relation")?;
    if relation.as_str() != Some("delegate_permission/common.handle_all_urls") {
        return Err("the only relation is `delegate_permission/common.handle_all_urls`".into());
    }
    let target = object(
        statement
            .get("target")
            .ok_or_else(|| "missing `target`".to_owned())?,
        &["namespace", "package_name", "sha256_cert_fingerprints"],
    )?;
    if target.get("namespace").and_then(Value::as_str) != Some("android_app") {
        return Err("`target.namespace` must be `android_app`".into());
    }
    if target.get("package_name").and_then(Value::as_str) != Some(RELEASE_APPLICATION) {
        return Err(format!(
            "`target.package_name` must be the release application `{RELEASE_APPLICATION}`"
        ));
    }
    let prints = target
        .get("sha256_cert_fingerprints")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing `sha256_cert_fingerprints`".to_owned())?;
    let unique: std::collections::BTreeSet<_> = prints.iter().filter_map(Value::as_str).collect();
    if prints.is_empty()
        || prints.len() > MAX_FINGERPRINTS
        || unique.len() != prints.len()
        || !prints.iter().all(fingerprint)
    {
        return Err(format!(
            "`sha256_cert_fingerprints` must list 1–{MAX_FINGERPRINTS} distinct uppercase `AB:CD:…` SHA-256 fingerprints"
        ));
    }
    Ok(())
}

/// One `applinks` detail: the release application under one Apple team, claiming only
/// [`VERIFY_EMAIL_PATH`].
fn validate_apple_association(value: &Value) -> Result<(), String> {
    let root = object(value, &["applinks"])?;
    let applinks = object(
        root.get("applinks")
            .ok_or_else(|| "missing `applinks`".to_owned())?,
        &["apps", "details"],
    )?;
    if let Some(apps) = applinks.get("apps") {
        if apps.as_array().is_none_or(|apps| !apps.is_empty()) {
            return Err("`applinks.apps`, when present, must be the empty legacy array".into());
        }
    }
    let detail = object(
        single(applinks.get("details"), "applinks.details")?,
        &["appIDs", "components"],
    )?;
    let app_id = single(detail.get("appIDs"), "appIDs")?
        .as_str()
        .unwrap_or_default();
    let team_ok = app_id
        .strip_suffix(&format!(".{RELEASE_APPLICATION}"))
        .is_some_and(|team| {
            team.len() == 10
                && team
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
        });
    if !team_ok {
        return Err(format!(
            "`appIDs` must be `<TEAMID>.{RELEASE_APPLICATION}` with a 10-character team ID"
        ));
    }
    let component = object(
        single(detail.get("components"), "components")?,
        &["/", "comment"],
    )?;
    if component.get("/").and_then(Value::as_str) != Some(VERIFY_EMAIL_PATH) {
        return Err(format!("the only component path is `{VERIFY_EMAIL_PATH}`"));
    }
    if component
        .get("comment")
        .is_some_and(|comment| !comment.is_string())
    {
        return Err("`comment` must be text".into());
    }
    Ok(())
}

fn validate_not_found(text: &str, problems: &mut Vec<String>) {
    let lower = text.to_ascii_lowercase();
    if text.trim().is_empty() {
        problems.push(
            "404.html is empty: without it Pages serves index.html with 200 for unknown paths"
                .to_owned(),
        );
    }
    if text.len() > MAX_NOT_FOUND_BYTES {
        problems.push(format!(
            "404.html is larger than {MAX_NOT_FOUND_BYTES} bytes"
        ));
    }
    for forbidden in [
        "<script",
        "data-trunk",
        "http://",
        "https://",
        "<iframe",
        "<form",
    ] {
        if lower.contains(forbidden) {
            problems.push(format!(
                "404.html must be a static page with no script, form or external URL (found `{forbidden}`)"
            ));
        }
    }
}

fn validate_redirects(text: &str, problems: &mut Vec<String>) {
    let mut rules = 0usize;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        rules += 1;
        if line.len() > MAX_LINE {
            problems.push(format!("_redirects line {number} is too long"));
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let (source, status) = match parts.as_slice() {
            [source, _destination] => (*source, "302"),
            [source, _destination, status] => (*source, *status),
            _ => {
                problems.push(format!(
                    "_redirects line {number}: expected `source destination [status]`"
                ));
                continue;
            }
        };
        if !matches!(status, "301" | "302" | "303" | "307" | "308") {
            problems.push(format!(
                "_redirects line {number}: status `{status}` is not a redirect (a 200 rewrite or proxy can hide a missing asset behind HTML)"
            ));
        }
        if source == "/api" || source.starts_with("/api/") {
            problems.push(format!(
                "_redirects line {number}: `/api` stays on the API origin and is never routed here"
            ));
        }
    }
    if rules > MAX_REDIRECT_RULES {
        problems.push(format!(
            "_redirects has more than {MAX_REDIRECT_RULES} rules"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const HEADERS: &str = "\
/*
  Referrer-Policy: no-referrer
  X-Content-Type-Options: nosniff
  X-Frame-Options: DENY
  Content-Security-Policy: base-uri 'self'; object-src 'none'; frame-ancestors 'none'
  Permissions-Policy: camera=(), microphone=(), geolocation=()
/
  Cache-Control: no-store
/index.html
  Cache-Control: no-store
/verify-email
  Cache-Control: no-store
/verify-email.html
  Cache-Control: no-store
";
    const NOT_FOUND: &str = "<!doctype html><html lang=\"vi\"><title>Không tìm thấy</title><p>Không tìm thấy trang.</p></html>";
    const ACCOUNT: &str = "8339afe44b4e8cd2b633a38a77f1db41";

    const FINGERPRINT: &str = "14:6D:E9:83:C5:73:06:50:D8:EE:B9:95:2F:34:FC:64:16:A0:83:42:E6:1D:BE:A8:8A:04:96:B2:3F:CF:44:E5";

    fn files() -> PolicyFiles {
        PolicyFiles {
            headers: HEADERS.to_owned(),
            not_found: NOT_FOUND.to_owned(),
            redirects: None,
            asset_links: None,
            apple_app_site_association: None,
        }
    }

    fn problems(files: &PolicyFiles) -> Vec<String> {
        files.validate(App::Theatre).err().unwrap_or_default()
    }

    fn asset_links(package: &str, prints: &[&str]) -> String {
        serde_json::json!([{
            "relation": ["delegate_permission/common.handle_all_urls"],
            "target": {"namespace": "android_app", "package_name": package,
                       "sha256_cert_fingerprints": prints}
        }])
        .to_string()
    }

    fn apple(app_id: &str, path: &str) -> String {
        serde_json::json!({"applinks": {"details": [
            {"appIDs": [app_id], "components": [{"/": path, "comment": "ADR 0048 signup action"}]}
        ]}})
        .to_string()
    }

    fn associated() -> PolicyFiles {
        PolicyFiles {
            headers: format!(
                "{HEADERS}/.well-known/apple-app-site-association\n  Content-Type: application/json\n"
            ),
            asset_links: Some(asset_links("com.cantos.theatre", &[FINGERPRINT])),
            apple_app_site_association: Some(apple("ABCDE12345.com.cantos.theatre", "/verify-email")),
            ..files()
        }
    }

    #[test]
    fn the_reference_policy_is_accepted() {
        assert_eq!(files().validate(App::Theatre), Ok(()));
        assert_eq!(files().validate(App::Studio), Ok(()));
        assert_eq!(associated().validate(App::Theatre), Ok(()));
    }

    #[test]
    fn theatre_entry_rules_cover_the_signup_email_action_path() {
        for path in ["/verify-email", "/verify-email.html"] {
            let mut policy = files();
            policy.headers = policy
                .headers
                .replace(&format!("{path}\n  Cache-Control: no-store\n"), "");
            assert!(
                problems(&policy)
                    .iter()
                    .any(|p| p.contains(&format!("`{path}` must set"))),
                "{path}"
            );
            // Studio has no signup handler.
            assert_eq!(policy.validate(App::Studio), Ok(()));
        }
    }

    #[test]
    fn asset_links_claim_only_the_release_application_with_real_fingerprints() {
        let lower = FINGERPRINT.to_ascii_lowercase();
        let short = &FINGERPRINT[..92];
        for (links, needle) in [
            (
                asset_links("com.cantos.theatre.dev", &[FINGERPRINT]),
                "release application",
            ),
            (
                asset_links("com.example.other", &[FINGERPRINT]),
                "release application",
            ),
            (asset_links("com.cantos.theatre", &[]), "fingerprints"),
            (asset_links("com.cantos.theatre", &[&lower]), "fingerprints"),
            (asset_links("com.cantos.theatre", &[short]), "fingerprints"),
            (
                asset_links("com.cantos.theatre", &[FINGERPRINT, FINGERPRINT]),
                "fingerprints",
            ),
            ("[]".to_owned(), "exactly one statement"),
            ("{}".to_owned(), "exactly one statement"),
            ("not json".to_owned(), "not JSON"),
            (
                asset_links("com.cantos.theatre", &[FINGERPRINT])
                    .replace("handle_all_urls", "get_login_creds"),
                "relation",
            ),
            (
                asset_links("com.cantos.theatre", &[FINGERPRINT])
                    .replace("\"target\"", "\"extra\":1,\"target\""),
                "unexpected field",
            ),
        ] {
            let policy = PolicyFiles {
                asset_links: Some(links.clone()),
                ..files()
            };
            let found = problems(&policy);
            assert!(
                found
                    .iter()
                    .any(|p| p.starts_with(ASSET_LINKS) && p.contains(needle)),
                "{links}: {found:?}"
            );
        }
        let rotated = PolicyFiles {
            asset_links: Some(asset_links(
                "com.cantos.theatre",
                &[FINGERPRINT, &FINGERPRINT.replace("14:6D", "15:6D")],
            )),
            ..files()
        };
        assert_eq!(rotated.validate(App::Theatre), Ok(()));
    }

    #[test]
    fn the_apple_association_claims_only_verify_email_for_the_release_application() {
        for (association, needle) in [
            (
                apple("ABCDE12345.com.cantos.theatre", "/"),
                "component path",
            ),
            (
                apple("ABCDE12345.com.cantos.theatre", "/verify-email/*"),
                "component path",
            ),
            (
                apple("ABCDE12345.com.cantos.theatre", "*"),
                "component path",
            ),
            (
                apple("ABCDE12345.com.cantos.theatre.dev", "/verify-email"),
                "appIDs",
            ),
            (
                apple("abcde12345.com.cantos.theatre", "/verify-email"),
                "appIDs",
            ),
            (apple("ABC.com.cantos.theatre", "/verify-email"), "appIDs"),
            (
                apple("ABCDE12345.com.cantos.theatre", "/verify-email")
                    .replace("\"applinks\"", "\"webcredentials\":{},\"applinks\""),
                "unexpected field",
            ),
            (
                apple("ABCDE12345.com.cantos.theatre", "/verify-email")
                    .replace("\"comment\"", "\"?\":{\"mode\":\"x\"},\"comment\""),
                "unexpected field",
            ),
        ] {
            let policy = PolicyFiles {
                apple_app_site_association: Some(association.clone()),
                ..associated()
            };
            let found = problems(&policy);
            assert!(
                found
                    .iter()
                    .any(|p| p.starts_with(APPLE_APP_SITE_ASSOCIATION) && p.contains(needle)),
                "{association}: {found:?}"
            );
        }
        // Without its JSON media type Apple may refuse it.
        let untyped = PolicyFiles {
            headers: HEADERS.to_owned(),
            ..associated()
        };
        assert!(problems(&untyped)
            .iter()
            .any(|p| p.contains("Content-Type: application/json")));
    }

    #[test]
    fn studio_never_publishes_association_files() {
        let found = associated().validate(App::Studio).unwrap_err();
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.iter().all(|p| p.contains("only Theatre")));
    }

    #[test]
    fn the_tracked_policy_files_of_both_apps_pass_this_validator() {
        for app in ["theatre", "studio"] {
            let read = |name: &str| {
                std::fs::read_to_string(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("..")
                        .join(POLICY_DIR)
                        .join(app)
                        .join(name),
                )
                .unwrap_or_else(|error| panic!("{app}/{name}: {error}"))
            };
            let tracked = PolicyFiles {
                headers: read("_headers"),
                not_found: read("404.html"),
                redirects: None,
                asset_links: None,
                apple_app_site_association: None,
            };
            let app_kind = if app == "theatre" {
                App::Theatre
            } else {
                App::Studio
            };
            assert_eq!(tracked.validate(app_kind), Ok(()), "{app}");
        }
    }

    #[test]
    fn each_required_protection_is_individually_enforced() {
        for (removed, expected) in [
            ("Referrer-Policy: no-referrer", "referrer-policy"),
            ("X-Content-Type-Options: nosniff", "x-content-type-options"),
            ("X-Frame-Options: DENY", "x-frame-options"),
            (
                "Permissions-Policy: camera=(), microphone=(), geolocation=()",
                "permissions-policy",
            ),
        ] {
            let mut policy = files();
            policy.headers = policy.headers.replace(removed, "");
            let found = problems(&policy);
            assert!(
                found.iter().any(|p| p.contains(expected)),
                "{removed}: {found:?}"
            );
        }
        let mut policy = files();
        policy.headers = policy.headers.replace("frame-ancestors 'none'", "");
        assert!(problems(&policy)
            .iter()
            .any(|p| p.contains("frame-ancestors")));
    }

    #[test]
    fn html_entry_rules_must_be_no_store_and_must_not_leak_into_every_asset() {
        let mut policy = files();
        policy.headers = policy
            .headers
            .replace("/index.html\n  Cache-Control: no-store\n", "");
        assert!(problems(&policy)
            .iter()
            .any(|p| p.contains("`/index.html` must set")));
        let mut policy = files();
        policy.headers = policy.headers.replacen(
            "/*\n",
            "/*\n  Cache-Control: public, max-age=31536000, immutable\n",
            1,
        );
        assert!(problems(&policy)
            .iter()
            .any(|p| p.contains("must not set Cache-Control")));
    }

    #[test]
    fn cookies_cors_links_and_header_detachment_are_refused() {
        for extra in [
            "  Set-Cookie: a=b",
            "  Access-Control-Allow-Origin: *",
            "  Link: <https://x>; rel=preload",
            "  ! Referrer-Policy",
        ] {
            let mut policy = files();
            policy.headers.push_str(&format!("/x\n{extra}\n"));
            assert!(!problems(&policy).is_empty(), "{extra}");
        }
    }

    #[test]
    fn malformed_header_lines_are_reported_with_their_line_numbers() {
        let mut policy = files();
        policy.headers = format!(
            "  Orphan: 1\n{}\nnot-a-path\n  Bad Name: 1\n  no colon\n",
            HEADERS
        );
        let found = problems(&policy).join("\n");
        assert!(
            found.contains("line 1: a header before any path"),
            "{found}"
        );
        assert!(found.contains("absolute path"), "{found}");
        assert!(found.contains("is not a header name"), "{found}");
        assert!(found.contains("expected `Name: value`"), "{found}");
    }

    #[test]
    fn the_not_found_page_is_mandatory_static_and_small() {
        for (page, needle) in [
            ("", "is empty"),
            ("<script>1</script>", "no script"),
            ("<a href=\"https://x.example\">x</a>", "external URL"),
            ("<link data-trunk>", "data-trunk"),
        ] {
            let mut policy = files();
            policy.not_found = page.to_owned();
            assert!(
                problems(&policy).iter().any(|p| p.contains(needle)),
                "{page}"
            );
        }
        let mut policy = files();
        policy.not_found = "x".repeat(MAX_NOT_FOUND_BYTES + 1);
        assert!(problems(&policy).iter().any(|p| p.contains("larger than")));
    }

    #[test]
    fn redirects_are_optional_and_never_rewrite_or_touch_the_api() {
        let mut policy = files();
        policy.redirects = Some("# comment\n/old /new 301\n/legacy /new\n".to_owned());
        assert_eq!(policy.validate(App::Theatre), Ok(()));
        for bad in [
            "/* /index.html 200",
            "/api/* https://x 301",
            "/a",
            "/a /b 418",
        ] {
            let mut policy = files();
            policy.redirects = Some(bad.to_owned());
            assert!(!problems(&policy).is_empty(), "{bad}");
        }
    }

    #[test]
    fn the_digest_changes_with_any_byte_and_lists_files_in_a_fixed_order() {
        let base = files();
        let mut changed = files();
        changed.not_found.push(' ');
        assert_ne!(base.digest(), changed.digest());
        assert_eq!(base.digest(), files().digest());
        assert_eq!(
            base.files()
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>(),
            ["_headers", "404.html"]
        );
        let mut with = files();
        with.redirects = Some("/a /b 301".to_owned());
        assert_eq!(with.files().last().unwrap().0, "_redirects");
        assert_eq!(
            associated()
                .files()
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>(),
            [
                "_headers",
                "404.html",
                ASSET_LINKS,
                APPLE_APP_SITE_ASSOCIATION
            ]
        );
        assert_ne!(associated().digest(), base.digest());
    }

    fn mapping(extra: &str) -> String {
        format!(
            r#"{{"schema_version":1,"accounts":{{"{ACCOUNT}":{{
                "theatre":{{"project":"cantos-theatre","production_branch":"main"}},
                "studio":{{"project":"cantos-studio","production_branch":"main"}}{extra}}}}}}}"#
        )
    }

    #[test]
    fn the_mapping_resolves_one_destination_per_account_and_app() {
        let account = AccountId::parse(ACCOUNT).unwrap();
        let studio = resolve_mapping(Some(&mapping("")), &account, "studio").unwrap();
        assert_eq!(studio.project.as_str(), "cantos-studio");
        assert_eq!(studio.production_branch.as_str(), "main");
        let other = AccountId::parse(&"1".repeat(32)).unwrap();
        assert!(resolve_mapping(Some(&mapping("")), &other, "studio")
            .unwrap_err()
            .contains("not in"));
    }

    #[test]
    fn the_mapping_refuses_absence_unknown_fields_apps_and_shared_projects() {
        let account = AccountId::parse(ACCOUNT).unwrap();
        assert!(resolve_mapping(None, &account, "theatre")
            .unwrap_err()
            .contains("projects.example.json"));
        assert!(resolve_mapping(Some("{"), &account, "theatre").is_err());
        assert!(resolve_mapping(
            Some(&mapping(
                r#","api":{"project":"x","production_branch":"main"}"#
            )),
            &account,
            "theatre"
        )
        .unwrap_err()
        .contains("unknown app"));
        let shared = mapping("").replace("cantos-studio", "cantos-theatre");
        assert!(resolve_mapping(Some(&shared), &account, "theatre")
            .unwrap_err()
            .contains("separate"));
        let wrong_version = mapping("").replace("\"schema_version\":1", "\"schema_version\":2");
        assert!(resolve_mapping(Some(&wrong_version), &account, "theatre").is_err());
        let extra_field = mapping("").replace(
            "\"production_branch\":\"main\"}",
            "\"production_branch\":\"main\",\"token\":\"x\"}",
        );
        assert!(resolve_mapping(Some(&extra_field), &account, "theatre").is_err());
    }
}
