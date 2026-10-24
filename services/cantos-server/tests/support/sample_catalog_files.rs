//! Crash-safe files for the explicit sample fixture. The xtask operation lock serializes
//! callers. Incomplete staging names are never adopted or removed by a later invocation.
//! Existing final files must match exactly; mutable journals additionally require their
//! previously validated bytes before atomic replacement.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::unix::{ffi::OsStrExt, fs::DirBuilderExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
};

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn parent(path: &Path) -> io::Result<&Path> {
    path.parent()
        .ok_or_else(|| invalid("sample file parent is missing"))
}

fn sync_parent(path: &Path) -> io::Result<()> {
    File::open(parent(path)?)?.sync_all()
}

fn read_regular(path: &Path) -> io::Result<Vec<u8>> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Err(invalid("sample target is not a regular file"));
    }
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(libc::O_NOFOLLOW);
    let mut file = options.open(path)?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut bytes)?;
    Ok(bytes)
}

fn require_bytes(path: &Path, expected: &[u8]) -> io::Result<()> {
    if read_regular(path)? != expected {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "existing sample file differs; retained without modification",
        ));
    }
    Ok(())
}

fn staging_name(path: &Path) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| invalid("sample file name is missing"))?;
    let mut stage = std::ffi::OsString::from(".");
    stage.push(name);
    stage.push(format!(".sample-stage-{}", uuid::Uuid::new_v4()));
    Ok(parent(path)?.join(stage))
}

fn stage_with(
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
) -> io::Result<PathBuf> {
    let stage = staging_name(path)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&stage)?;
    // On write/sync failure leave this uniquely created staging file as evidence. A retry
    // starts from trusted input in a different name; it never adopts a possibly foreign stage.
    write(&mut file, bytes)?;
    file.sync_all()?;
    drop(file);
    Ok(stage)
}

fn write_once_with(
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => return require_bytes(path, bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let stage = stage_with(path, bytes, write)?;
    // A hard link atomically publishes complete, synced bytes without replacing a final name.
    // Both names are on the same filesystem and this invocation alone owns the new stage.
    match fs::hard_link(&stage, path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            require_bytes(path, bytes)?;
        }
        Err(error) => return Err(error),
    }
    fs::remove_file(stage)?;
    sync_parent(path)
}

pub fn write_once(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_once_with(path, bytes, |file, bytes| file.write_all(bytes))
}

/// Only for a marker-bound mutable journal whose prior bytes were validated by its owner.
/// The enclosing xtask operation lock must remain held through this operation.
pub fn replace_owned(path: &Path, expected: &[u8], bytes: &[u8]) -> io::Result<()> {
    require_bytes(path, expected)?;
    let stage = stage_with(path, bytes, |file, bytes| file.write_all(bytes))?;
    require_bytes(path, expected)?;
    fs::rename(stage, path)?;
    sync_parent(path)
}

