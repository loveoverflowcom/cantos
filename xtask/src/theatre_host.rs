//! The production Theatre Android host. Unlike the Desktop Preview, it composes `:shared` only and
//! has no mock, sample or diagnostic fallback. `android-release` builds an unsigned optimized
//! release APK and AAB, plus an isolated unshrunk release audit and inspects the actual bytes; `android-debug` builds the debug APK pointed
//! at one explicit API origin for the production composition. Release may pin an HTTPS API.
//! Debug uses the Android debug signer. Neither installs nor distributes anything.

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::Result;
use clap::Subcommand;

use crate::{
    command::{CommandSpec, Runner},
    config::THEATRE_MOBILE_DIR,
    mobile_toolchain::{self, ANDROID_BUILD_TOOLS},
    pipeline::{self, Step},
    report::{Report, Status},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

pub const APPLICATION_ID: &str = "com.cantos.theatre";
/// The debug build type's side-by-side identity (`applicationIdSuffix = ".dev"`); never distributed here.
pub const DEBUG_APPLICATION_ID: &str = "com.cantos.theatre.dev";
const LAUNCHER: &str = "com.cantos.theatre.app.TheatreActivity";
const APK: &str = "androidApp/build/outputs/apk/release/androidApp-release-unsigned.apk";
const AAB: &str = "androidApp/build/outputs/bundle/release/androidApp-release.aab";
const REPORT: &str = "androidApp/build/reports/release-boundary/artifact-inspection.txt";
const DEBUG_APK: &str = "androidApp/build/outputs/apk/debug/androidApp-debug.apk";
/// The sandbox's source declarations and resources form the forbidden inventory. The
/// sandbox has no Android target (#287); source declarations seed the forbidden class inventory.
const SANDBOX_CLASSES: &str = "preview-sandbox/src/commonMain/kotlin";
const SANDBOX_RESOURCES: &str = "preview-sandbox/src/commonMain/composeResources";
const COMPOSE_RESOURCE_PACKAGE: &str = "com.cantos.theatre.resources";
/// Production classes whose absence would make a finding-free inspection vacuous.
const REQUIRED_CLASSES: [&str; 9] = [
    "Lcom/cantos/theatre/app/TheatreActivity;",
    "Lcom/cantos/theatre/app/AppContainer;",
    "Lcom/cantos/theatre/features/launch/presentation/CantosListeningUnavailableKt;",
    "Lcom/cantos/theatre/app/TheatreProcessKt;",
    "Lcom/cantos/theatre/app/CantosTheatreAppKt;",
    "Lcom/cantos/theatre/app/TheatreShell;",
    "Lcom/cantos/theatre/features/shell/presentation/CantosPresentationKt;",
    "Lcom/cantos/theatre/adapters/playback/AndroidMediaEngine;",
    "Lcom/cantos/theatre/adapters/playback/CantosPlaybackService;",
];
/// Anonymous catalog transport needs INTERNET; no account or playback capability is implied.
const ALLOWED_PERMISSIONS: [&str; 5] = [
    "com.cantos.theatre.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION",
    "android.permission.INTERNET",
    "android.permission.WAKE_LOCK",
    "android.permission.FOREGROUND_SERVICE",
    "android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK",
];

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum HostAction {
    /// Build optimized release APK/AAB plus an isolated unshrunk release audit; inspect release
    /// classpaths, DEX classes, resources and manifest for sandbox, mock and test content.
    AndroidRelease {
        /// Optional exact HTTPS API origin. Omission builds an explicitly unconfigured host.
        #[arg(long, value_name = "ORIGIN")]
        api_origin: Option<String>,
        /// Optional exact native issuer; identical in audit and optimized builds.
        #[arg(long)]
        native_issuer: Option<String>,
        /// Use the operator-provided release key; never used by the audit build.
        #[arg(long)]
        sign: bool,
    },
    /// Build the production host's debug APK, whose live public catalog reads one development
    /// API origin. Nothing is installed; cleartext is limited to debug loopback.
    AndroidDebug {
        /// `http://127.0.0.1:<port>` or `http://localhost:<port>` (for `adb reverse`), or an
        /// `https://` origin; no path, query, credentials or other cleartext host.
        #[arg(long, value_name = "ORIGIN")]
        api_origin: String,
        /// Optional exact HTTPS public issuer. No privileged provider API is called by the app.
        #[arg(long, value_name = "ISSUER")]
        native_issuer: Option<String>,
    },
    /// Link the shared-only iOS simulator frameworks with installed Kotlin/Native and Xcode.
    IosBuild {
        #[arg(long, value_name = "PATH")]
        kotlin_native_home: PathBuf,
    },
}

pub fn tools() -> Vec<Tool> {
    vec![
        Tool::Gradle,
        Tool::Java,
        Tool::TheatrePreviewGradleProject,
        Tool::Python3,
    ]
}

/// The production host and the sandbox inventory it is checked against must both exist.
pub fn prerequisite(project: &Path, sdk: Option<&Path>) -> Result<(), &'static str> {
    if !project.join(SANDBOX_RESOURCES).is_dir() {
        return Err("production Android host or the sandbox resource inventory is missing");
    }
    mobile_toolchain::android_prerequisite(project, sdk)?;
    if sdk.and_then(Path::to_str).is_none() {
        return Err("configured Android SDK path must be UTF-8 to pass to the inspector");
    }
    Ok(())
}

