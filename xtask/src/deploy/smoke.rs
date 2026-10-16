//! The post-release HTTP smoke: read-only HTTPS requests to the URL the Firebase CLI reported,
//! judged as pure functions over curl's header dump and the digest of the body it saved.
//!
//! It answers one narrow question, **is the selected static bundle being served as built?**, and
//! nothing about login, the API origin, playback or browser behavior:
//! - the page and each script, WebAssembly and stylesheet file `index.html` loads answer `200`
//!   with the right media type (notably `application/wasm`), a revalidating `Cache-Control` and
//!   *exactly the bytes of the validated bundle* (compared by SHA-256, so a CDN or rewrite that
//!   substitutes anything is caught);
//! - a path that does not exist, and a path under `/api/`, answer `404`, never `200 index.html`.
//!
//! A smoke failure after a confirmed release is a separate local result: the release stays known.

// ACCEPTANCE(NOT_RUN): this judge has only seen fixtures. It has not been run against a real
// Firebase Hosting response, so the media types Hosting assigns (`.wasm`, `.js`, `.css`), the
// `Cache-Control` it emits for the policy in services/firebase-hosting and its CDN propagation
// delay after a release are unobserved. The verified local counterpart is the Firebase Hosting
// emulator run recorded in docs/evidence/issue199-firebase-hosting.md. Owner: the operator of the
// first live preview deploy; command: `cargo xtask deploy firebase <app> --web --project <id>
// --channel <name>` and read the `HTTP smoke` row of its report.

use std::{path::Path, time::Duration};

use super::{bundle::Manifest, receipt::ProbeRecord};
use crate::command::CommandSpec;

pub const TIMEOUT: Duration = Duration::from_secs(60);
/// Read-only requests may be repeated while a new release propagates.
pub const ATTEMPTS: u32 = 3;
pub const PAUSE: Duration = Duration::from_secs(5);
const MAX_BODY_BYTES: &str = "134217728";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Script,
    Style,
    Wasm,
}

impl Family {
    fn media_types(self) -> &'static [&'static str] {
        match self {
            Family::Script => &["text/javascript", "application/javascript"],
            Family::Style => &["text/css"],
            Family::Wasm => &["application/wasm"],
        }
    }

    fn of(path: &str) -> Option<Self> {
        match path.rsplit_once('.')?.1 {
            "js" => Some(Family::Script),
            "css" => Some(Family::Style),
            "wasm" => Some(Family::Wasm),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    Page { sha256: String },
    Asset { family: Family, sha256: String },
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    pub path: String,
    pub expect: Expect,
}

impl Probe {
    pub fn describe(&self) -> String {
        match &self.expect {
            Expect::Page { .. } => {
                "200 text/html, revalidating, bytes equal to the bundle's index.html".to_owned()
            }
            Expect::Asset { family, .. } => format!(
                "200 {}, revalidating, bytes equal to the bundle's file",
                family.media_types()[0]
            ),
            Expect::Absent => "404 (never the page)".to_owned(),
        }
    }
}

/// The probes for a validated bundle: its page, every asset the page loads, a missing `.wasm`
/// and a path under `/api/`. `run_id` makes the missing names unique to this run.
pub fn probes(manifest: &Manifest, run_id: &str) -> Vec<Probe> {
    let digest_of = |path: &str| {
        manifest
            .files
            .iter()
            .find(|file| file.path == path)
            .map(|file| file.sha256.clone())
    };
    let mut probes = Vec::new();
    if let Some(sha256) = digest_of("index.html") {
        probes.push(Probe {
            path: "/".to_owned(),
            expect: Expect::Page { sha256 },
        });
    }
    for asset in &manifest.entry_assets {
        if let (Some(family), Some(sha256)) = (Family::of(asset), digest_of(asset)) {
            probes.push(Probe {
                path: format!("/{asset}"),
                expect: Expect::Asset { family, sha256 },
            });
        }
    }
    probes.push(Probe {
        path: format!("/__cantos-smoke-{run_id}.wasm"),
        expect: Expect::Absent,
    });
    probes.push(Probe {
        path: format!("/api/v1/__cantos-smoke-{run_id}"),
        expect: Expect::Absent,
    });
    probes
}

/// `base` plus `path`, when `base` is a plain `https://<host>` origin and `path` a safe absolute
/// path. Nothing else can be requested.
pub fn url_for(base: &str, path: &str) -> Option<String> {
    let host = base.strip_prefix("https://")?;
    let host_ok = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'.'));
    let path_ok = path.starts_with('/')
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~'));
    (host_ok && path_ok).then(|| format!("{base}{path}"))
}

