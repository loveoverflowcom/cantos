//! Android build output: the Gradle output metadata, parsing of the installed Android tools'
//! reports and the identity checks that decide whether bytes may be distributed. Pure: the shell
//! runs the tools and reads files; these functions only interpret their results.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use serde_json::Value;

use super::request::{CertFingerprint, Policy, DEMO_VERSION_SUFFIX};
use crate::command::CommandSpec;

/// Largest APK accepted for staging (a local bound; the service limit is not claimed).
pub const APK_LIMIT: u64 = 1024 * 1024 * 1024;

/// The single installable output Gradle declared for the selected variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFile {
    pub file_name: String,
    pub version_code: String,
    pub version_name: String,
}

/// Why the build output cannot be trusted as one installable artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputError {
    MetadataMissing,
    MetadataUnreadable(String),
    MetadataMalformed(String),
    VariantMismatch { expected: String, found: String },
    ApplicationIdMismatch { expected: String, found: String },
    NoOutput,
    MultipleOutputs(usize),
    SplitOutput(String),
    UnsafeFileName(String),
    FileMissing(String),
    NotRegularFile(String),
    Empty(String),
    TooLarge(u64),
    Io(String),
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::MetadataMissing => f.write_str(
                "the selected variant produced no output-metadata.json; the Gradle build did not declare an APK",
            ),
            OutputError::MetadataUnreadable(why) => write!(f, "output-metadata.json is unreadable: {why}"),
            OutputError::MetadataMalformed(why) => write!(f, "output-metadata.json is malformed: {why}"),
            OutputError::VariantMismatch { expected, found } => {
                write!(f, "Gradle output is for variant `{found}`, expected `{expected}`")
            }
            OutputError::ApplicationIdMismatch { expected, found } => write!(
                f,
                "Gradle output declares application `{found}`, expected `{expected}`"
            ),
            OutputError::NoOutput => f.write_str("the variant declares no APK output"),
            OutputError::MultipleOutputs(count) => write!(
                f,
                "the variant declares {count} outputs; exactly one is distributable and none is chosen by glob order"
            ),
            OutputError::SplitOutput(why) => write!(f, "split or filtered output is not distributable: {why}"),
            OutputError::UnsafeFileName(name) => write!(f, "output file name `{name}` is not a plain APK file name"),
            OutputError::FileMissing(name) => write!(f, "declared output `{name}` does not exist"),
            OutputError::NotRegularFile(name) => write!(f, "declared output `{name}` is not a regular file"),
            OutputError::Empty(name) => write!(f, "declared output `{name}` is empty"),
            OutputError::TooLarge(bytes) => write!(f, "APK is {bytes} bytes; the local staging bound is {APK_LIMIT}"),
            OutputError::Io(why) => write!(f, "could not read the APK: {why}"),
        }
    }
}

/// Resolve exactly one distributable output from AGP's own metadata for the selected variant.
pub fn parse_output_metadata(bytes: &[u8], policy: Policy) -> Result<OutputFile, OutputError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| OutputError::MetadataMalformed(error.to_string()))?;
    let text = |key: &str| value.get(key).and_then(Value::as_str);
    let malformed = |what: &str| OutputError::MetadataMalformed(format!("missing {what}"));

    if value.pointer("/artifactType/type").and_then(Value::as_str) != Some("APK") {
        return Err(OutputError::MetadataMalformed(
            "artifactType is not APK".to_owned(),
        ));
    }
    let variant = text("variantName").ok_or_else(|| malformed("variantName"))?;
    if variant != policy.variant() {
        return Err(OutputError::VariantMismatch {
            expected: policy.variant().to_owned(),
            found: variant.to_owned(),
        });
    }
    let application = text("applicationId").ok_or_else(|| malformed("applicationId"))?;
    if application != policy.package() {
        return Err(OutputError::ApplicationIdMismatch {
            expected: policy.package().to_owned(),
            found: application.to_owned(),
        });
    }
    let elements = value
        .get("elements")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed("elements"))?;
    let element = match elements.as_slice() {
        [] => return Err(OutputError::NoOutput),
        [only] => only,
        many => return Err(OutputError::MultipleOutputs(many.len())),
    };
    if element.get("type").and_then(Value::as_str) != Some("SINGLE") {
        return Err(OutputError::SplitOutput(
            "the output type is not SINGLE".to_owned(),
        ));
    }
    let has = |key: &str| {
        element
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|entries| !entries.is_empty())
    };
    if has("filters") {
        return Err(OutputError::SplitOutput(
            "the output has ABI/density filters".to_owned(),
        ));
    }
    let file_name = element
        .get("outputFile")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("outputFile"))?;
    if file_name.is_empty()
        || file_name == ".."
        || file_name.contains(['/', '\\', '\0'])
        || !file_name.ends_with(".apk")
    {
        return Err(OutputError::UnsafeFileName(file_name.to_owned()));
    }
    let version_code = element
        .get("versionCode")
        .map(|code| match code {
            Value::Number(number) => number.to_string(),
            Value::String(text) => text.clone(),
            _ => String::new(),
        })
        .filter(|code| !code.is_empty())
        .ok_or_else(|| malformed("versionCode"))?;
    let version_name = element
        .get("versionName")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| malformed("versionName"))?;
    Ok(OutputFile {
        file_name: file_name.to_owned(),
        version_code,
        version_name: version_name.to_owned(),
    })
}