pub fn validate_native_issuer(api_origin: &str, issuer: &str) -> Result<(), String> {
    validate_release_api_origin(api_origin)?;
    let base = issuer
        .strip_suffix("/oauth2/openid/cantos-theatre-native")
        .filter(|_| issuer.len() <= 2048)
        .ok_or_else(|| {
            "--native-issuer needs the exact cantos-theatre-native issuer path".to_owned()
        })?;
    validate_release_api_origin(base)
}

/// Builds only the production host's release outputs. Its `preBuild` also enforces the
/// resolved release-classpath and source boundary; the sandbox is never compiled here.
pub fn gradle_release_command(workspace: &Workspace, origin: Option<&str>) -> CommandSpec {
    configured_release_command(workspace, origin, None, false, false)
}

fn configured_release_command(
    workspace: &Workspace,
    origin: Option<&str>,
    issuer: Option<&str>,
    audit: bool,
    signed: bool,
) -> CommandSpec {
    mobile_toolchain::gradle_command(workspace).args([
        "-Pcantos.android=true".to_owned(),
        "-Pandroid.builder.sdkDownload=false".to_owned(),
        format!("-Pcantos.releaseAudit={audit}"),
        format!("-Pcantos.releaseSigning={signed}"),
        format!("-Pcantos.apiOrigin={}", origin.unwrap_or_default()),
        format!("-Pcantos.nativeIssuer={}", issuer.unwrap_or_default()),
        ":androidApp:assertReleaseDependencyBoundary".to_owned(),
        ":androidApp:assembleRelease".to_owned(),
        ":androidApp:bundleRelease".to_owned(),
    ])
}

pub fn validate_release_api_origin(origin: &str) -> Result<(), String> {
    validate_api_origin(origin)?;
    if !origin.starts_with("https://") {
        return Err("release --api-origin must use HTTPS".to_owned());
    }
    Ok(())
}

/// The debug host's origin: an exact loopback HTTP origin or an HTTPS origin, nothing else.
/// The app enforces the same rule at runtime; refusing here avoids building a useless APK.
pub fn validate_api_origin(origin: &str) -> Result<(), String> {
    let (scheme, authority) = origin.split_once("://").ok_or_else(|| {
        format!("--api-origin must be an origin like http://127.0.0.1:8080, got `{origin}`")
    })?;
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    let host_ok = !host.is_empty()
        && !host.contains("..")
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-');
    let port_ok = port.is_none_or(|port| port.parse::<u16>().is_ok_and(|port| port != 0));
    let loopback = matches!(host, "127.0.0.1" | "localhost");
    match scheme {
        _ if !host_ok || !port_ok => Err(format!(
            "--api-origin must contain only scheme, lower-case host and optional port, got `{origin}`"
        )),
        "https" => Ok(()),
        "http" if loopback && port.is_some() => Ok(()),
        "http" => Err(format!(
            "--api-origin may use http only for 127.0.0.1 or localhost with a port (adb reverse), got `{origin}`"
        )),
        _ => Err(format!("--api-origin must use https or loopback http, got `{origin}`")),
    }
}

/// Builds only the production host's debug APK with the given origin; never installs it.
#[cfg(test)]
pub fn gradle_debug_command(workspace: &Workspace, origin: &str) -> CommandSpec {
    gradle_debug_identity_command(workspace, origin, None)
}

pub fn gradle_debug_identity_command(
    workspace: &Workspace,
    origin: &str,
    issuer: Option<&str>,
) -> CommandSpec {
    mobile_toolchain::gradle_command(workspace).args([
        "-Pcantos.android=true".to_owned(),
        "-Pandroid.builder.sdkDownload=false".to_owned(),
        "-Pcantos.releaseAudit=false".to_owned(),
        "-Pcantos.releaseSigning=false".to_owned(),
        format!("-Pcantos.apiOrigin={origin}"),
        format!("-Pcantos.nativeIssuer={}", issuer.unwrap_or_default()),
        ":androidApp:assembleDebug".to_owned(),
    ])
}

