//! The Hosting configuration, as pure functions over file text.
//!
//! Two inputs are kept apart on purpose:
//! - the **tracked policy** (`services/firebase-hosting/firebase.json`): headers and the shape of
//!   each target, reviewed in Git, free of any real project or site ID;
//! - the **operator mapping** (`.firebaserc` beside it, ignored by Git): which Hosting site each
//!   deploy target means in a given project. Site IDs are globally allocated names that only the
//!   operator knows, so they are never invented here.
//!
//! A deploy never mutates either file. It validates the policy, resolves exactly one site for the
//! selected target, and renders a *run-specific effective* pair (`firebase.json`, `.firebaserc`)
//! holding that one target, that one site and a run-relative `public` directory, so the Firebase
//! CLI cannot reach the other app, another project or any product besides Hosting.
//!
//! The policy is deliberately narrow. Every rule below exists to keep one of the issue's
//! invariants true by construction rather than by review:
//! - only `hosting` is deployable (no Functions, Cloud Run, Firestore, App Hosting, hooks);
//! - no catch-all rewrite, so `/api/*` or a missing `.wasm`/`.js` is never answered with `200
//!   index.html`. The Cantos web clients navigate by hash and query on `/` and call the API with
//!   absolute `/api/v1/...` paths; no path-based client route exists to rewrite;
//! - a revalidating `Cache-Control` rule covers every path, because Hosting's own default is
//!   `max-age=3600`, and no rule may grant long-lived caching (see [`check_cache_value`]).

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::request::{hosting_target, ProjectId};
use crate::config::App;

/// Repository-relative directory of the tracked policy and the operator's mapping.
pub const CONFIG_DIR: &str = "services/firebase-hosting";
pub const FIREBASE_JSON: &str = "firebase.json";
pub const FIREBASERC: &str = ".firebaserc";
/// The Hosting `public` directory inside a run, relative to the effective config beside it.
pub const PUBLIC_DIR: &str = "dist";

/// A Hosting site ID: DNS-label shaped. Not a project ID, a deploy target or an App ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteId(String);

impl SiteId {
    pub fn parse(text: &str) -> Result<Self, String> {
        let valid = (1..=63).contains(&text.len())
            && !text.starts_with('-')
            && !text.ends_with('-')
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(format!(
                "`{text}` is not a Hosting site ID (lowercase letters, digits and hyphens, at most 63 characters)"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The destination as far as local files can tell. A dry run may carry `Unresolved`; a real run
/// refuses before any tool starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Resolved { site: SiteId },
    Unresolved { reason: String },
}

impl Resolution {
    pub fn from_mapping(result: Result<SiteId, MappingError>) -> Self {
        match result {
            Ok(site) => Resolution::Resolved { site },
            Err(error) => Resolution::Unresolved {
                reason: error.to_string(),
            },
        }
    }
}

/// Why the operator's `.firebaserc` cannot name exactly one site for the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingError {
    /// There is no `.firebaserc` in the configuration directory.
    Missing,
    Malformed,
    /// No `targets` entry for this project. Another project's mapping is never borrowed.
    ProjectNotMapped,
    TargetUnmapped(&'static str),
    TargetAmbiguous {
        target: &'static str,
        sites: usize,
    },
    BadSite {
        target: &'static str,
    },
    /// Both deploy targets name one site, so deploying either would replace the other app.
    SharedSite(String),
}

impl std::fmt::Display for MappingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MappingError::Missing => write!(
                f,
                "no {CONFIG_DIR}/{FIREBASERC}; copy firebaserc.example or run `firebase target:apply hosting <target> <site> --project <id>` there"
            ),
            MappingError::Malformed => write!(
                f,
                "{CONFIG_DIR}/{FIREBASERC} is not strict JSON (comments are not accepted here)"
            ),
            MappingError::ProjectNotMapped => write!(
                f,
                "{FIREBASERC} has no `targets` entry for this project; another project's mapping is never reused"
            ),
            MappingError::TargetUnmapped(target) => write!(
                f,
                "{FIREBASERC} maps no Hosting site to target `{target}` in this project"
            ),
            MappingError::TargetAmbiguous { target, sites } => write!(
                f,
                "{FIREBASERC} maps target `{target}` to {sites} sites; exactly one is required"
            ),
            MappingError::BadSite { target } => write!(
                f,
                "{FIREBASERC} names an invalid site for target `{target}` (one lowercase site ID string is required)"
            ),
            MappingError::SharedSite(site) => write!(
                f,
                "both deploy targets map to site `{site}`; Theatre and Studio need two distinct sites"
            ),
        }
    }
}