/// What `aapt2 dump badging` says the bytes are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badging {
    pub package: String,
    pub version_code: String,
    pub version_name: String,
    pub debuggable: bool,
}

/// What `apksigner verify --print-certs -v` says about the signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signing {
    pub signer_count: usize,
    pub cert_sha256: String,
    pub v1: bool,
    pub v2: bool,
    pub v3: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectError {
    /// The tool did not run to a report (could not start, timed out, was interrupted).
    ToolUnavailable(String),
    /// The APK is not parseable by `aapt2`.
    Unreadable(String),
    /// `apksigner` did not verify the signature (unsigned, tampered or malformed).
    NotVerified(String),
    /// The tool exited cleanly but its report was not recognized.
    Unrecognized(&'static str),
}

impl fmt::Display for InspectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InspectError::ToolUnavailable(why) => write!(f, "Android tool did not report: {why}"),
            InspectError::Unreadable(why) => write!(f, "APK is not readable by aapt2: {why}"),
            InspectError::NotVerified(why) => write!(f, "APK signature does not verify: {why}"),
            InspectError::Unrecognized(what) => {
                write!(f, "unrecognized Android tool report: {what}")
            }
        }
    }
}

/// The value of `name='...'` in an aapt2 line.
fn attribute<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!(" {name}='");
    let start = line.find(&marker)? + marker.len();
    let length = line[start..].find('\'')?;
    Some(&line[start..start + length])
}

pub fn parse_badging(code: i32, stdout: &str, stderr: &str) -> Result<Badging, InspectError> {
    if code != 0 {
        return Err(InspectError::Unreadable(first_line(
            if stderr.trim().is_empty() {
                stdout
            } else {
                stderr
            },
        )));
    }
    let package_line = stdout
        .lines()
        .find(|line| line.starts_with("package: "))
        .ok_or(InspectError::Unrecognized("no package line"))?;
    let line = format!(
        " {}",
        package_line.trim_start_matches("package:").trim_start()
    );
    let field = |name: &'static str| {
        attribute(&line, name)
            .map(str::to_owned)
            .ok_or(InspectError::Unrecognized(match name {
                "name" => "package line has no name",
                "versionCode" => "package line has no versionCode",
                _ => "package line has no versionName",
            }))
    };
    Ok(Badging {
        package: field("name")?,
        version_code: field("versionCode")?,
        version_name: field("versionName")?,
        debuggable: stdout
            .lines()
            .any(|line| line.trim() == "application-debuggable"),
    })
}

pub fn parse_signing(code: i32, stdout: &str, stderr: &str) -> Result<Signing, InspectError> {
    if code != 0 || stdout.lines().next().map(str::trim) != Some("Verifies") {
        let reason = if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        };
        return Err(InspectError::NotVerified(first_line(reason)));
    }
    let scheme = |label: &str| {
        let prefix = format!("Verified using {label}: ");
        stdout
            .lines()
            .find_map(|line| line.trim().strip_prefix(prefix.as_str()))
            .map(|value| value.trim() == "true")
            .ok_or(InspectError::Unrecognized(
                "a signature scheme line is missing",
            ))
    };
    let signer_count = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("Number of signers: "))
        .and_then(|count| count.trim().parse::<usize>().ok())
        .ok_or(InspectError::Unrecognized("no signer count"))?;
    let cert_sha256 = stdout
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("Signer #1 certificate SHA-256 digest: ")
        })
        .map(|digest| digest.trim().to_ascii_lowercase())
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or(InspectError::Unrecognized("no signer certificate digest"))?;
    Ok(Signing {
        signer_count,
        cert_sha256,
        v1: scheme("v1 scheme (JAR signing)")?,
        v2: scheme("v2 scheme (APK Signature Scheme v2)")?,
        v3: scheme("v3 scheme (APK Signature Scheme v3)")?,
    })
}