pub fn gradle_ios_command(workspace: &Workspace, compiler_home: &str) -> CommandSpec {
    mobile_toolchain::gradle_command(workspace).args([
        "-Pcantos.ios=true".to_owned(),
        format!("-Pkotlin.native.home={compiler_home}"),
        ":shared:linkDebugFrameworkIosSimulatorArm64".to_owned(),
        ":shared:linkReleaseFrameworkIosSimulatorArm64".to_owned(),
    ])
}

pub fn inspection_command(workspace: &Workspace, sdk: &str, origin: Option<&str>) -> CommandSpec {
    let mobile = |path: &str| format!("{THEATRE_MOBILE_DIR}/{path}");
    let mut args = vec![
        "scripts/inspect_android_release.py".to_owned(),
        "--check-audit-marker".to_owned(),
        "--check-callback".to_owned(),
        "--check-build-inputs".to_owned(),
        "--apk".to_owned(),
        mobile(APK),
        "--aab".to_owned(),
        mobile(AAB),
        "--aapt2".to_owned(),
        format!("{sdk}/{ANDROID_BUILD_TOOLS}/aapt2"),
        "--application-id".to_owned(),
        APPLICATION_ID.to_owned(),
        "--launcher".to_owned(),
        LAUNCHER.to_owned(),
        "--compose-resource-package".to_owned(),
        COMPOSE_RESOURCE_PACKAGE.to_owned(),
        "--forbidden-classes".to_owned(),
        mobile(SANDBOX_CLASSES),
        "--forbidden-resources".to_owned(),
        mobile(SANDBOX_RESOURCES),
        "--report".to_owned(),
        mobile(REPORT),
    ];
    for class in REQUIRED_CLASSES {
        args.extend(["--require-class".to_owned(), class.to_owned()]);
    }
    for permission in ALLOWED_PERMISSIONS {
        args.extend(["--allowed-permission".to_owned(), permission.to_owned()]);
    }
    args.extend([
        "--allowed-component".to_owned(),
        "com.cantos.theatre.adapters.playback.CantosPlaybackService".to_owned(),
    ]);
    args.extend([
        "--api-origin".to_owned(),
        origin.unwrap_or_default().to_owned(),
    ]);
    CommandSpec::new("python3", workspace.root()).args(args)
}

pub struct ReleaseInspection<'a> {
    pub sdk: &'a str,
    pub apk: &'a Path,
    pub origin: &'a str,
    pub issuer: Option<&'a str>,
    pub mapping: Option<&'a Path>,
    pub audit: bool,
    pub apk_only: bool,
}

pub fn release_inspection_command(
    workspace: &Workspace,
    input: ReleaseInspection<'_>,
) -> CommandSpec {
    let ReleaseInspection {
        sdk,
        apk,
        origin,
        issuer,
        mapping,
        audit,
        apk_only,
    } = input;
    let mut command = inspection_command(workspace, sdk, Some(origin));
    let mut args = Vec::new();
    let mut source = command.args.into_iter();
    while let Some(arg) = source.next() {
        match arg.as_str() {
            "--apk" => {
                source.next();
                args.extend([arg, apk.to_string_lossy().into_owned()]);
            }
            "--aab" if apk_only => {
                source.next();
            }
            "--aab" | "--report" if audit => {
                let value = source
                    .next()
                    .unwrap_or_default()
                    .replace("androidApp/build/", "androidApp/build-release-audit/");
                args.extend([arg, value]);
            }
            _ => args.push(arg),
        }
    }
    args.extend([
        "--native-issuer".to_owned(),
        issuer.unwrap_or_default().to_owned(),
    ]);
    if let Some(mapping) = mapping {
        args.extend([
            "--mapping".to_owned(),
            mapping.to_string_lossy().into_owned(),
        ]);
    }
    if audit {
        args.push("--audit".to_owned());
    }
    if apk_only {
        args.push("--apk-only".to_owned());
    }
    command.args = args;
    command
}

fn evidence_command(workspace: &Workspace, phase: &str, signed: bool) -> CommandSpec {
    CommandSpec::new("python3", workspace.root())
        .args(["scripts/android_release_evidence.py", phase])
        .arg_if(signed, "--signed")
}

