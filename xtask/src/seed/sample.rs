//! Explicit issue #427 sample import. The Server owns all schema and application effects;
//! xtask only validates the isolated destination and invokes its opt-in native fixture.

use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{anyhow, ensure, Context, Result};
use serde_json::Value;

use super::SeedOptions;
use crate::{
    build::Refusal,
    command::{CommandSpec, Runner},
    report::{Report, Status},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

const DATASET: &str = "cantos-sample-catalog-v1";
const MARKER: &str = "sample-owner.json";
const TARGET: &str = "sample_catalog_turso";

#[derive(Debug, PartialEq, Eq)]
struct Plan {
    root: PathBuf,
    input: Option<PathBuf>,
    media: Option<PathBuf>,
    serve: bool,
}

fn outside_git(path: &Path) -> Result<()> {
    ensure!(
        path.ancestors().all(|parent| !parent.join(".git").exists()),
        "sample output must be outside every Git checkout"
    );
    Ok(())
}

pub(super) fn destination(path: &Path) -> Result<PathBuf> {
    ensure!(path.is_absolute(), "--sample-root must be absolute");
    ensure!(
        path.components()
            .all(|part| !matches!(part, std::path::Component::ParentDir)),
        "--sample-root must not contain '..'"
    );
    ensure!(
        path.file_name().is_some(),
        "a dedicated sample directory is required"
    );
    if let Ok(meta) = fs::symlink_metadata(path) {
        ensure!(
            !meta.file_type().is_symlink(),
            "sample output cannot be a symbolic link"
        );
    }
    let resolved = if path.exists() {
        ensure!(
            !fs::symlink_metadata(path)?.file_type().is_symlink(),
            "sample output cannot be a symbolic link"
        );
        ensure!(path.is_dir(), "sample output must be a directory");
        path.canonicalize()?
    } else {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow!("sample parent required"))?;
        ensure!(parent.is_dir(), "create the sample parent directory first");
        parent.canonicalize()?.join(
            path.file_name()
                .ok_or_else(|| anyhow!("sample name required"))?,
        )
    };
    outside_git(&resolved)?;
    Ok(resolved)
}

/// Serialize supported imports/serve/reset even before the Server takes its database lock.
/// A sibling sidecar survives reset's directory rename and never contains dataset contents.
pub(super) fn operation_lock(root: &Path) -> Result<File> {
    let name = root
        .file_name()
        .ok_or_else(|| anyhow!("sample name missing"))?
        .to_string_lossy();
    let path = root.with_file_name(format!(".{name}.sample-owner"));
    ensure!(
        !fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()),
        "sample operation lock cannot be a symbolic link"
    );
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    file.try_lock()
        .map_err(|_| anyhow!("another sample command owns this output; stop it before retrying"))?;
    Ok(file)
}

fn plan(options: &SeedOptions) -> Result<Plan> {
    ensure!(
        !options.is_retired(),
        "retired seed environment options are not supported"
    );
    let root = destination(
        options
            .sample_root
            .as_deref()
            .ok_or_else(|| anyhow!("--sample-root is required"))?,
    )?;
    if options.reset {
        ensure!(root.is_dir(), "sample output does not exist");
        return Ok(Plan {
            root,
            input: None,
            media: None,
            serve: false,
        });
    }
    let input = options
        .sample_input
        .as_deref()
        .ok_or_else(|| anyhow!("--sample-input is required"))?
        .canonicalize()
        .context("sample input is missing")?;
    ensure!(input.is_file(), "sample input must be a file");
    let media = options
        .sample_media_source
        .as_deref()
        .ok_or_else(|| anyhow!("--sample-media-source is required"))?
        .canonicalize()
        .context("sample media source is missing")?;
    ensure!(media.is_dir(), "sample media source must be a directory");
    ensure!(
        !input.starts_with(&root) && !media.starts_with(&root) && !root.starts_with(&media),
        "sample source and output must be separate, non-overlapping directories"
    );
    Ok(Plan {
        root,
        input: Some(input),
        media: Some(media),
        serve: options.sample_serve,
    })
}