/// One read-only HTTPS GET. `--dump-header -` puts the headers on stdout; the body goes to
/// `body` so its digest can be compared. Redirects are not followed and only `https` is allowed.
pub fn curl_command(url: &str, body: &Path, cwd: &Path) -> CommandSpec {
    // `--disable` must be the first argument: it ignores `~/.curlrc`, so ambient configuration
    // (for example `location`) cannot make the request follow a redirect to another host.
    CommandSpec::new("curl", cwd).args([
        "--disable",
        "--silent",
        "--show-error",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--tlsv1.2",
        "--max-time",
        "60",
        "--max-filesize",
        MAX_BODY_BYTES,
        "--dump-header",
        "-",
        "--output",
        &body.display().to_string(),
        url,
    ])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub content_type: Option<String>,
    pub cache_control: Option<String>,
}

/// The last response block of a header dump (an interim `100 Continue` may precede it).
pub fn parse_headers(dump: &str) -> Option<Response> {
    let mut last = None;
    let mut current: Option<Response> = None;
    for line in dump.lines().map(|line| line.trim_end_matches('\r')) {
        if line.starts_with("HTTP/") {
            if let Some(done) = current.take() {
                last = Some(done);
            }
            let status = line.split_whitespace().nth(1)?.parse::<u16>().ok()?;
            current = Some(Response {
                status,
                content_type: None,
                cache_control: None,
            });
        } else if let Some(response) = current.as_mut() {
            if let Some((name, value)) = line.split_once(':') {
                let value = value.trim().to_owned();
                match name.trim().to_ascii_lowercase().as_str() {
                    "content-type" => response.content_type = Some(value),
                    "cache-control" => response.cache_control = Some(value),
                    _ => {}
                }
            }
        }
    }
    current.or(last)
}

/// What one request observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    pub response: Response,
    /// SHA-256 of the saved body, when it could be read.
    pub body_sha256: Option<String>,
}