fn rename_directory_exclusive(source: &Path, destination: &Path) -> io::Result<()> {
    use std::ffi::CString;
    let source =
        CString::new(source.as_os_str().as_bytes()).map_err(|_| invalid("invalid staging path"))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| invalid("invalid output path"))?;
    #[cfg(target_os = "macos")]
    // SAFETY: both C strings remain valid during the call; EXCL forbids replacing any target.
    let result =
        unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
    #[cfg(target_os = "linux")]
    // SAFETY: both C strings remain valid during the call; NOREPLACE preserves foreign targets.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "sample root publication needs an exclusive atomic directory rename",
    ));
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn initialize_root_with(
    root: &Path,
    marker: &[u8],
    write: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    match fs::symlink_metadata(root) {
        Ok(metadata) => {
            if !metadata.file_type().is_dir() {
                return Err(invalid("sample output is not a real directory"));
            }
            return require_bytes(&root.join("sample-owner.json"), marker);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let stage = staging_name(root)?;
    fs::DirBuilder::new().mode(0o700).create(&stage)?;
    // The output directory itself becomes visible only after its complete ownership marker.
    // A crash before that leaves only a unique sibling stage, which is never adopted later.
    write_once_with(&stage.join("sample-owner.json"), marker, write)?;
    File::open(&stage)?.sync_all()?;
    rename_directory_exclusive(&stage, root)?;
    sync_parent(root)
}

pub fn initialize_root(root: &Path, marker: &[u8]) -> io::Result<()> {
    initialize_root_with(root, marker, |file, bytes| file.write_all(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_store_turso::testing::TestRun;

    fn partial(file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(&bytes[..bytes.len() / 2])?;
        file.sync_all()?;
        Err(io::Error::from_raw_os_error(libc::ENOSPC))
    }

    #[test]
    fn partial_cover_write_never_publishes_a_truncated_final_or_adopts_foreign_staging() {
        let run = TestRun::new("sample-atomic-cover");
        let final_path = run.path("cover.png");
        let foreign = run.path(".cover.png.sample-stage-foreign");
        fs::write(&foreign, b"foreign evidence").unwrap();
        assert_eq!(
            write_once_with(&final_path, b"complete image bytes", partial)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::ENOSPC)
        );
        assert!(!final_path.exists());
        let stages: Vec<_> = fs::read_dir(run.dir())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        write_once(&final_path, b"complete image bytes").unwrap();
        write_once(&final_path, b"complete image bytes").unwrap();
        assert_eq!(fs::read(&final_path).unwrap(), b"complete image bytes");
        assert_eq!(fs::read(&foreign).unwrap(), b"foreign evidence");
        assert!(
            stages.iter().all(|path| path.exists()),
            "retry must retain unknown prior staging"
        );
        assert_eq!(
            write_once(&final_path, b"different").unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(final_path).unwrap(), b"complete image bytes");
    }

    #[test]
    fn interrupted_marker_publication_keeps_root_absent_then_retries_without_adoption() {
        let run = TestRun::new("sample-atomic-marker");
        let root = run.path("output");
        let marker = br#"{"dataset":"sample","binding":"immutable"}"#;
        assert_eq!(
            initialize_root_with(&root, marker, partial)
                .unwrap_err()
                .raw_os_error(),
            Some(libc::ENOSPC)
        );
        assert!(!root.exists());
        let stages: Vec<_> = fs::read_dir(run.dir())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        initialize_root(&root, marker).unwrap();
        initialize_root(&root, marker).unwrap();
        assert_eq!(fs::read(root.join("sample-owner.json")).unwrap(), marker);
        assert!(stages.iter().all(|path| path.exists()));
        assert!(initialize_root(&root, b"foreign marker").is_err());
        assert_eq!(fs::read(root.join("sample-owner.json")).unwrap(), marker);
    }

    #[test]
    fn exclusive_root_publication_and_files_preserve_foreign_targets_and_symlinks() {
        let run = TestRun::new("sample-atomic-foreign");
        let root = run.path("foreign");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("evidence"), b"retain").unwrap();
        assert!(initialize_root(&root, b"new marker").is_err());
        assert_eq!(fs::read(root.join("evidence")).unwrap(), b"retain");
        let stage = run.path("owned-stage");
        fs::create_dir(&stage).unwrap();
        let empty_foreign = run.path("empty-foreign");
        fs::create_dir(&empty_foreign).unwrap();
        assert_eq!(
            rename_directory_exclusive(&stage, &empty_foreign)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(stage.exists() && empty_foreign.exists());
        let link = run.path("link");
        std::os::unix::fs::symlink(root.join("evidence"), &link).unwrap();
        assert!(write_once(&link, b"retain").is_err());
        assert_eq!(fs::read(root.join("evidence")).unwrap(), b"retain");
    }

    #[test]
    fn journal_replacement_requires_exact_prior_owned_bytes() {
        let run = TestRun::new("sample-atomic-journal");
        let journal = run.path("journal.json");
        write_once(&journal, b"attempt zero").unwrap();
        assert_eq!(
            replace_owned(&journal, b"foreign", b"attempt one")
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&journal).unwrap(), b"attempt zero");
        replace_owned(&journal, b"attempt zero", b"attempt one").unwrap();
        assert_eq!(fs::read(journal).unwrap(), b"attempt one");
    }
}