/// Resolve the one site for `target` in `project` from the operator's `.firebaserc` text.
///
/// The file is read as strict JSON. (The Firebase CLI also tolerates comments; xtask does not, and
/// reports that instead of guessing.) Project aliases (`projects`) are irrelevant: the effective
/// configuration never carries them, so `--project` can only mean a project ID.
pub fn resolve_mapping(
    firebaserc: Option<&str>,
    project: &ProjectId,
    target: &'static str,
) -> Result<SiteId, MappingError> {
    let text = firebaserc.ok_or(MappingError::Missing)?;
    let document: Value = serde_json::from_str(text).map_err(|_| MappingError::Malformed)?;
    let mapped = document
        .get("targets")
        .and_then(Value::as_object)
        .and_then(|targets| targets.get(project.as_str()))
        .and_then(Value::as_object)
        .ok_or(MappingError::ProjectNotMapped)?;
    let hosting = mapped.get("hosting").and_then(Value::as_object);
    let site_of = |name: &'static str| -> Result<Option<SiteId>, MappingError> {
        let Some(sites) = hosting.and_then(|hosting| hosting.get(name)) else {
            return Ok(None);
        };
        let sites = sites
            .as_array()
            .ok_or(MappingError::BadSite { target: name })?;
        match sites.as_slice() {
            [] => Ok(None),
            [one] => one
                .as_str()
                .and_then(|text| SiteId::parse(text).ok())
                .map(Some)
                .ok_or(MappingError::BadSite { target: name }),
            many => Err(MappingError::TargetAmbiguous {
                target: name,
                sites: many.len(),
            }),
        }
    };
    let site = site_of(target)?.ok_or(MappingError::TargetUnmapped(target))?;
    for other in [App::Theatre, App::Studio].map(hosting_target) {
        if other != target {
            if let Ok(Some(other_site)) = site_of(other) {
                if other_site == site {
                    return Err(MappingError::SharedSite(site.0));
                }
            }
        }
    }
    Ok(site)
}

/// The run-specific `.firebaserc`: one project, one target, one site, no aliases.
pub fn effective_firebaserc(project: &ProjectId, target: &str, site: &SiteId) -> String {
    let document = json!({
        "projects": {},
        "targets": {
            project.as_str(): { "hosting": { target: [site.as_str()] } }
        }
    });
    pretty(&document)
}

fn pretty(document: &Value) -> String {
    let mut text = serde_json::to_string_pretty(document)
        .expect("a document of plain JSON values always serializes");
    text.push('\n');
    text
}