fn command(workspace: &Workspace, plan: &Plan) -> CommandSpec {
    CommandSpec::new("cargo", workspace.root())
        .args([
            "test",
            "--locked",
            "-p",
            "cantos-server",
            "--test",
            TARGET,
            "sample_catalog",
            "--",
            "--ignored",
            "--exact",
            "--nocapture",
        ])
        .env("CANTOS_SAMPLE_ROOT", plan.root.to_string_lossy())
        .env(
            "CANTOS_SAMPLE_INPUT",
            plan.input
                .as_deref()
                .unwrap_or(Path::new(""))
                .to_string_lossy(),
        )
        .env(
            "CANTOS_SAMPLE_MEDIA_SOURCE",
            plan.media
                .as_deref()
                .unwrap_or(Path::new(""))
                .to_string_lossy(),
        )
        .env("CANTOS_SAMPLE_SERVE", if plan.serve { "1" } else { "0" })
        // The isolated fixture never consumes ambient live API/auth/storage configuration.
        .unset("CANTOS_DATABASE_URL")
        .unset("CANTOS_SEED_API_ORIGIN")
        .unset("CANTOS_SEED_STUDIO_TOKEN")
}

fn archive(root: &Path) -> Result<PathBuf> {
    archive_owned(root, DATASET)
}

