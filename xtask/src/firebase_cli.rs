//! What the explicit Firebase commands (`distribute firebase`, `deploy firebase`) share about the
//! Firebase CLI itself: the compatible release line, the version probe, terminal-escape removal
//! and redaction of child output. Each command keeps its own argv builders and output
//! interpreters, because App Distribution and Hosting print different things.
//!
//! The contract is `firebase-tools` **15.x, tested against 15.34.0**. xtask never installs the
//! CLI, never logs in and never reads a credential; the CLI authenticates by its own login or
//! Application Default Credentials.

use std::{path::Path, time::Duration};

use crate::command::CommandSpec;

/// The documented compatible release line, and the exact release the fixtures were taken from.
// ACCEPTANCE(NOT_RUN): 15.34.0 itself was not executed where the Hosting adapter was written (15.33.0
// was installed). The Hosting commands' sources were diffed between 15.33.0 and 15.34.0 and are
// unchanged, but no real Hosting response was captured with either. Owner: the operator's first run.
pub const SUPPORTED_MAJOR: u64 = 15;
pub const MINIMUM_VERSION: (u64, u64, u64) = (15, 34, 0);
pub const TESTED_VERSION: &str = "15.34.0";

pub const VERSION_TIMEOUT: Duration = Duration::from_secs(30);

pub fn command(cwd: &Path) -> CommandSpec {
    CommandSpec::new("firebase", cwd)
}

pub fn version_command(cwd: &Path) -> CommandSpec {
    command(cwd).args(["--version"])
}

pub fn parse_version(stdout: &str) -> Option<(u64, u64, u64)> {
    let line = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let mut parts = line.split('.').map(|part| part.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

pub fn version_supported(version: (u64, u64, u64)) -> bool {
    version.0 == SUPPORTED_MAJOR && version >= MINIMUM_VERSION
}

/// Remove terminal escape sequences, which colour the CLI's markers.
pub fn strip_ansi(text: &str) -> String {
    let mut clean = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        clean.push(c);
    }
    clean
}

/// `uri` when it is an `https://` link whose authority is exactly `host`.
pub fn link_on_host(uri: &str, host: &str) -> Option<String> {
    let rest = uri.strip_prefix("https://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    (authority == host).then(|| uri.to_owned())
}

/// The most informative single error line, redacted. Prefers the CLI's `Error:` line.
pub fn error_line(
    primary: &str,
    secondary: &str,
    secrets: &[&str],
    keep_link: &dyn Fn(&str) -> bool,
) -> String {
    let clean = strip_ansi(&format!("{primary}\n{secondary}"));
    let line = clean
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("Error:"))
        .or_else(|| {
            clean
                .lines()
                .map(str::trim)
                .rev()
                .find(|line| !line.is_empty())
        })
        .unwrap_or("no diagnostic output");
    redact(
        &line.chars().take(240).collect::<String>(),
        secrets,
        keep_link,
    )
}

/// Make child output safe to display or save: no addresses, signed links, tokens or
/// credentials. A link survives only when `keep_link` names it a durable, nonsecret destination.
/// `secrets` are exact values the caller knows to be sensitive.
pub fn redact(text: &str, secrets: &[&str], keep_link: &dyn Fn(&str) -> bool) -> String {
    strip_ansi(text)
        .lines()
        .map(|line| {
            line.split(' ')
                .map(|token| redact_token(token, secrets, keep_link))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_token(token: &str, secrets: &[&str], keep_link: &dyn Fn(&str) -> bool) -> String {
    let trim = |c: char| {
        matches!(
            c,
            '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\'' | ',' | ';' | '.'
        )
    };
    let core = token.trim_matches(trim);
    if core.is_empty() {
        return token.to_owned();
    }
    let replacement = if secrets
        .iter()
        .any(|secret| !secret.is_empty() && core.contains(secret))
    {
        Some("[redacted]")
    } else if core.starts_with("http://") || core.starts_with("https://") {
        (!keep_link(core)).then_some("[url redacted]")
    } else if looks_like_email(core) {
        Some("[email redacted]")
    } else if looks_like_secret(core) {
        Some("[secret redacted]")
    } else {
        None
    };
    match replacement {
        Some(text) => token.replacen(core, text, 1),
        None => token.to_owned(),
    }
}

fn looks_like_email(core: &str) -> bool {
    match core.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty()
                && domain.contains('.')
                && !domain.contains('@')
                && !core.contains('/')
        }
        None => false,
    }
}

fn looks_like_secret(core: &str) -> bool {
    let lower = core.to_ascii_lowercase();
    core.starts_with("ya29.")
        || core.starts_with("AIza")
        || core.starts_with("1//")
        || core.starts_with("-----BEGIN")
        || (core.starts_with("eyJ") && core.len() > 20)
        || [
            "token=",
            "key=",
            "secret=",
            "password=",
            "signature=",
            "x-goog-",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cli_version_gate_accepts_only_the_documented_line() {
        assert_eq!(parse_version("15.34.0\n"), Some((15, 34, 0)));
        assert_eq!(
            parse_version("\n 15.40.2 \nupdate available"),
            Some((15, 40, 2))
        );
        for bad in [
            "",
            "15.34",
            "15.34.0.1",
            "v15.34.0",
            "15.34.0-beta.1",
            "abc",
            "15.x.0",
        ] {
            assert_eq!(parse_version(bad), None, "{bad}");
        }
        assert!(version_supported((15, 34, 0)));
        assert!(version_supported((15, 99, 1)));
        for old_or_new in [(15, 33, 9), (14, 99, 0), (16, 0, 0), (0, 0, 0)] {
            assert!(!version_supported(old_or_new), "{old_or_new:?}");
        }
    }

    #[test]
    fn terminal_escapes_are_removed_before_interpretation() {
        assert_eq!(strip_ansi("\u{1b}[32m✔\u{1b}[0m done"), "✔ done");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    #[test]
    fn links_survive_redaction_only_when_the_caller_names_them_durable() {
        let only_console = |uri: &str| link_on_host(uri, "console.example.com").is_some();
        let text =
            "see https://console.example.com/p/1 and https://signed.example.com/x?token=abc \
                    for qa@example.com using ya29.secret-token";
        let clean = redact(text, &["private-value"], &only_console);
        assert!(clean.contains("https://console.example.com/p/1"));
        assert!(!clean.contains("signed.example.com"));
        assert!(!clean.contains("qa@example.com"));
        assert!(!clean.contains("ya29.secret-token"));
        assert!(redact("private-value", &["private-value"], &only_console).contains("[redacted]"));
    }

    #[test]
    fn host_matching_is_exact_not_a_prefix() {
        assert!(link_on_host("https://a.example.com/x", "a.example.com").is_some());
        assert!(link_on_host("https://a.example.com.evil.test/x", "a.example.com").is_none());
        assert!(link_on_host("http://a.example.com/x", "a.example.com").is_none());
    }
}