/// Why the tracked policy is not acceptable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    Malformed,
    NotAnObject,
    /// Only `hosting` (and the local-only `emulators`) may appear at the top level.
    UnexpectedTopLevel(String),
    HostingMissing,
    HostingNotAnArray,
    EntryNotAnObject(usize),
    EntryTargetMissing(usize),
    UnknownTarget(String),
    DuplicateTarget(String),
    MissingTarget(&'static str),
    /// A key outside the reviewed allow-list (`site`, `predeploy`, `postdeploy`, `source`,
    /// `redirects`, `i18n`, ...).
    ForbiddenKey {
        target: String,
        key: String,
    },
    BadField {
        target: String,
        field: &'static str,
    },
    /// A rewrite whose source is not a literal route outside `/api`: a pattern could answer `/api/*`
    /// or a missing asset with a page.
    BroadRewrite {
        target: String,
        source: String,
    },
    NonStaticRewrite {
        target: String,
    },
    /// A `Cache-Control` value that grants long-lived caching.
    LongLivedCache {
        target: String,
        value: String,
    },
    /// No rule covers every path with a revalidating `Cache-Control`.
    NoRevalidation {
        target: String,
    },
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let file = format!("{CONFIG_DIR}/{FIREBASE_JSON}");
        match self {
            PolicyError::Malformed => write!(f, "{file} is not valid JSON"),
            PolicyError::NotAnObject => write!(f, "{file} must be a JSON object"),
            PolicyError::UnexpectedTopLevel(key) => write!(
                f,
                "{file} has top-level key `{key}`; only `hosting` (and local-only `emulators`) are allowed, so no other Firebase product can be deployed"
            ),
            PolicyError::HostingMissing => write!(f, "{file} has no `hosting` section"),
            PolicyError::HostingNotAnArray => {
                write!(f, "{file} `hosting` must be an array with one entry per target")
            }
            PolicyError::EntryNotAnObject(index) => {
                write!(f, "{file} hosting entry {index} must be an object")
            }
            PolicyError::EntryTargetMissing(index) => write!(
                f,
                "{file} hosting entry {index} needs a `target` (a `site` would bypass the operator mapping)"
            ),
            PolicyError::UnknownTarget(target) => {
                write!(f, "{file} names unknown target `{target}` (theatre and studio only)")
            }
            PolicyError::DuplicateTarget(target) => {
                write!(f, "{file} has more than one entry for target `{target}`")
            }
            PolicyError::MissingTarget(target) => {
                write!(f, "{file} has no entry for target `{target}`")
            }
            PolicyError::ForbiddenKey { target, key } => write!(
                f,
                "{file} target `{target}` uses `{key}`, which is not allowed (no hooks, sites, redirects, i18n or source frameworks)"
            ),
            PolicyError::BadField { target, field } => {
                write!(f, "{file} target `{target}` has an invalid `{field}`")
            }
            PolicyError::BroadRewrite { target, source } => write!(
                f,
                "{file} target `{target}` rewrites `{source}`; a rewrite source must be a literal route outside /api, because a pattern could answer missing assets and API paths with HTML"
            ),
            PolicyError::NonStaticRewrite { target } => write!(
                f,
                "{file} target `{target}` has a rewrite that is not a static `source` -> `destination` page (no functions, Cloud Run or dynamic links)"
            ),
            PolicyError::LongLivedCache { target, value } => write!(
                f,
                "{file} target `{target}` sets Cache-Control `{value}`; nothing establishes that any URL's bytes never change, so only revalidating values are allowed"
            ),
            PolicyError::NoRevalidation { target } => write!(
                f,
                "{file} target `{target}` has no `**` rule with `Cache-Control: no-cache`; Hosting's own default (max-age=3600) would apply"
            ),
        }
    }
}

const ENTRY_KEYS: [&str; 7] = [
    "target",
    "public",
    "ignore",
    "headers",
    "cleanUrls",
    "trailingSlash",
    "rewrites",
];

/// A reviewed policy per deploy target, in the shape the Firebase CLI reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    entries: BTreeMap<String, Map<String, Value>>,
}

impl Policy {
    pub fn parse(text: &str) -> Result<Self, PolicyError> {
        let document: Value = serde_json::from_str(text).map_err(|_| PolicyError::Malformed)?;
        let root = document.as_object().ok_or(PolicyError::NotAnObject)?;
        if let Some(key) = root
            .keys()
            .find(|key| !matches!(key.as_str(), "hosting" | "emulators"))
        {
            return Err(PolicyError::UnexpectedTopLevel(key.clone()));
        }
        let hosting = root
            .get("hosting")
            .ok_or(PolicyError::HostingMissing)?
            .as_array()
            .ok_or(PolicyError::HostingNotAnArray)?;
        let known = [App::Theatre, App::Studio].map(hosting_target);
        let mut entries = BTreeMap::new();
        for (index, entry) in hosting.iter().enumerate() {
            let entry = entry
                .as_object()
                .ok_or(PolicyError::EntryNotAnObject(index))?;
            let target = entry
                .get("target")
                .and_then(Value::as_str)
                .ok_or(PolicyError::EntryTargetMissing(index))?
                .to_owned();
            if !known.contains(&target.as_str()) {
                return Err(PolicyError::UnknownTarget(target));
            }
            validate_entry(&target, entry)?;
            if entries.insert(target.clone(), entry.clone()).is_some() {
                return Err(PolicyError::DuplicateTarget(target));
            }
        }
        for target in known {
            if !entries.contains_key(target) {
                return Err(PolicyError::MissingTarget(target));
            }
        }
        Ok(Self { entries })
    }