pub(super) fn archive_owned(root: &Path, dataset: &str) -> Result<PathBuf> {
    let marker_path = root.join(MARKER);
    ensure!(
        !fs::symlink_metadata(&marker_path)?.file_type().is_symlink(),
        "sample marker cannot be a symbolic link"
    );
    let marker: Value = serde_json::from_slice(&fs::read(&marker_path)?)?;
    ensure!(
        marker["dataset_id"] == dataset,
        "not an owned sample dataset"
    );
    ensure!(
        marker["root"].as_str() == root.to_str(),
        "sample marker belongs to another output path"
    );
    // Both artifacts can be opened by a Server owner independently. Preserve their paths
    // while either owner is active, including a consumer inspecting the snapshot.
    let mut locks = Vec::new();
    for name in ["server.db", "sample.db"] {
        let lock_path = root.join(format!("{name}.owner"));
        for path in [root.join(name), lock_path.clone()] {
            if let Ok(meta) = fs::symlink_metadata(path) {
                ensure!(
                    !meta.file_type().is_symlink(),
                    "sample database and owner lock cannot be symbolic links"
                );
            }
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let lock: File = options.open(lock_path)?;
        lock.try_lock().map_err(|_| {
            anyhow!("sample database is owned; stop its Server/import before reset")
        })?;
        locks.push(lock);
    }
    let stamp = crate::run_files::now().replace([':', '-'], "");
    let name = root
        .file_name()
        .ok_or_else(|| anyhow!("sample name missing"))?
        .to_string_lossy();
    let archived = root.with_file_name(format!("{name}.retired-{stamp}-{}", std::process::id()));
    ensure!(
        !archived.exists(),
        "sample archive destination already exists"
    );
    fs::rename(root, &archived).context("could not archive sample root")?;
    // Keep both database locks through the atomic move. No DB/WAL/media is deleted.
    drop(locks);
    Ok(archived)
}

pub(super) fn execute(
    workspace: &Workspace,
    options: &SeedOptions,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<std::result::Result<ExitCode, Refusal>> {
    let plan = match plan(options) {
        Ok(plan) => plan,
        Err(error) => return Ok(Err(Refusal::Usage(error.to_string()))),
    };
    let _operation = match operation_lock(&plan.root) {
        Ok(lock) => lock,
        Err(error) => return Ok(Err(Refusal::Usage(error.to_string()))),
    };
    let mut report = Report::new("seed theatre sample");
    if options.reset {
        match archive(&plan.root) {
            Ok(path) => report.record(
                "archive owned sample",
                Status::Pass,
                format!("preserved all files in {}", path.display()),
            ),
            Err(error) => report.record("archive owned sample", Status::Fail, error.to_string()),
        }
    } else if !tools.missing(&[Tool::Cargo]).is_empty() {
        report.record(
            "native sample fixture",
            Status::Unsupported,
            "cargo is unavailable; no sample files touched",
        );
    } else {
        let spec = command(workspace, &plan);
        match runner.run(&spec) {
            Ok(()) => report.record("native sample fixture", Status::Pass, format!("{}; draft-only metadata and owner-only artwork; public Web/CMP acceptance pending", plan.root.display())),
            Err(error) => report.record("native sample fixture", Status::Fail, error.to_string()),
        }
    }
    report.print();
    Ok(Ok(report.exit_code()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "cantos-sample-plan-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            Self(root.canonicalize().unwrap())
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn sample_plan_refuses_git_and_overlapping_sources_before_effects() {
        let temp = Root::new();
        fs::write(temp.0.join("input.json"), "{}").unwrap();
        let mut options = SeedOptions {
            sample_input: Some(temp.0.join("input.json")),
            sample_media_source: Some(temp.0.clone()),
            sample_root: Some(temp.0.join("output")),
            ..SeedOptions::default()
        };
        assert!(plan(&options)
            .unwrap_err()
            .to_string()
            .contains("non-overlapping"));
        fs::create_dir(temp.0.join(".git")).unwrap();
        options.sample_media_source = Some(temp.0.join(".git"));
        assert!(plan(&options)
            .unwrap_err()
            .to_string()
            .contains("outside every Git"));
        assert!(!temp.0.join("output").exists());
    }

    #[test]
    fn reset_refuses_an_unowned_root_and_an_active_database_then_archives_every_file() {
        let temp = Root::new();
        let root = temp.0.join("output");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("server.db"), b"preserve database").unwrap();
        fs::write(root.join("server.db-wal"), b"preserve wal").unwrap();
        assert!(archive(&root).is_err());
        fs::write(
            root.join(MARKER),
            serde_json::to_vec(&serde_json::json!({"dataset_id": DATASET, "root":root})).unwrap(),
        )
        .unwrap();
        let lock = File::create(root.join("server.db.owner")).unwrap();
        lock.try_lock().unwrap();
        assert_eq!(
            archive(&root).unwrap_err().to_string(),
            "sample database is owned; stop its Server/import before reset"
        );
        // Explicit fixture release also unlocks a descriptor inherited by a parallel fork.
        lock.unlock().unwrap();
        drop(lock);
        fs::write(root.join("sample.db"), b"preserve snapshot").unwrap();
        let snapshot_lock = File::create(root.join("sample.db.owner")).unwrap();
        snapshot_lock.try_lock().unwrap();
        assert_eq!(
            archive(&root).unwrap_err().to_string(),
            "sample database is owned; stop its Server/import before reset"
        );
        assert!(root.join("server.db").exists());
        snapshot_lock.unlock().unwrap();
        drop(snapshot_lock);
        let path = archive(&root).unwrap();
        assert!(!root.exists());
        assert_eq!(
            fs::read(path.join("server.db")).unwrap(),
            b"preserve database"
        );
        assert_eq!(
            fs::read(path.join("server.db-wal")).unwrap(),
            b"preserve wal"
        );
        assert_eq!(
            fs::read(path.join("sample.db")).unwrap(),
            b"preserve snapshot"
        );
    }

    #[test]
    fn sample_command_selects_only_the_native_ignored_fixture() {
        let workspace = Workspace::discover().unwrap();
        let plan = Plan {
            root: "/sample/output".into(),
            input: Some("/sample/source/input.json".into()),
            media: Some("/sample/source".into()),
            serve: false,
        };
        let spec = command(&workspace, &plan);
        assert_eq!(spec.cwd, workspace.root());
        assert_eq!(
            &spec.args[3..],
            [
                "cantos-server",
                "--test",
                TARGET,
                "sample_catalog",
                "--",
                "--ignored",
                "--exact",
                "--nocapture"
            ]
        );
        assert!(spec
            .env
            .iter()
            .any(|(key, value)| key == "CANTOS_SAMPLE_ROOT" && value == "/sample/output"));
        assert!(!spec.display(workspace.root()).contains("ffmpeg"));
    }

    #[test]
    fn sample_operation_lock_spans_the_output_directory_lifecycle() {
        let temp = Root::new();
        let root = temp.0.join("output");
        let first = operation_lock(&root).unwrap();
        assert!(operation_lock(&root)
            .unwrap_err()
            .to_string()
            .contains("another sample command"));
        fs::create_dir(&root).unwrap();
        fs::rename(&root, temp.0.join("archived")).unwrap();
        assert!(operation_lock(&root).is_err());
        drop(first);
        // Parallel process tests may fork while this descriptor is open. The inherited
        // descriptor retains the lock until close-on-exec, even after the parent closes it.
        // Observe release with a deadline; the live-owner refusal assertions stay immediate.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let acquired = operation_lock(&root);
            if acquired.is_ok() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "sample operation lock was not released: {acquired:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