/// A short single line for diagnostics; callers redact before display.
fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no diagnostic output")
        .chars()
        .take(200)
        .collect()
}

/// Every fact about the staged bytes, gathered before anything is judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApkFacts {
    pub sha256: String,
    pub size_bytes: u64,
    pub badging: Badging,
    pub signing: Signing,
}

/// A reason the inspected bytes must not be distributed. Independent findings accumulate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactError {
    PackageMismatch { expected: String, found: String },
    VersionDisagreesWithGradle { gradle: String, apk: String },
    NotDemoLabeled { version_name: String },
    NotDebuggable,
    SignerCount(usize),
    NoModernSignature,
    CertificateMismatch { expected: String, found: String },
}

impl fmt::Display for ArtifactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactError::PackageMismatch { expected, found } => {
                write!(f, "APK package is `{found}`, the demo channel requires `{expected}`")
            }
            ArtifactError::VersionDisagreesWithGradle { gradle, apk } => {
                write!(f, "Gradle declared version {gradle} but the APK says {apk}")
            }
            ArtifactError::NotDemoLabeled { version_name } => write!(
                f,
                "version name `{version_name}` does not end with `{DEMO_VERSION_SUFFIX}`; a demo channel must carry a demo label"
            ),
            ArtifactError::NotDebuggable => {
                f.write_str("the APK is not debuggable, so it is not the debug-variant demo build")
            }
            ArtifactError::SignerCount(count) => {
                write!(f, "the APK has {count} signers; exactly one is required")
            }
            ArtifactError::NoModernSignature => {
                f.write_str("the APK verifies only with the legacy JAR scheme, which Android 7+ may refuse to install")
            }
            ArtifactError::CertificateMismatch { expected, found } => write!(
                f,
                "signing certificate {found} differs from the configured expectation {expected}; a mismatched debug key prevents in-place updates"
            ),
        }
    }
}

pub fn validate(
    policy: Policy,
    declared: &OutputFile,
    facts: &ApkFacts,
    expected_cert: Option<&CertFingerprint>,
) -> Vec<ArtifactError> {
    let mut findings = Vec::new();
    let badging = &facts.badging;
    if badging.package != policy.package() {
        findings.push(ArtifactError::PackageMismatch {
            expected: policy.package().to_owned(),
            found: badging.package.clone(),
        });
    }
    if badging.version_code != declared.version_code
        || badging.version_name != declared.version_name
    {
        findings.push(ArtifactError::VersionDisagreesWithGradle {
            gradle: format!("{} ({})", declared.version_name, declared.version_code),
            apk: format!("{} ({})", badging.version_name, badging.version_code),
        });
    }
    if !badging.version_name.ends_with(DEMO_VERSION_SUFFIX) {
        findings.push(ArtifactError::NotDemoLabeled {
            version_name: badging.version_name.clone(),
        });
    }
    if !badging.debuggable {
        findings.push(ArtifactError::NotDebuggable);
    }
    if facts.signing.signer_count != 1 {
        findings.push(ArtifactError::SignerCount(facts.signing.signer_count));
    }
    if !(facts.signing.v2 || facts.signing.v3) {
        findings.push(ArtifactError::NoModernSignature);
    }
    if let Some(expected) = expected_cert {
        if expected.as_str() != facts.signing.cert_sha256 {
            findings.push(ArtifactError::CertificateMismatch {
                expected: expected.as_str().to_owned(),
                found: facts.signing.cert_sha256.clone(),
            });
        }
    }
    findings
}

pub fn badging_command(aapt2: &Path, apk: &Path, cwd: &Path) -> CommandSpec {
    CommandSpec::new(path_text(aapt2), cwd).args(["dump", "badging", &path_text(apk)])
}