    /// The `public` directory the tracked file names for raw Firebase CLI diagnostics. A deploy
    /// replaces it with the run's own bundle.
    #[cfg(test)]
    pub fn diagnostic_public(&self, target: &str) -> Option<&str> {
        self.entries.get(target)?.get("public")?.as_str()
    }

    /// The run-specific `firebase.json`: exactly this target's reviewed entry, `public` pointing at
    /// the run's own bundle, and nothing else (no `emulators`, no other target).
    pub fn effective_firebase_json(&self, target: &str) -> Option<String> {
        let mut entry = self.entries.get(target)?.clone();
        entry.insert("public".to_owned(), Value::String(PUBLIC_DIR.to_owned()));
        Some(pretty(&json!({ "hosting": [Value::Object(entry)] })))
    }
}

fn validate_entry(target: &str, entry: &Map<String, Value>) -> Result<(), PolicyError> {
    if let Some(key) = entry.keys().find(|key| !ENTRY_KEYS.contains(&key.as_str())) {
        return Err(PolicyError::ForbiddenKey {
            target: target.to_owned(),
            key: key.clone(),
        });
    }
    let bad = |field: &'static str| PolicyError::BadField {
        target: target.to_owned(),
        field,
    };
    match entry.get("public") {
        Some(Value::String(path)) if !path.is_empty() => {}
        _ => return Err(bad("public")),
    }
    if let Some(ignore) = entry.get("ignore") {
        let all_strings = ignore
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string));
        if !all_strings {
            return Err(bad("ignore"));
        }
    }
    for flag in ["cleanUrls", "trailingSlash"] {
        if entry.get(flag).is_some_and(|value| !value.is_boolean()) {
            return Err(bad(if flag == "cleanUrls" {
                "cleanUrls"
            } else {
                "trailingSlash"
            }));
        }
    }
    validate_rewrites(target, entry)?;
    validate_headers(target, entry)
}

fn validate_rewrites(target: &str, entry: &Map<String, Value>) -> Result<(), PolicyError> {
    let Some(rewrites) = entry.get("rewrites") else {
        return Ok(());
    };
    let bad = || PolicyError::BadField {
        target: target.to_owned(),
        field: "rewrites",
    };
    for rewrite in rewrites.as_array().ok_or_else(bad)? {
        let rewrite = rewrite.as_object().ok_or_else(bad)?;
        let static_only = rewrite
            .keys()
            .all(|key| key == "source" || key == "destination");
        let (Some(source), Some(destination)) = (
            rewrite.get("source").and_then(Value::as_str),
            rewrite.get("destination").and_then(Value::as_str),
        ) else {
            return Err(PolicyError::NonStaticRewrite {
                target: target.to_owned(),
            });
        };
        if !static_only || !destination.starts_with('/') || !destination.ends_with(".html") {
            return Err(PolicyError::NonStaticRewrite {
                target: target.to_owned(),
            });
        }
        if !is_literal_route(source) || touches_api(source) {
            return Err(PolicyError::BroadRewrite {
                target: target.to_owned(),
                source: source.to_owned(),
            });
        }
    }
    Ok(())
}

/// A rewrite is acceptable only for a literal absolute route: no glob metacharacter at all, so no
/// catch-all or negated pattern can be smuggled in.
fn is_literal_route(source: &str) -> bool {
    // A last segment with a dot names a file; rewriting a missing asset to a page is the very
    // masking this policy exists to prevent.
    let file_like = source
        .rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.'));
    !file_like
        && source.starts_with('/')
        && source
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~'))
}

fn touches_api(source: &str) -> bool {
    source == "/api" || source.starts_with("/api/")
}