fn media_type(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

fn revalidates(cache_control: &str) -> bool {
    let lower = cache_control.to_ascii_lowercase();
    lower.contains("no-cache") || lower.contains("no-store") || lower.contains("max-age=0")
}

/// Judge one probe. `Err` is a request that did not complete (reason redacted by the caller).
pub fn judge(probe: &Probe, observed: Result<Observed, String>) -> ProbeRecord {
    let mut record = ProbeRecord {
        path: probe.path.clone(),
        expectation: probe.describe(),
        status: None,
        content_type: None,
        cache_control: None,
        bytes_match: None,
        verdict: "fail",
        problems: Vec::new(),
    };
    let observed = match observed {
        Ok(observed) => observed,
        Err(why) => {
            record.verdict = "unreachable";
            record
                .problems
                .push(format!("the request did not complete: {why}"));
            return record;
        }
    };
    let response = &observed.response;
    record.status = Some(response.status);
    record.content_type = response.content_type.clone();
    record.cache_control = response.cache_control.clone();
    let problems = &mut record.problems;
    match &probe.expect {
        Expect::Absent => {
            if response.status != 404 {
                let page = response
                    .content_type
                    .as_deref()
                    .is_some_and(|ct| media_type(ct) == "text/html");
                problems.push(if response.status == 200 && page {
                    "answered 200 with a page: a rewrite is masking a path that does not exist"
                        .to_owned()
                } else {
                    format!("answered {}, expected 404", response.status)
                });
            }
        }
        Expect::Page { sha256 } | Expect::Asset { sha256, .. } => {
            if response.status != 200 {
                problems.push(format!("answered {}, expected 200", response.status));
            }
            let expected_types: &[&str] = match &probe.expect {
                Expect::Asset { family, .. } => family.media_types(),
                _ => &["text/html"],
            };
            match response.content_type.as_deref().map(media_type) {
                Some(found) if expected_types.contains(&found.as_str()) => {}
                Some(found) => problems.push(format!(
                    "media type is `{found}`, expected `{}`",
                    expected_types.join("` or `")
                )),
                None => problems.push("no Content-Type header".to_owned()),
            }
            match response.cache_control.as_deref() {
                Some(value) if revalidates(value) => {}
                Some(value) => {
                    problems.push(format!("Cache-Control `{value}` does not revalidate"))
                }
                None => problems.push("no Cache-Control header".to_owned()),
            }
            match &observed.body_sha256 {
                Some(found) => {
                    let same = found == sha256;
                    record.bytes_match = Some(same);
                    if !same {
                        problems.push("served bytes differ from the validated bundle".to_owned());
                    }
                }
                None => problems.push("the body could not be read to compare bytes".to_owned()),
            }
        }
    }
    if record.problems.is_empty() {
        record.verdict = "ok";
    }
    record
}

/// The smoke result: the last attempt's probes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub attempts: u32,
    pub probes: Vec<ProbeRecord>,
    /// `curl` is not installed, so no request was made.
    pub tool_missing: bool,
}

impl Verdict {
    pub fn unavailable() -> Self {
        Self {
            attempts: 0,
            probes: Vec::new(),
            tool_missing: true,
        }
    }

    pub fn passed(&self) -> bool {
        !self.tool_missing
            && !self.probes.is_empty()
            && self.probes.iter().all(|probe| probe.verdict == "ok")
    }

