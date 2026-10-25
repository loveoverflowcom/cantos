//! Flags resolved into a validated request, and the pure checks on a payload's metadata and bytes.
//! Nothing here reads a file, the environment or the network; refusals never quote a payload.

use std::path::PathBuf;

use super::inventory::{self, Format, Inventory, Secret, MAX_PAYLOAD};

/// What the operator typed, before validation.
#[derive(Debug, Clone, Default)]
pub struct RawImport {
    pub project: String,
    pub account: String,
    pub secret: String,
    pub file: Option<String>,
    pub stdin: bool,
    pub expect_latest: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    File(PathBuf),
    Stdin,
}

/// The version the operator last observed. A mismatch means another (possibly uncertain) write
/// landed, so the import refuses instead of stacking a duplicate version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    NoVersion,
    Latest(u64),
}

impl Expect {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "none" => Some(Expect::NoVersion),
            _ if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) => {
                text.parse().ok().filter(|v| *v > 0).map(Expect::Latest)
            }
            _ => None,
        }
    }

    pub fn of(latest: Option<u64>) -> Self {
        latest.map_or(Expect::NoVersion, Expect::Latest)
    }

    pub fn describe(self) -> String {
        match self {
            Expect::NoVersion => "no version".into(),
            Expect::Latest(version) => format!("v{version}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import<'a> {
    pub project: String,
    pub account: String,
    pub secret: &'a Secret,
    pub source: Source,
    pub expect: Expect,
    pub dry_run: bool,
}

/// A gcloud credential name: a user or service-account email. Never inferred from the active
/// gcloud configuration.
pub fn valid_account(account: &str) -> bool {
    let Some((local, domain)) = account.split_once('@') else {
        return false;
    };
    account.len() <= 254
        && !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && account
            .bytes()
            .all(|b| b.is_ascii_graphic() && !matches!(b, b'"' | b'\'' | b'\\' | b',' | b'='))
        && account.matches('@').count() == 1
}

/// The project must be the one the tracked inventory names; another project is refused before
/// any tool starts, whatever gcloud's active configuration says.
pub fn check_project(project: &str, inventory: &Inventory) -> Result<(), String> {
    if project != inventory.project {
        return Err(format!(
            "--project {project:?} is not the inventoried project {}; refusing before any request",
            inventory.project
        ));
    }
    Ok(())
}

pub fn resolve<'a>(raw: &RawImport, inventory: &'a Inventory) -> Result<Import<'a>, String> {
    check_project(&raw.project, inventory)?;
    if !valid_account(&raw.account) {
        return Err("--account must be the explicit gcloud account email to act as".into());
    }
    let secret = inventory.find(&raw.secret).ok_or_else(|| {
        format!(
            "secret {:?} is not in {}; add it there in a reviewed change first",
            raw.secret,
            inventory::INVENTORY_FILE
        )
    })?;
    let source = match (&raw.file, raw.stdin) {
        (Some(_), true) | (None, false) => {
            return Err("give exactly one input: --file PATH or --stdin".into())
        }
        (None, true) => Source::Stdin,
        (Some(path), false) => {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err("--file must be an absolute path outside the checkout".into());
            }
            Source::File(path)
        }
    };
    let expect = Expect::parse(&raw.expect_latest).ok_or_else(|| {
        "--expect-latest must be `none` or the positive version number you last observed".to_owned()
    })?;
    Ok(Import {
        project: raw.project.clone(),
        account: raw.account.clone(),
        secret,
        source,
        expect,
        dry_run: raw.dry_run,
    })
}

/// File facts gathered without reading content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFacts {
    pub symlink: bool,
    pub regular: bool,
    pub owned_by_caller: bool,
    pub mode: u32,
    pub len: u64,
    pub inside_checkout: bool,
}

pub fn check_file(facts: FileFacts, format: Format) -> Result<(), &'static str> {
    if facts.symlink {
        return Err("the input file is a symbolic link; give the real file");
    }
    if !facts.regular {
        return Err("the input is not a regular file");
    }
    if facts.inside_checkout {
        return Err("the input file is inside the repository checkout; keep secrets outside Git");
    }
    if !facts.owned_by_caller {
        return Err("the input file is not owned by the current user");
    }
    if facts.mode & 0o077 != 0 {
        return Err("the input file is readable or writable by group/others; chmod 600 it first");
    }
    check_len(facts.len, format)
}

fn check_len(len: u64, format: Format) -> Result<(), &'static str> {
    if len == 0 {
        return Err("the input is empty; a missing secret stays MISSING, never a blank version");
    }
    if len > MAX_PAYLOAD {
        return Err("the input exceeds Secret Manager's 64 KiB payload limit");
    }
    if format == Format::Bytes32 && len != 32 {
        return Err("this secret must be exactly 32 bytes (the consumer rejects any other size)");
    }
    Ok(())
}