fn validate_headers(target: &str, entry: &Map<String, Value>) -> Result<(), PolicyError> {
    let bad = || PolicyError::BadField {
        target: target.to_owned(),
        field: "headers",
    };
    let rules = entry
        .get("headers")
        .and_then(Value::as_array)
        .ok_or_else(|| PolicyError::NoRevalidation {
            target: target.to_owned(),
        })?;
    let mut revalidates_everything = false;
    for rule in rules {
        let rule = rule.as_object().ok_or_else(bad)?;
        let source = rule
            .get("source")
            .and_then(Value::as_str)
            .filter(|source| !source.is_empty())
            .ok_or_else(bad)?;
        for header in rule
            .get("headers")
            .and_then(Value::as_array)
            .ok_or_else(bad)?
        {
            let key = header.get("key").and_then(Value::as_str).ok_or_else(bad)?;
            let value = header
                .get("value")
                .and_then(Value::as_str)
                .ok_or_else(bad)?;
            if key.eq_ignore_ascii_case("cache-control") {
                check_cache_value(value).map_err(|()| PolicyError::LongLivedCache {
                    target: target.to_owned(),
                    value: value.to_owned(),
                })?;
                if source == "**" && value.to_ascii_lowercase().contains("no-cache") {
                    revalidates_everything = true;
                }
            }
        }
    }
    if revalidates_everything {
        Ok(())
    } else {
        Err(PolicyError::NoRevalidation {
            target: target.to_owned(),
        })
    }
}