fn release_steps(
    workspace: &Workspace,
    sdk: &str,
    origin: Option<&str>,
    issuer: Option<&str>,
    signed: bool,
) -> Vec<Step> {
    let audit = configured_release_command(workspace, origin, issuer, true, false);
    let optimized = if issuer.is_none() && !signed {
        gradle_release_command(workspace, origin)
    } else {
        configured_release_command(workspace, origin, issuer, false, signed)
    };
    let audit_apk = workspace.path(&format!(
        "{THEATRE_MOBILE_DIR}/{}",
        APK.replace("androidApp/build/", "androidApp/build-release-audit/")
    ));
    let optimized_apk = workspace.path(&format!(
        "{THEATRE_MOBILE_DIR}/{}",
        if signed {
            APK.replace("-unsigned.apk", ".apk")
        } else {
            APK.to_owned()
        }
    ));
    let mapping = workspace.path(&format!(
        "{THEATRE_MOBILE_DIR}/androidApp/build/outputs/mapping/release/mapping.txt"
    ));
    vec![
        Step::new(
            "release input identity",
            &tools(),
            vec![evidence_command(workspace, "begin", signed)],
        ),
        Step::new("unshrunk release audit build", &tools(), vec![audit]),
        Step::new(
            "unshrunk release audit inspection",
            &[Tool::Python3],
            vec![
                release_inspection_command(
                    workspace,
                    ReleaseInspection {
                        sdk,
                        apk: &audit_apk,
                        origin: origin.unwrap_or_default(),
                        issuer,
                        mapping: None,
                        audit: true,
                        apk_only: false,
                    },
                ),
                evidence_command(workspace, "audit", signed),
            ],
        ),
        Step::new("optimized release build", &tools(), vec![optimized]),
        Step::new(
            "optimized release inspection and binding",
            &[Tool::Python3],
            vec![
                release_inspection_command(
                    workspace,
                    ReleaseInspection {
                        sdk,
                        apk: &optimized_apk,
                        origin: origin.unwrap_or_default(),
                        issuer,
                        mapping: Some(&mapping),
                        audit: false,
                        apk_only: false,
                    },
                ),
                evidence_command(workspace, "optimized", signed),
            ],
        ),
    ]
}

/// Debug-signing is checked by the distribution coordinator. This proves the actual production
/// APK composition and requested public configuration before installation or upload.
pub fn debug_inspection_command(
    workspace: &Workspace,
    sdk: &str,
    apk: &Path,
    origin: &str,
    issuer: Option<&str>,
) -> CommandSpec {
    let mut command = inspection_command(workspace, sdk, Some(origin));
    let mut args = Vec::new();
    let mut source = command.args.into_iter();
    while let Some(arg) = source.next() {
        match arg.as_str() {
            "--aab" => {
                source.next();
            }
            "--application-id" => {
                source.next();
                args.extend([arg, DEBUG_APPLICATION_ID.to_owned()]);
            }
            // AndroidX derives this permission name from the final application ID.
            "--allowed-permission" => {
                let value = source.next().unwrap_or_default();
                let value = match value.strip_prefix(APPLICATION_ID) {
                    Some(".DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION") => {
                        format!("{DEBUG_APPLICATION_ID}.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION")
                    }
                    _ => value,
                };
                args.extend([arg, value]);
            }
            "--apk" => {
                source.next();
                args.extend([arg, apk.to_string_lossy().into_owned()]);
            }
            "--report" => {
                source.next();
                args.extend([arg, format!("{THEATRE_MOBILE_DIR}/androidApp/build/reports/release-boundary/debug-artifact-inspection.txt")]);
            }
            _ => args.push(arg),
        }
    }
    args.extend([
        "--debug-apk".to_owned(),
        "--native-issuer".to_owned(),
        issuer.unwrap_or_default().to_owned(),
    ]);
    command.args = args;
    command
}

fn release_lock(workspace: &Workspace) -> Result<std::fs::File> {
    let directory = workspace.path(&format!("{THEATRE_MOBILE_DIR}/androidApp/build"));
    std::fs::create_dir_all(&directory)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join("release-audit.lock"))?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: flock only reads the live owned File descriptor and retains no Rust pointer.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            anyhow::bail!("another release audit owns this checkout; use an independent checkout or wait for it to finish");
        }
    }
    #[cfg(not(unix))]
    anyhow::bail!("the release audit requires a POSIX checkout lock");
    #[cfg(unix)]
    Ok(file)
}

fn unsupported(subject: &str, reason: &str) -> ExitCode {
    let mut report = Report::new("theatre-host");
    report.record(subject, Status::Unsupported, reason);
    report.print();
    report.exit_code()
}