pub fn signer_command(apksigner: &Path, apk: &Path, cwd: &Path) -> CommandSpec {
    CommandSpec::new(path_text(apksigner), cwd).args([
        "verify".to_owned(),
        "--print-certs".to_owned(),
        "-v".to_owned(),
        path_text(apk),
    ])
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The directory in which AGP writes the selected variant's APK and its metadata.
pub fn output_directory(mobile_dir: &Path, policy: Policy) -> PathBuf {
    mobile_dir
        .join("androidApp/build/outputs/apk")
        .join(policy.variant())
}

#[cfg(test)]
mod tests {
    use super::*;

    const METADATA: &str = r#"{
      "version": 3,
      "artifactType": {"type": "APK", "kind": "Directory"},
      "applicationId": "com.cantos.theatre.demo",
      "variantName": "debug",
      "elements": [{"type": "SINGLE", "filters": [], "attributes": [], "versionCode": 1,
                    "versionName": "0.1.0-demo", "outputFile": "androidApp-debug.apk"}],
      "elementType": "File",
      "minSdkVersionForDexing": 23
    }"#;
    const BADGING: &str = include_str!("../../fixtures/android/badging-demo.txt");
    const SIGNER: &str = include_str!("../../fixtures/android/apksigner-demo.txt");

    fn parse(json: &str) -> Result<OutputFile, OutputError> {
        parse_output_metadata(json.as_bytes(), Policy::DemoApk)
    }

    #[test]
    fn one_declared_apk_resolves_with_its_version_identity() {
        let output = parse(METADATA).unwrap();
        assert_eq!(output.file_name, "androidApp-debug.apk");
        assert_eq!(output.version_code, "1");
        assert_eq!(output.version_name, "0.1.0-demo");
    }

    #[test]
    fn ambiguous_split_or_foreign_output_is_refused_with_the_exact_reason() {
        let with = |from: &str, to: &str| METADATA.replace(from, to);
        assert_eq!(
            parse(&with(
                r#""elements": [{"#,
                r#""elements": [{"type":"SINGLE","filters":[],"outputFile":"a.apk","versionCode":1,"versionName":"x"},{"#
            )),
            Err(OutputError::MultipleOutputs(2))
        );
        let empty = METADATA.replace(
            &METADATA[METADATA.find(r#""elements""#).unwrap()
                ..METADATA.find(r#""elementType""#).unwrap()],
            r#""elements": [],"#,
        );
        assert_eq!(parse(&empty), Err(OutputError::NoOutput));
        assert!(matches!(
            parse(&with(
                r#""filters": []"#,
                r#""filters": [{"filterType":"ABI","value":"x86"}]"#
            )),
            Err(OutputError::SplitOutput(_))
        ));
        assert!(matches!(
            parse(&with(r#""type": "SINGLE""#, r#""type": "ONE_OF_MANY""#)),
            Err(OutputError::SplitOutput(_))
        ));
        assert_eq!(
            parse(&with(
                r#""variantName": "debug""#,
                r#""variantName": "release""#
            )),
            Err(OutputError::VariantMismatch {
                expected: "debug".into(),
                found: "release".into()
            })
        );
        assert_eq!(
            parse(&with("com.cantos.theatre.demo", "com.cantos.theatre")),
            Err(OutputError::ApplicationIdMismatch {
                expected: "com.cantos.theatre.demo".into(),
                found: "com.cantos.theatre".into()
            })
        );
        for name in ["../escape.apk", "dir/a.apk", "a.aab", "", ".."] {
            assert!(
                matches!(
                    parse(&with("androidApp-debug.apk", name)),
                    Err(OutputError::UnsafeFileName(_))
                ),
                "{name}"
            );
        }
        assert!(matches!(
            parse("not json"),
            Err(OutputError::MetadataMalformed(_))
        ));
        assert!(matches!(
            parse(&with(r#""type": "APK""#, r#""type": "BUNDLE""#)),
            Err(OutputError::MetadataMalformed(_))
        ));
    }

    #[test]
    fn real_aapt2_and_apksigner_reports_parse() {
        let badging = parse_badging(0, BADGING, "").unwrap();
        assert_eq!(badging.package, "com.cantos.theatre.demo");
        assert_eq!(badging.version_code, "1");
        assert_eq!(badging.version_name, "0.1.0-demo");
        assert!(badging.debuggable);
        let signing = parse_signing(0, SIGNER, "WARNING: ignored").unwrap();
        assert_eq!(signing.signer_count, 1);
        assert_eq!(signing.cert_sha256.len(), 64);
        assert!(signing.v1 && signing.v2 && !signing.v3);
    }

    #[test]
    fn unreadable_unsigned_or_unrecognized_reports_are_typed_failures() {
        assert!(matches!(
            parse_badging(1, "", "x.apk: error: failed opening zip: Invalid file."),
            Err(InspectError::Unreadable(_))
        ));
        assert!(matches!(
            parse_badging(0, "something else\n", ""),
            Err(InspectError::Unrecognized(_))
        ));
        assert!(matches!(
            parse_signing(
                1,
                "DOES NOT VERIFY\nERROR: Missing META-INF/MANIFEST.MF\n",
                ""
            ),
            Err(InspectError::NotVerified(_))
        ));
        assert!(matches!(
            parse_signing(0, "Verifies\n", ""),
            Err(InspectError::Unrecognized(_))
        ));
        assert!(matches!(
            parse_signing(
                1,
                "",
                "Exception in thread \"main\" Malformed APK: not a ZIP archive"
            ),
            Err(InspectError::NotVerified(_))
        ));
        // The release build is not debuggable.
        let release = BADGING.replace("application-debuggable\n", "");
        assert!(!parse_badging(0, &release, "").unwrap().debuggable);
    }

    fn facts() -> ApkFacts {
        ApkFacts {
            sha256: "0".repeat(64),
            size_bytes: 9,
            badging: parse_badging(0, BADGING, "").unwrap(),
            signing: parse_signing(0, SIGNER, "").unwrap(),
        }
    }

    #[test]
    fn identity_validation_accumulates_independent_findings() {
        let declared = parse(METADATA).unwrap();
        assert!(validate(Policy::DemoApk, &declared, &facts(), None).is_empty());

        let mut bad = facts();
        bad.badging.package = "com.cantos.theatre".into();
        bad.badging.debuggable = false;
        bad.badging.version_name = "1.0.0".into();
        bad.signing.signer_count = 2;
        bad.signing.v2 = false;
        let findings = validate(Policy::DemoApk, &declared, &bad, None);
        assert!(findings.contains(&ArtifactError::NotDebuggable));
        assert!(findings.contains(&ArtifactError::SignerCount(2)));
        assert!(findings.contains(&ArtifactError::NoModernSignature));
        assert!(findings
            .iter()
            .any(|f| matches!(f, ArtifactError::PackageMismatch { .. })));
        assert!(findings
            .iter()
            .any(|f| matches!(f, ArtifactError::NotDemoLabeled { .. })));
        assert!(findings
            .iter()
            .any(|f| matches!(f, ArtifactError::VersionDisagreesWithGradle { .. })));
    }

    #[test]
    fn a_configured_certificate_must_match_exactly() {
        let declared = parse(METADATA).unwrap();
        let facts = facts();
        let right = CertFingerprint::parse(&facts.signing.cert_sha256).unwrap();
        assert!(validate(Policy::DemoApk, &declared, &facts, Some(&right)).is_empty());
        let wrong = CertFingerprint::parse(&"ab".repeat(32)).unwrap();
        let findings = validate(Policy::DemoApk, &declared, &facts, Some(&wrong));
        assert_eq!(
            findings,
            [ArtifactError::CertificateMismatch {
                expected: wrong.as_str().into(),
                found: facts.signing.cert_sha256.clone()
            }]
        );
    }

    #[test]
    fn tool_commands_pass_paths_with_spaces_as_single_arguments() {
        let apk = Path::new("/repo/artifacts/run one/artifact/ứng dụng.apk");
        let badging = badging_command(
            Path::new("/sdk/build-tools/35.0.0/aapt2"),
            apk,
            Path::new("/repo"),
        );
        assert_eq!(
            badging.args,
            [
                "dump",
                "badging",
                "/repo/artifacts/run one/artifact/ứng dụng.apk"
            ]
        );
        let signer = signer_command(
            Path::new("/sdk/build-tools/35.0.0/apksigner"),
            apk,
            Path::new("/repo"),
        );
        assert_eq!(
            signer.args.last().unwrap(),
            "/repo/artifacts/run one/artifact/ứng dụng.apk"
        );
        assert_eq!(
            output_directory(
                Path::new("/repo/apps/cantos-theatre/mobile"),
                Policy::DemoApk
            ),
            Path::new("/repo/apps/cantos-theatre/mobile/androidApp/build/outputs/apk/debug")
        );
    }
}