    pub fn state(&self) -> &'static str {
        if self.tool_missing {
            "unavailable"
        } else if self.passed() {
            "pass"
        } else {
            "fail"
        }
    }

    /// One line per problem, for the report.
    pub fn problems(&self) -> Vec<String> {
        self.probes
            .iter()
            .flat_map(|probe| {
                probe
                    .problems
                    .iter()
                    .map(|problem| format!("{}: {problem}", probe.path))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::bundle::{Manifest, ManifestFile};

    fn manifest() -> Manifest {
        let file = |path: &str, sha: &str| ManifestFile {
            path: path.to_owned(),
            size: 1,
            sha256: sha.to_owned(),
        };
        Manifest {
            files: vec![
                file("cantos-theatre-a.js", "js-sha"),
                file("cantos-theatre-a_bg.wasm", "wasm-sha"),
                file("index.html", "html-sha"),
                file("tokens-b.css", "css-sha"),
            ],
            total_bytes: 4,
            digest: "d".into(),
            entry_assets: vec![
                "cantos-theatre-a.js".into(),
                "cantos-theatre-a_bg.wasm".into(),
                "tokens-b.css".into(),
            ],
        }
    }

    fn observed(
        status: u16,
        content_type: Option<&str>,
        cache: Option<&str>,
        sha: &str,
    ) -> Observed {
        Observed {
            response: Response {
                status,
                content_type: content_type.map(str::to_owned),
                cache_control: cache.map(str::to_owned),
            },
            body_sha256: Some(sha.to_owned()),
        }
    }

    fn probe_of(path: &str) -> Probe {
        probes(&manifest(), "RUN")
            .into_iter()
            .find(|probe| probe.path == path)
            .unwrap()
    }

    #[test]
    fn the_probes_cover_the_page_every_loaded_asset_and_two_paths_that_must_not_exist() {
        let probes = probes(&manifest(), "20261016T000000Z-1-0");
        let paths: Vec<&str> = probes.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "/",
                "/cantos-theatre-a.js",
                "/cantos-theatre-a_bg.wasm",
                "/tokens-b.css",
                "/__cantos-smoke-20261016T000000Z-1-0.wasm",
                "/api/v1/__cantos-smoke-20261016T000000Z-1-0",
            ]
        );
        assert_eq!(
            probes[2].expect,
            Expect::Asset {
                family: Family::Wasm,
                sha256: "wasm-sha".into()
            }
        );
        assert_eq!(probes[4].expect, Expect::Absent);
    }

    #[test]
    fn urls_are_built_only_from_a_plain_https_origin_and_a_safe_path() {
        assert_eq!(
            url_for("https://a-site--x-1.web.app", "/cantos_a.js").as_deref(),
            Some("https://a-site--x-1.web.app/cantos_a.js")
        );
        for (base, path) in [
            ("http://a.web.app", "/x"),
            ("https://a.web.app/", "/x"),
            ("https://a.web.app:8443", "/x"),
            ("https://user@a.web.app", "/x"),
            ("https://A.web.app", "/x"),
            ("https://", "/x"),
            ("https://a.web.app", "x"),
            ("https://a.web.app", "/x?y=1"),
            ("https://a.web.app", "/x#y"),
            ("https://a.web.app", "/x y"),
            ("https://a.web.app", ""),
        ] {
            assert_eq!(url_for(base, path), None, "{base} {path:?}");
        }
        // A doubled slash stays a path under the same host; it cannot change the host.
        assert_eq!(
            url_for("https://a.web.app", "//evil.example/x").as_deref(),
            Some("https://a.web.app//evil.example/x")
        );
    }

    #[test]
    fn the_curl_command_is_a_read_only_https_get_with_no_redirect_following() {
        let command = curl_command(
            "https://a.web.app/",
            Path::new("/run dir/smoke/0.body"),
            Path::new("/run dir"),
        );
        assert_eq!(command.program, "curl");
        let args = command.args.join(" ");
        assert!(args.contains("--proto =https --proto-redir =https"));
        assert!(args.contains("--dump-header -"));
        assert!(!args.contains("--location") && !args.contains(" -L"));
        assert_eq!(
            command.args[0], "--disable",
            "ambient curlrc must be ignored"
        );
        assert!(!args.contains("--request") && !args.contains("--data") && !args.contains("-X"));
        assert_eq!(command.args.last().unwrap(), "https://a.web.app/");
        assert!(command.args.contains(&"/run dir/smoke/0.body".to_owned()));
    }

    #[test]
    fn headers_are_read_from_the_last_response_block_case_insensitively() {
        let dump = "HTTP/2 200 \r\nContent-Type: application/wasm\r\nCACHE-CONTROL: no-cache\r\nserver: x\r\n\r\n";
        assert_eq!(
            parse_headers(dump),
            Some(Response {
                status: 200,
                content_type: Some("application/wasm".into()),
                cache_control: Some("no-cache".into()),
            })
        );
        let interim = "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 404 Not Found\r\ncontent-type: text/html; charset=utf-8\r\n\r\n";
        let response = parse_headers(interim).unwrap();
        assert_eq!(response.status, 404);
        assert_eq!(response.cache_control, None);
        assert_eq!(parse_headers(""), None);
        assert_eq!(parse_headers("not http\n"), None);
        assert_eq!(parse_headers("HTTP/2 abc\n"), None);
    }

    #[test]
    fn a_served_asset_passes_only_with_the_right_status_type_cache_policy_and_bytes() {
        let probe = probe_of("/cantos-theatre-a_bg.wasm");
        let good = judge(
            &probe,
            Ok(observed(
                200,
                Some("application/wasm"),
                Some("no-cache"),
                "wasm-sha",
            )),
        );
        assert_eq!(good.verdict, "ok", "{good:?}");
        assert_eq!(good.bytes_match, Some(true));
        for (label, bad, needle) in [
            (
                "wrong media type",
                observed(
                    200,
                    Some("text/html; charset=utf-8"),
                    Some("no-cache"),
                    "wasm-sha",
                ),
                "media type",
            ),
            (
                "octet stream",
                observed(
                    200,
                    Some("application/octet-stream"),
                    Some("no-cache"),
                    "wasm-sha",
                ),
                "media type",
            ),
            (
                "no content type",
                observed(200, None, Some("no-cache"), "wasm-sha"),
                "Content-Type",
            ),
            (
                "status",
                observed(404, Some("application/wasm"), Some("no-cache"), "wasm-sha"),
                "expected 200",
            ),
            (
                "long cache",
                observed(
                    200,
                    Some("application/wasm"),
                    Some("public, max-age=31536000"),
                    "wasm-sha",
                ),
                "does not revalidate",
            ),
            (
                "no cache header",
                observed(200, Some("application/wasm"), None, "wasm-sha"),
                "Cache-Control",
            ),
            (
                "different bytes",
                observed(200, Some("application/wasm"), Some("no-cache"), "other"),
                "differ",
            ),
        ] {
            let record = judge(&probe, Ok(bad));
            assert_eq!(record.verdict, "fail", "{label}");
            assert!(
                record.problems.iter().any(|p| p.contains(needle)),
                "{label}: {:?}",
                record.problems
            );
        }
        let unreadable = Observed {
            body_sha256: None,
            ..observed(200, Some("application/wasm"), Some("no-cache"), "x")
        };
        assert_eq!(judge(&probe, Ok(unreadable)).verdict, "fail");
    }

    #[test]
    fn scripts_may_be_served_as_either_javascript_media_type_and_the_page_must_be_html() {
        let script = probe_of("/cantos-theatre-a.js");
        for media in ["text/javascript", "application/javascript; charset=utf-8"] {
            let record = judge(
                &script,
                Ok(observed(200, Some(media), Some("no-cache"), "js-sha")),
            );
            assert_eq!(record.verdict, "ok", "{media}");
        }
        let page = probe_of("/");
        assert_eq!(
            judge(
                &page,
                Ok(observed(
                    200,
                    Some("text/html; charset=utf-8"),
                    Some("no-cache, must-revalidate"),
                    "html-sha"
                ))
            )
            .verdict,
            "ok"
        );
        assert_eq!(
            judge(
                &page,
                Ok(observed(
                    200,
                    Some("application/json"),
                    Some("no-cache"),
                    "html-sha"
                ))
            )
            .verdict,
            "fail"
        );
    }

    #[test]
    fn a_missing_path_must_be_404_and_a_masking_page_is_named() {
        let missing = probe_of("/__cantos-smoke-RUN.wasm");
        assert_eq!(
            judge(&missing, Ok(observed(404, Some("text/html"), None, "x"))).verdict,
            "ok"
        );
        let masked = judge(
            &missing,
            Ok(observed(
                200,
                Some("text/html; charset=utf-8"),
                Some("no-cache"),
                "x",
            )),
        );
        assert_eq!(masked.verdict, "fail");
        assert!(
            masked.problems[0].contains("masking"),
            "{:?}",
            masked.problems
        );
        let api = probe_of("/api/v1/__cantos-smoke-RUN");
        let other = judge(&api, Ok(observed(502, None, None, "x")));
        assert_eq!(other.verdict, "fail");
        assert!(other.problems[0].contains("expected 404"));
    }

    const EMU_PAGE: &str = include_str!("../../fixtures/hosting-emulator/page.headers");
    const EMU_WASM: &str = include_str!("../../fixtures/hosting-emulator/wasm.headers");
    const EMU_SCRIPT: &str = include_str!("../../fixtures/hosting-emulator/script.headers");
    const EMU_STYLE: &str = include_str!("../../fixtures/hosting-emulator/style.headers");
    const EMU_MISSING: &str = include_str!("../../fixtures/hosting-emulator/missing-wasm.headers");
    const EMU_API: &str = include_str!("../../fixtures/hosting-emulator/api.headers");

    #[test]
    fn headers_captured_from_the_hosting_emulator_satisfy_the_judge_and_only_the_right_probe() {
        let probe = |path: &str, expect: Expect| Probe {
            path: path.to_owned(),
            expect,
        };
        let asset = |family| Expect::Asset {
            family,
            sha256: "same".into(),
        };
        let observe = |dump: &str| {
            Ok(Observed {
                response: parse_headers(dump).unwrap(),
                body_sha256: Some("same".into()),
            })
        };
        let page = probe(
            "/",
            Expect::Page {
                sha256: "same".into(),
            },
        );
        let wasm = probe("/a_bg.wasm", asset(Family::Wasm));
        let script = probe("/a.js", asset(Family::Script));
        let style = probe("/a.css", asset(Family::Style));
        let missing = probe("/__cantos-smoke-RUN.wasm", Expect::Absent);
        let api = probe("/api/v1/__cantos-smoke-RUN", Expect::Absent);
        for (probe, dump) in [
            (&page, EMU_PAGE),
            (&wasm, EMU_WASM),
            (&script, EMU_SCRIPT),
            (&style, EMU_STYLE),
            (&missing, EMU_MISSING),
            (&api, EMU_API),
        ] {
            let record = judge(probe, observe(dump));
            assert_eq!(
                record.verdict, "ok",
                "{}: {:?}",
                probe.path, record.problems
            );
        }
        // The same real responses do not satisfy a different expectation.
        assert_eq!(judge(&style, observe(EMU_WASM)).verdict, "fail");
        assert_eq!(judge(&wasm, observe(EMU_MISSING)).verdict, "fail");
        assert_eq!(judge(&page, observe(EMU_API)).verdict, "fail");
        // A real 404 is not a page that exists.
        assert_eq!(judge(&script, observe(EMU_MISSING)).verdict, "fail");
        // Real header facts the judge relies on.
        let wasm = parse_headers(EMU_WASM).unwrap();
        assert_eq!(wasm.content_type.as_deref(), Some("application/wasm"));
        assert_eq!(wasm.cache_control.as_deref(), Some("no-cache"));
        assert_eq!(parse_headers(EMU_MISSING).unwrap().status, 404);
    }

    #[test]
    fn an_incomplete_request_is_unreachable_not_a_pass() {
        let record = judge(&probe_of("/"), Err("timed out after 60s".into()));
        assert_eq!(record.verdict, "unreachable");
        assert_eq!(record.status, None);
    }

    #[test]
    fn the_verdict_passes_only_when_every_probe_did_and_a_missing_tool_is_unavailable() {
        let ok = judge(
            &probe_of("/"),
            Ok(observed(
                200,
                Some("text/html"),
                Some("no-cache"),
                "html-sha",
            )),
        );
        let bad = judge(
            &probe_of("/tokens-b.css"),
            Ok(observed(500, None, None, "x")),
        );
        let passing = Verdict {
            attempts: 1,
            probes: vec![ok.clone()],
            tool_missing: false,
        };
        assert!(passing.passed());
        assert_eq!(passing.state(), "pass");
        let failing = Verdict {
            attempts: 3,
            probes: vec![ok, bad],
            tool_missing: false,
        };
        assert!(!failing.passed());
        assert_eq!(failing.state(), "fail");
        assert!(failing
            .problems()
            .iter()
            .any(|line| line.starts_with("/tokens-b.css: ")));
        assert_eq!(Verdict::unavailable().state(), "unavailable");
        assert!(!Verdict::unavailable().passed());
        // No probe at all is not a pass.
        assert!(!Verdict {
            attempts: 1,
            probes: Vec::new(),
            tool_missing: false
        }
        .passed());
    }
}