pub fn execute(
    workspace: &Workspace,
    action: HostAction,
    tool_probe: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    match action {
        HostAction::AndroidRelease {
            api_origin,
            native_issuer,
            sign,
        } => {
            if let Some(origin) = &api_origin {
                if let Err(message) = validate_release_api_origin(origin) {
                    eprintln!("xtask: error: {message}");
                    return Ok(ExitCode::from(2));
                }
            }
            let sdk = mobile_toolchain::android_sdk_from_environment();
            if let Err(reason) = prerequisite(&workspace.path(THEATRE_MOBILE_DIR), sdk.as_deref()) {
                return Ok(unsupported("Theatre production Android release", reason));
            }
            // The prerequisite has accepted an absolute UTF-8 SDK path.
            let sdk = sdk.as_deref().and_then(Path::to_str).unwrap_or_default();
            if let Some(issuer) = &native_issuer {
                if let Err(message) =
                    validate_native_issuer(api_origin.as_deref().unwrap_or_default(), issuer)
                {
                    eprintln!("xtask: error: {message}");
                    return Ok(ExitCode::from(2));
                }
            }
            let _checkout_lock = release_lock(workspace)?;
            let steps = release_steps(
                workspace,
                sdk,
                api_origin.as_deref(),
                native_issuer.as_deref(),
                sign,
            );
            let report = pipeline::execute("theatre-host", &steps, tool_probe, runner);
            report.print();
            Ok(report.exit_code())
        }
        HostAction::AndroidDebug {
            api_origin,
            native_issuer,
        } => {
            if let Err(message) = validate_api_origin(&api_origin) {
                eprintln!("xtask: error: {message}");
                return Ok(ExitCode::from(2));
            }
            if let Some(issuer) = &native_issuer {
                if let Err(message) = validate_native_issuer(&api_origin, issuer) {
                    eprintln!("xtask: error: {message}");
                    return Ok(ExitCode::from(2));
                }
            }
            let sdk = mobile_toolchain::android_sdk_from_environment();
            if let Err(reason) = prerequisite(&workspace.path(THEATRE_MOBILE_DIR), sdk.as_deref()) {
                return Ok(unsupported("Theatre production Android debug", reason));
            }
            let sdk_text = sdk.as_deref().and_then(Path::to_str).unwrap_or_default();
            let apksigner = Path::new(sdk_text)
                .join(ANDROID_BUILD_TOOLS)
                .join("apksigner");
            if !mobile_toolchain::local_executable(&apksigner) {
                return Ok(unsupported("Theatre production Android debug", "Android build-tools has no executable apksigner; signed artifact inspection requires it"));
            }
            let debug_apk = workspace.path(&format!("{THEATRE_MOBILE_DIR}/{DEBUG_APK}"));
            let steps = vec![
                Step::new(
                    "Theatre production Android debug build",
                    &tools(),
                    vec![gradle_debug_identity_command(
                        workspace,
                        &api_origin,
                        native_issuer.as_deref(),
                    )],
                ),
                Step::new(
                    "Theatre production Android debug inspection",
                    &[Tool::Python3],
                    vec![
                        debug_inspection_command(
                            workspace,
                            sdk_text,
                            &debug_apk,
                            &api_origin,
                            native_issuer.as_deref(),
                        ),
                        CommandSpec::new(apksigner.to_string_lossy(), workspace.root()).args([
                            "verify".to_owned(),
                            "--print-certs".to_owned(),
                            "-v".to_owned(),
                            debug_apk.to_string_lossy().into_owned(),
                        ]),
                        CommandSpec::new(
                            Path::new(sdk_text)
                                .join(ANDROID_BUILD_TOOLS)
                                .join("aapt2")
                                .to_string_lossy(),
                            workspace.root(),
                        )
                        .args([
                            "dump".to_owned(),
                            "badging".to_owned(),
                            debug_apk.to_string_lossy().into_owned(),
                        ]),
                    ],
                ),
            ];
            let mut report = pipeline::execute("theatre-host", &steps, tool_probe, runner);
            if report.succeeded() {
                report.note(format!(
                    "APK: {THEATRE_MOBILE_DIR}/{DEBUG_APK}; install it yourself with `adb install -r` \
                     and, for a loopback origin, `adb reverse tcp:PORT tcp:PORT`. Runtime acceptance is separate"
                ));
            }
            report.print();
            Ok(report.exit_code())
        }
        HostAction::IosBuild { kotlin_native_home } => {
            if let Err(reason) = mobile_toolchain::native_compiler_prerequisite(&kotlin_native_home)
            {
                return Ok(unsupported("Theatre production iOS frameworks", reason));
            }
            if !mobile_toolchain::ios_sdk_available() {
                return Ok(unsupported("Theatre production iOS frameworks",
                    "requires full installed Xcode with a usable iOS simulator SDK; xtask never installs or selects Xcode"));
            }
            let project = workspace.path(THEATRE_MOBILE_DIR);
            if !project
                .join("iosApp/Cantos.xcodeproj/project.pbxproj")
                .is_file()
                || !project
                    .join("shared/src/iosMain/kotlin/com/cantos/theatre/app/IosCatalogOwner.kt")
                    .is_file()
            {
                return Ok(unsupported(
                    "Theatre production iOS frameworks",
                    "production iOS project or catalog owner is missing",
                ));
            }
            let steps = vec![Step::new(
                "Theatre production iOS frameworks",
                &tools(),
                vec![gradle_ios_command(
                    workspace,
                    &kotlin_native_home.to_string_lossy(),
                )],
            )];
            let report = pipeline::execute("theatre-host", &steps, tool_probe, runner);
            report.print();
            Ok(report.exit_code())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::testing::RecordingRunner, tools::testing::FakeTools};
    use std::path::PathBuf;

    fn workspace() -> Workspace {
        Workspace::discover().unwrap()
    }

    fn sdk_fixture(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-theatre-host-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("platforms/android-35")).unwrap();
        std::fs::create_dir_all(dir.join(ANDROID_BUILD_TOOLS)).unwrap();
        std::fs::write(dir.join("platforms/android-35/android.jar"), "").unwrap();
        for tool in ["aapt2", "d8"] {
            let path = dir.join(ANDROID_BUILD_TOOLS).join(tool);
            std::fs::write(&path, "fixture executable; never executed").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        dir
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_audits_cannot_replace_each_others_fixed_outputs() {
        let workspace = workspace();
        let first = release_lock(&workspace).unwrap();
        assert!(release_lock(&workspace)
            .unwrap_err()
            .to_string()
            .contains("another release audit"));
        drop(first);
        assert!(release_lock(&workspace).is_ok());
    }

    #[test]
    fn audit_precedes_optimization_and_cannot_inherit_the_operator_signer() {
        let workspace = workspace();
        let steps = release_steps(
            &workspace,
            "/sdk",
            Some("https://api.example.org"),
            None,
            true,
        );
        assert_eq!(steps.len(), 5);
        let mut runner = RecordingRunner {
            fail_on: Some("releaseAudit=true".into()),
            ..Default::default()
        };
        let report = pipeline::execute("release audit", &steps, &FakeTools::all(), &mut runner);
        assert_eq!(report.outcomes[1].status, Status::Fail);
        assert_eq!(report.outcomes[3].status, Status::NotRun);
        assert_eq!(runner.commands.len(), 2);
        let audit = &runner.commands[1];
        assert!(audit.args.contains(&"-Pcantos.releaseSigning=false".into()));
        assert!(!audit.args.iter().any(|a| a.contains("sandbox")));
        let mut runner = RecordingRunner::default();
        assert!(
            pipeline::execute("release audit", &steps, &FakeTools::all(), &mut runner).succeeded()
        );
        let gradle: Vec<_> = runner
            .commands
            .iter()
            .filter(|c| c.program == "gradle")
            .collect();
        assert_eq!(gradle.len(), 2);
        assert!(gradle[1]
            .args
            .contains(&"-Pcantos.releaseSigning=true".into()));
        assert!(gradle[1]
            .args
            .contains(&"-Pcantos.releaseAudit=false".into()));
    }

    #[test]
    fn release_build_names_only_production_outputs_and_the_sandbox_inventory() {
        let workspace = workspace();
        let command = gradle_release_command(&workspace, None);
        assert_eq!(command.program, "gradle");
        assert!(command.env.is_empty());
        assert!(command.args.ends_with(&[
            "-Pcantos.android=true".to_owned(),
            "-Pandroid.builder.sdkDownload=false".to_owned(),
            "-Pcantos.releaseAudit=false".to_owned(),
            "-Pcantos.releaseSigning=false".to_owned(),
            "-Pcantos.apiOrigin=".to_owned(),
            "-Pcantos.nativeIssuer=".to_owned(),
            ":androidApp:assertReleaseDependencyBoundary".to_owned(),
            ":androidApp:assembleRelease".to_owned(),
            ":androidApp:bundleRelease".to_owned(),
        ]));
        assert!(command
            .args
            .contains(&"-Dorg.gradle.java.installations.auto-download=false".to_owned()));
        // Exactly these tasks: never the retired qa build type, signing, installation or a debug variant.
        let tasks: Vec<_> = command
            .args
            .iter()
            .filter(|arg| arg.starts_with(':'))
            .collect();
        assert_eq!(tasks.len(), 3);
        assert!(!tasks.iter().any(|task| task.contains("Qa")
            || task.contains("Debug")
            || task.contains("install")
            || task.contains("sign")));
    }

    #[test]
    fn debug_build_names_only_the_production_debug_apk_and_the_origin() {
        let workspace = workspace();
        let command = gradle_debug_command(&workspace, "http://127.0.0.1:8080");
        assert_eq!(command.program, "gradle");
        assert!(command.args.ends_with(&[
            "-Pcantos.android=true".to_owned(),
            "-Pandroid.builder.sdkDownload=false".to_owned(),
            "-Pcantos.releaseAudit=false".to_owned(),
            "-Pcantos.releaseSigning=false".to_owned(),
            "-Pcantos.apiOrigin=http://127.0.0.1:8080".to_owned(),
            "-Pcantos.nativeIssuer=".to_owned(),
            ":androidApp:assembleDebug".to_owned(),
        ]));
        let tasks: Vec<_> = command
            .args
            .iter()
            .filter(|arg| arg.starts_with(':'))
            .collect();
        assert_eq!(tasks, [":androidApp:assembleDebug"]);
    }

    #[test]
    fn release_origin_is_https_and_pinned_into_build_and_artifact_inspection() {
        let workspace = workspace();
        let origin = "https://api.cantos.example";
        assert_eq!(validate_release_api_origin(origin), Ok(()));
        for invalid in ["http://127.0.0.1:8080", "https://api.cantos.example/path"] {
            assert!(validate_release_api_origin(invalid).is_err(), "{invalid}");
        }
        assert!(gradle_release_command(&workspace, Some(origin))
            .args
            .contains(&format!("-Pcantos.apiOrigin={origin}")));
        assert!(gradle_release_command(&workspace, None)
            .args
            .contains(&"-Pcantos.apiOrigin=".to_owned()));
        assert!(inspection_command(&workspace, "/sdk", Some(origin))
            .args
            .windows(2)
            .any(|pair| pair == ["--api-origin", origin]));
    }

    #[test]
    fn ios_build_links_only_shared_production_frameworks() {
        let command = gradle_ios_command(&workspace(), "/installed/kotlin-native");
        let tasks: Vec<_> = command
            .args
            .iter()
            .filter(|arg| arg.starts_with(':'))
            .collect();
        assert_eq!(
            tasks,
            [
                ":shared:linkDebugFrameworkIosSimulatorArm64",
                ":shared:linkReleaseFrameworkIosSimulatorArm64"
            ]
        );
        assert!(!command
            .args
            .iter()
            .any(|arg| arg.contains("preview-sandbox")));
    }

    #[test]
    fn debug_origin_admits_loopback_http_or_https_only() {
        for origin in [
            "http://127.0.0.1:8080",
            "http://localhost:8090",
            "https://api.cantos.example",
            "https://cantos.localhost:8443",
        ] {
            assert_eq!(validate_api_origin(origin), Ok(()), "{origin}");
        }
        for origin in [
            "",
            "127.0.0.1:8080",
            "http://127.0.0.1",
            "http://192.168.1.20:8080",
            "http://10.0.2.2:8080",
            "http://example.com:8080",
            "http://127.0.0.1:8080/api",
            "https://user@api.cantos.example",
            "https://API.cantos.example",
            "https://api.cantos.example:0",
            "https://api.cantos.example:99999",
            "https://api.cantos.example?x=1",
            "ftp://127.0.0.1:21",
            "https://.example",
        ] {
            assert!(validate_api_origin(origin).is_err(), "{origin}");
        }
    }

    #[test]
    fn inspection_reads_the_built_artifacts_against_the_sandbox_inventory() {
        let workspace = workspace();
        let command = inspection_command(&workspace, "/sdk with space", None);
        assert_eq!(command.program, "python3");
        assert_eq!(command.args[0], "scripts/inspect_android_release.py");
        let value = |flag: &str| -> Vec<&str> {
            command
                .args
                .windows(2)
                .filter(|pair| pair[0] == flag)
                .map(|pair| pair[1].as_str())
                .collect()
        };
        assert_eq!(value("--apk"), [format!("{THEATRE_MOBILE_DIR}/{APK}")]);
        assert_eq!(value("--aab"), [format!("{THEATRE_MOBILE_DIR}/{AAB}")]);
        assert_eq!(
            value("--aapt2"),
            ["/sdk with space/build-tools/35.0.0/aapt2"]
        );
        assert_eq!(value("--application-id"), [APPLICATION_ID]);
        assert_eq!(
            value("--forbidden-classes"),
            [format!("{THEATRE_MOBILE_DIR}/{SANDBOX_CLASSES}")]
        );
        assert_eq!(
            value("--forbidden-resources"),
            [format!("{THEATRE_MOBILE_DIR}/{SANDBOX_RESOURCES}")]
        );
        assert_eq!(value("--require-class"), REQUIRED_CLASSES);
        assert_eq!(value("--allowed-permission"), ALLOWED_PERMISSIONS);
        assert!(value("--allowed-permission").contains(&"android.permission.INTERNET"));
    }

    #[test]
    fn missing_host_or_sdk_is_unsupported_and_runs_nothing() {
        let dir = std::env::temp_dir().join(format!("xtask-theatre-host-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sdk = sdk_fixture("sdk");
        assert!(prerequisite(&dir, Some(&sdk))
            .unwrap_err()
            .contains("production Android host"));
        std::fs::create_dir_all(dir.join("androidApp/src/main")).unwrap();
        std::fs::create_dir_all(dir.join("shared/src/androidMain/kotlin")).unwrap();
        std::fs::write(dir.join("androidApp/build.gradle.kts"), "").unwrap();
        std::fs::write(dir.join("androidApp/src/main/AndroidManifest.xml"), "").unwrap();
        // The forbidden inventory is required: inspecting without it would be vacuous.
        assert!(prerequisite(&dir, Some(&sdk))
            .unwrap_err()
            .contains("inventory"));
        std::fs::create_dir_all(dir.join(SANDBOX_RESOURCES)).unwrap();
        assert!(prerequisite(&dir, None)
            .unwrap_err()
            .contains("ANDROID_HOME"));
        assert!(prerequisite(&dir, Some(Path::new("relative-sdk")))
            .unwrap_err()
            .contains("absolute"));
        assert_eq!(prerequisite(&dir, Some(&sdk)), Ok(()));
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::remove_dir_all(sdk).unwrap();
    }

    #[test]
    fn every_missing_tool_prevents_build_and_inspection() {
        let workspace = workspace();
        let steps = |sdk: &str| {
            vec![
                Step::new(
                    "build",
                    &tools(),
                    vec![gradle_release_command(&workspace, None)],
                ),
                Step::new(
                    "inspect",
                    &[Tool::Python3],
                    vec![inspection_command(&workspace, sdk, None)],
                ),
            ]
        };
        for tool in tools() {
            let mut runner = RecordingRunner::default();
            let report = pipeline::execute(
                "theatre-host",
                &steps("/sdk"),
                &FakeTools::without(&[tool]),
                &mut runner,
            );
            assert_eq!(report.exit_code(), ExitCode::FAILURE);
            assert!(runner.commands.is_empty());
        }
        let mut runner = RecordingRunner {
            fail_on: Some("gradle".into()),
            ..RecordingRunner::default()
        };
        let report = pipeline::execute(
            "theatre-host",
            &steps("/sdk"),
            &FakeTools::all(),
            &mut runner,
        );
        assert_eq!(report.exit_code(), ExitCode::FAILURE);
        // A failed build never lets the inspector read stale artifacts.
        assert_eq!(runner.commands.len(), 1);
        assert_eq!(runner.commands[0].program, "gradle");
    }

    #[test]
    fn debug_inspection_expects_the_dev_identity_and_release_keeps_the_production_id() {
        let workspace = workspace();
        let command = debug_inspection_command(
            &workspace,
            "/sdk",
            Path::new("/tmp/app-debug.apk"),
            "http://127.0.0.1:8080",
            None,
        );
        let at = command
            .args
            .iter()
            .position(|arg| arg == "--application-id")
            .unwrap();
        assert_eq!(command.args[at + 1], "com.cantos.theatre.dev");
        assert!(command.args.contains(
            &"com.cantos.theatre.dev.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION".to_owned()
        ));
        assert!(!command
            .args
            .contains(&"com.cantos.theatre.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION".to_owned()));
        let release = inspection_command(&workspace, "/sdk", None);
        let at = release
            .args
            .iter()
            .position(|arg| arg == "--application-id")
            .unwrap();
        assert_eq!(release.args[at + 1], APPLICATION_ID);
    }
}