/// Shape checks that match what the consumer will accept. Messages describe the defect, never
/// the bytes.
pub fn check_payload(bytes: &[u8], format: Format) -> Result<(), &'static str> {
    check_len(bytes.len() as u64, format)?;
    match format {
        Format::Bytes32 | Format::Opaque => Ok(()),
        Format::Text => {
            let text = std::str::from_utf8(bytes).map_err(|_| "the text secret is not UTF-8")?;
            if text.chars().any(char::is_control) {
                return Err(
                    "the text secret contains a control character (often a trailing newline)",
                );
            }
            if text.trim().is_empty() {
                return Err("the text secret is blank");
            }
            Ok(())
        }
        Format::ServiceAccountJson => {
            let value: serde_json::Value = serde_json::from_slice(bytes)
                .map_err(|_| "the service-account input is not JSON")?;
            let field = |name: &str| value.get(name).and_then(serde_json::Value::as_str);
            if field("type") != Some("service_account")
                || field("client_email").is_none()
                || field("private_key").is_none()
                || field("project_id").is_none()
            {
                return Err("the input is not a Google service-account credential file");
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory() -> Inventory {
        let root = env!("CARGO_MANIFEST_DIR");
        let text =
            std::fs::read_to_string(format!("{root}/../{}", inventory::INVENTORY_FILE)).unwrap();
        inventory::parse(&text).unwrap()
    }

    fn raw() -> RawImport {
        RawImport {
            project: "cantos-a348b".into(),
            account: "operator@example.test".into(),
            secret: "cantos-dev-identity-smtp-password".into(),
            file: Some("/home/operator/private/smtp".into()),
            stdin: false,
            expect_latest: "none".into(),
            dry_run: true,
        }
    }

    #[test]
    fn a_complete_request_resolves_against_the_inventory() {
        let inventory = inventory();
        let request = resolve(&raw(), &inventory).unwrap();
        assert_eq!(request.secret.id, "cantos-dev-identity-smtp-password");
        assert_eq!(request.expect, Expect::NoVersion);
        assert_eq!(
            request.source,
            Source::File("/home/operator/private/smtp".into())
        );
    }

    #[test]
    fn the_wrong_project_or_an_unlisted_secret_is_refused() {
        let inventory = inventory();
        let mut wrong = raw();
        wrong.project = "cantos-other".into();
        assert!(resolve(&wrong, &inventory)
            .unwrap_err()
            .contains("not the inventoried project"));
        let mut unlisted = raw();
        unlisted.secret = "cantos-prod-anything".into();
        assert!(resolve(&unlisted, &inventory)
            .unwrap_err()
            .contains("not in infra/"));
    }

    #[test]
    fn inputs_accounts_and_fences_must_be_explicit() {
        let inventory = inventory();
        for broken in [
            RawImport {
                stdin: true,
                ..raw()
            },
            RawImport {
                file: None,
                ..raw()
            },
            RawImport {
                file: Some("relative/secret".into()),
                ..raw()
            },
            RawImport {
                account: String::new(),
                ..raw()
            },
            RawImport {
                account: "operator".into(),
                ..raw()
            },
            RawImport {
                expect_latest: "latest".into(),
                ..raw()
            },
            RawImport {
                expect_latest: "0".into(),
                ..raw()
            },
            RawImport {
                expect_latest: String::new(),
                ..raw()
            },
        ] {
            assert!(resolve(&broken, &inventory).is_err(), "{broken:?}");
        }
        assert_eq!(Expect::parse("7"), Some(Expect::Latest(7)));
        assert_eq!(Expect::parse("+7"), None);
    }

    fn facts() -> FileFacts {
        FileFacts {
            symlink: false,
            regular: true,
            owned_by_caller: true,
            mode: 0o600,
            len: 32,
            inside_checkout: false,
        }
    }

    #[test]
    fn file_metadata_must_be_private_regular_and_outside_the_checkout() {
        assert!(check_file(facts(), Format::Bytes32).is_ok());
        assert!(check_file(
            FileFacts {
                mode: 0o400,
                ..facts()
            },
            Format::Bytes32
        )
        .is_ok());
        for bad in [
            FileFacts {
                mode: 0o640,
                ..facts()
            },
            FileFacts {
                mode: 0o604,
                ..facts()
            },
            FileFacts {
                symlink: true,
                ..facts()
            },
            FileFacts {
                regular: false,
                ..facts()
            },
            FileFacts {
                owned_by_caller: false,
                ..facts()
            },
            FileFacts {
                inside_checkout: true,
                ..facts()
            },
            FileFacts { len: 0, ..facts() },
            FileFacts { len: 33, ..facts() },
        ] {
            assert!(check_file(bad, Format::Bytes32).is_err(), "{bad:?}");
        }
        assert!(check_file(
            FileFacts {
                len: MAX_PAYLOAD + 1,
                ..facts()
            },
            Format::Opaque
        )
        .is_err());
    }

    #[test]
    fn payload_shapes_match_their_consumers_and_errors_never_quote_bytes() {
        let synthetic = b"synthetic-not-a-real-password";
        assert!(check_payload(synthetic, Format::Text).is_ok());
        let newline = check_payload(b"synthetic-value\n", Format::Text).unwrap_err();
        assert!(newline.contains("trailing newline"));
        assert!(!newline.contains("synthetic-value"));
        assert!(check_payload(b"   ", Format::Text).is_err());
        assert!(check_payload(&[0xff, 0xfe], Format::Text).is_err());
        assert!(check_payload(&[7u8; 32], Format::Bytes32).is_ok());
        assert!(check_payload(&[7u8; 31], Format::Bytes32).is_err());
        assert!(check_payload(b"\x00\x01binary", Format::Opaque).is_ok());
        let account = br#"{"type":"service_account","project_id":"demo-synthetic","client_email":"synthetic@demo-synthetic.iam.gserviceaccount.com","private_key":"SYNTHETIC"}"#;
        assert!(check_payload(account, Format::ServiceAccountJson).is_ok());
        assert!(
            check_payload(br#"{"type":"authorized_user"}"#, Format::ServiceAccountJson).is_err()
        );
        assert!(check_payload(b"not json", Format::ServiceAccountJson).is_err());
    }

    #[test]
    fn accounts_are_plain_emails() {
        assert!(valid_account("operator@example.test"));
        assert!(valid_account("reader@cantos-a348b.iam.gserviceaccount.com"));
        for bad in [
            "",
            "a@b",
            "@example.test",
            "a@@example.test",
            "a b@example.test",
            "a@example.test,b@example.test",
        ] {
            assert!(!valid_account(bad), "{bad}");
        }
    }
}