/// Only revalidating `Cache-Control` directives are accepted.
///
/// Long-lived caching needs URL-to-byte immutability, which a hash-looking filename does not
/// establish. Observed on the real release bundles of this repository (Trunk 0.21.14): a CSS
/// file's name carries the SeaHash of its delivered bytes, but the `*.js` and `*_bg.wasm` names of
/// one app carry the *same* hash although their bytes differ, so the name identifies the build
/// input and not the delivered file. Until an oracle establishes immutability for a named class of
/// assets, granting `immutable`/`max-age` would let a stale cached `.wasm` outlive the HTML that
/// pins it by integrity, and break the page until the cache expires.
// ACCEPTANCE(NOT_RUN): long-lived caching of hashed assets is deliberately not granted. Reaching
// it needs (a) a verified rule that a URL's bytes change whenever its name must, per asset class,
// and (b) real response headers from Hosting. Owner: a follow-up to issue #199. The VPS edge
// (`services/edge/Caddyfile`, `cantos_spa`) keeps its own existing policy.
pub fn check_cache_value(value: &str) -> Result<(), ()> {
    let lower = value.to_ascii_lowercase();
    let directives: Vec<&str> = lower.split(',').map(str::trim).collect();
    if directives.is_empty() || directives.iter().any(|d| d.is_empty()) {
        return Err(());
    }
    let allowed = |directive: &&str| {
        matches!(
            *directive,
            "no-cache" | "no-store" | "must-revalidate" | "max-age=0"
        )
    };
    if directives.iter().all(allowed) {
        Ok(())
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> ProjectId {
        ProjectId::parse("cantos-example").unwrap()
    }

    fn tracked() -> String {
        let root = crate::workspace::Workspace::discover().unwrap();
        std::fs::read_to_string(root.path(CONFIG_DIR).join(FIREBASE_JSON)).unwrap()
    }

    fn entry(target: &str, extra: &str) -> String {
        format!(
            r#"{{"target":"{target}","public":"x","headers":[{{"source":"**","headers":[{{"key":"Cache-Control","value":"no-cache"}}]}}]{extra}}}"#
        )
    }

    fn policy_text(theatre_extra: &str) -> String {
        format!(
            r#"{{"hosting":[{},{}]}}"#,
            entry("theatre", theatre_extra),
            entry("studio", "")
        )
    }

    #[test]
    fn the_tracked_policy_is_accepted_and_names_each_apps_trunk_output() {
        let policy = Policy::parse(&tracked()).unwrap();
        for app in App::ALL {
            assert_eq!(
                policy.diagnostic_public(hosting_target(app)),
                Some(format!("../../{}/dist", app.web_dir()).as_str())
            );
        }
    }

    #[test]
    fn the_effective_config_holds_exactly_one_target_and_a_run_relative_public_dir() {
        let policy = Policy::parse(&tracked()).unwrap();
        let theatre: Value =
            serde_json::from_str(&policy.effective_firebase_json("theatre").unwrap()).unwrap();
        let hosting = theatre["hosting"].as_array().unwrap();
        assert_eq!(hosting.len(), 1);
        assert_eq!(hosting[0]["target"], "theatre");
        assert_eq!(hosting[0]["public"], PUBLIC_DIR);
        assert_eq!(
            theatre.as_object().unwrap().len(),
            1,
            "no emulators, no products"
        );
        assert!(!theatre.to_string().contains("studio"));
        let studio = policy.effective_firebase_json("studio").unwrap();
        assert!(studio.contains("\"studio\"") && !studio.contains("theatre"));
        assert_eq!(policy.effective_firebase_json("api"), None);
    }

    #[test]
    fn the_effective_rc_binds_one_project_one_target_one_site_and_no_aliases() {
        let site = SiteId::parse("cantos-theatre-example").unwrap();
        let rc: Value =
            serde_json::from_str(&effective_firebaserc(&project(), "theatre", &site)).unwrap();
        assert_eq!(rc["projects"], json!({}));
        assert_eq!(
            rc["targets"],
            json!({"cantos-example": {"hosting": {"theatre": ["cantos-theatre-example"]}}})
        );
    }

    #[test]
    fn other_firebase_products_hooks_and_site_bypasses_are_rejected() {
        for (extra, key) in [
            (r#","predeploy":["npm run build"]"#, "predeploy"),
            (r#","postdeploy":"x""#, "postdeploy"),
            (r#","site":"a-site""#, "site"),
            (r#","redirects":[]"#, "redirects"),
            (r#","i18n":{"root":"/"}"#, "i18n"),
            (r#","source":"app""#, "source"),
        ] {
            assert_eq!(
                Policy::parse(&policy_text(extra)),
                Err(PolicyError::ForbiddenKey {
                    target: "theatre".into(),
                    key: key.into()
                }),
                "{extra}"
            );
        }
        let function_rewrite = r#","rewrites":[{"source":"/x","function":"f"}]"#;
        assert!(matches!(
            Policy::parse(&policy_text(function_rewrite)),
            Err(PolicyError::NonStaticRewrite { .. })
        ));
        for product in [
            "functions",
            "firestore",
            "storage",
            "apphosting",
            "extensions",
        ] {
            let text = format!(r#"{{"{product}":{{}},"hosting":[]}}"#);
            assert_eq!(
                Policy::parse(&text),
                Err(PolicyError::UnexpectedTopLevel(product.into()))
            );
        }
    }

    #[test]
    fn rewrites_must_be_literal_routes_outside_the_api() {
        for source in [
            "**",
            "/**",
            "*",
            "/*",
            "**/**",
            "/api/**",
            "/api",
            "/api/",
            "**/foo",
            "/!(api)/**",
            "/a*",
            "/{a,b}",
            "about",
            "",
            "/app.wasm",
            "/a/b.js",
            "/x.html",
        ] {
            let extra =
                format!(r#","rewrites":[{{"source":"{source}","destination":"/index.html"}}]"#);
            assert!(
                matches!(
                    Policy::parse(&policy_text(&extra)),
                    Err(PolicyError::BroadRewrite { .. })
                ),
                "{source}"
            );
        }
        for literal in ["/about", "/", "/apiary", "/a/b-c_d"] {
            let narrow =
                format!(r#","rewrites":[{{"source":"{literal}","destination":"/index.html"}}]"#);
            assert!(Policy::parse(&policy_text(&narrow)).is_ok(), "{literal}");
        }
        let dynamic = r#","rewrites":[{"source":"/x","run":{"serviceId":"s"}}]"#;
        assert!(matches!(
            Policy::parse(&policy_text(dynamic)),
            Err(PolicyError::NonStaticRewrite { .. })
        ));
        let not_a_page =
            r#","rewrites":[{"source":"/x","destination":"https://elsewhere.example/"}]"#;
        assert!(matches!(
            Policy::parse(&policy_text(not_a_page)),
            Err(PolicyError::NonStaticRewrite { .. })
        ));
    }

    #[test]
    fn long_lived_caching_is_never_granted_by_policy() {
        for value in [
            "public, max-age=31536000, immutable",
            "max-age=3600",
            "immutable",
            "s-maxage=60",
            "public",
            "no-cache, immutable",
            "",
            "no-cache,",
        ] {
            assert_eq!(check_cache_value(value), Err(()), "{value:?}");
            let text = format!(
                r#"{{"hosting":[{{"target":"theatre","public":"x","headers":[{{"source":"**","headers":[{{"key":"Cache-Control","value":"{value}"}}]}}]}},{}]}}"#,
                entry("studio", "")
            );
            assert!(Policy::parse(&text).is_err(), "{value:?}");
        }
        for value in [
            "no-cache",
            "no-store",
            "no-cache, must-revalidate",
            "max-age=0",
        ] {
            assert_eq!(check_cache_value(value), Ok(()), "{value:?}");
        }
    }

    #[test]
    fn a_hash_looking_filename_is_not_evidence_for_caching() {
        // A rule that tries to cache Trunk-hashed assets for a year is refused, whatever the glob.
        let extra = r#","headers":[{"source":"**","headers":[{"key":"Cache-Control","value":"no-cache"}]},{"source":"**/*-[0-9a-f]*.@(js|wasm|css)","headers":[{"key":"Cache-Control","value":"public, max-age=31536000, immutable"}]}]"#;
        let text = format!(
            r#"{{"hosting":[{{"target":"theatre","public":"x"{extra}}},{}]}}"#,
            entry("studio", "")
        );
        assert!(matches!(
            Policy::parse(&text),
            Err(PolicyError::LongLivedCache { .. })
        ));
    }

    #[test]
    fn every_target_must_revalidate_every_path() {
        let none = r#"{"hosting":[{"target":"theatre","public":"x","headers":[]},{"target":"studio","public":"x","headers":[]}]}"#;
        assert_eq!(
            Policy::parse(none),
            Err(PolicyError::NoRevalidation {
                target: "theatre".into()
            })
        );
        let narrow = r#"{"hosting":[{"target":"theatre","public":"x","headers":[{"source":"**/*.html","headers":[{"key":"Cache-Control","value":"no-cache"}]}]},{"target":"studio","public":"x"}]}"#;
        assert!(matches!(
            Policy::parse(narrow),
            Err(PolicyError::NoRevalidation { .. })
        ));
    }

    #[test]
    fn malformed_or_incomplete_policies_name_the_problem() {
        assert_eq!(Policy::parse("not json"), Err(PolicyError::Malformed));
        assert_eq!(Policy::parse("[]"), Err(PolicyError::NotAnObject));
        assert_eq!(Policy::parse("{}"), Err(PolicyError::HostingMissing));
        assert_eq!(
            Policy::parse(r#"{"hosting":{}}"#),
            Err(PolicyError::HostingNotAnArray)
        );
        assert_eq!(
            Policy::parse(r#"{"hosting":[1]}"#),
            Err(PolicyError::EntryNotAnObject(0))
        );
        assert_eq!(
            Policy::parse(r#"{"hosting":[{"public":"x"}]}"#),
            Err(PolicyError::EntryTargetMissing(0))
        );
        assert_eq!(
            Policy::parse(r#"{"hosting":[{"target":"api","public":"x"}]}"#),
            Err(PolicyError::UnknownTarget("api".into()))
        );
        let duplicate = format!(
            r#"{{"hosting":[{},{}]}}"#,
            entry("theatre", ""),
            entry("theatre", "")
        );
        assert_eq!(
            Policy::parse(&duplicate),
            Err(PolicyError::DuplicateTarget("theatre".into()))
        );
        let one = format!(r#"{{"hosting":[{}]}}"#, entry("theatre", ""));
        assert_eq!(
            Policy::parse(&one),
            Err(PolicyError::MissingTarget("studio"))
        );
        let no_public = r#"{"hosting":[{"target":"theatre"}]}"#;
        assert!(matches!(
            Policy::parse(no_public),
            Err(PolicyError::BadField {
                field: "public",
                ..
            })
        ));
        let bad_ignore = policy_text(r#","ignore":[1]"#);
        assert!(matches!(
            Policy::parse(&bad_ignore),
            Err(PolicyError::BadField {
                field: "ignore",
                ..
            })
        ));
    }

    fn rc(body: &str) -> String {
        format!(r#"{{"targets":{{"cantos-example":{{"hosting":{body}}}}}}}"#)
    }

    #[test]
    fn a_mapping_names_exactly_one_site_per_target_in_the_requested_project() {
        let text = rc(r#"{"theatre":["theatre-site"],"studio":["studio-site"]}"#);
        assert_eq!(
            resolve_mapping(Some(&text), &project(), "theatre")
                .unwrap()
                .as_str(),
            "theatre-site"
        );
        assert_eq!(
            resolve_mapping(Some(&text), &project(), "studio")
                .unwrap()
                .as_str(),
            "studio-site"
        );
        // Only the selected target has to be mapped.
        let only = rc(r#"{"theatre":["theatre-site"]}"#);
        assert!(resolve_mapping(Some(&only), &project(), "theatre").is_ok());
    }

    #[test]
    fn missing_ambiguous_cross_project_and_shared_mappings_fail_without_a_guess() {
        let theatre_only = rc(r#"{"theatre":["theatre-site"]}"#);
        let empty_list = rc(r#"{"theatre":[]}"#);
        let two_sites = rc(r#"{"theatre":["a","b"]}"#);
        let shared = rc(r#"{"theatre":["same"],"studio":["same"]}"#);
        let cases: [(Option<&str>, &str, MappingError); 8] = [
            (None, "theatre", MappingError::Missing),
            (Some("{"), "theatre", MappingError::Malformed),
            (
                Some(r#"{"targets":{"other-project":{"hosting":{"theatre":["a"]}}}}"#),
                "theatre",
                MappingError::ProjectNotMapped,
            ),
            (Some("{}"), "theatre", MappingError::ProjectNotMapped),
            (
                Some(theatre_only.as_str()),
                "studio",
                MappingError::TargetUnmapped("studio"),
            ),
            (
                Some(empty_list.as_str()),
                "theatre",
                MappingError::TargetUnmapped("theatre"),
            ),
            (
                Some(two_sites.as_str()),
                "theatre",
                MappingError::TargetAmbiguous {
                    target: "theatre",
                    sites: 2,
                },
            ),
            (
                Some(shared.as_str()),
                "studio",
                MappingError::SharedSite("same".into()),
            ),
        ];
        for (text, target, expected) in cases {
            assert_eq!(
                resolve_mapping(text, &project(), target),
                Err(expected.clone()),
                "{expected}"
            );
        }
        for bad_site in [
            r#"{"theatre":[1]}"#,
            r#"{"theatre":["Bad Site"]}"#,
            r#"{"theatre":"a"}"#,
        ] {
            assert_eq!(
                resolve_mapping(Some(&rc(bad_site)), &project(), "theatre"),
                Err(MappingError::BadSite { target: "theatre" }),
                "{bad_site}"
            );
        }
    }

    #[test]
    fn project_aliases_and_default_projects_never_decide_the_destination() {
        // `--project default` must not resolve through an alias: the lookup keys on the exact ID.
        let aliased = r#"{"projects":{"cantos-example":"another-project-id"},
            "targets":{"another-project-id":{"hosting":{"theatre":["wrong-site"]}}}}"#;
        assert_eq!(
            resolve_mapping(Some(aliased), &project(), "theatre"),
            Err(MappingError::ProjectNotMapped)
        );
    }

    #[test]
    fn resolution_text_is_a_reason_not_a_panic_for_the_dry_run() {
        let unresolved = Resolution::from_mapping(Err(MappingError::Missing));
        assert!(
            matches!(unresolved, Resolution::Unresolved { reason } if reason.contains("firebaserc"))
        );
        let site = SiteId::parse("a-site").unwrap();
        assert_eq!(
            Resolution::from_mapping(Ok(site.clone())),
            Resolution::Resolved { site }
        );
    }
}
