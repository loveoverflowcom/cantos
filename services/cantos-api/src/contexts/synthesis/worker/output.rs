//! Private create-only worker journals and bounded, descriptor-relative output recovery.
use super::WorkerError;
use crate::contexts::synthesis::media::audio::MAX_OUTPUT_BYTES;
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub(crate) fn prepare_root(path: &Path) -> Result<PathBuf, WorkerError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| WorkerError::UnsafeOutput)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(WorkerError::UnsafeOutput);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(WorkerError::UnsafeOutput);
        }
    }
    fs::canonicalize(path).map_err(|_| WorkerError::UnsafeOutput)
}

pub(super) fn create_attempt_directory(root: &Path, attempt: &str) -> Result<PathBuf, WorkerError> {
    if uuid::Uuid::parse_str(attempt).is_err() {
        return Err(WorkerError::UnsafeOutput);
    }
    let directory = root.join(attempt);
    create_private_directory(&directory)?;
    Ok(directory)
}

pub(super) fn create_private_directory(path: &Path) -> Result<(), WorkerError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|_| WorkerError::OutputIo)?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

pub(super) fn ensure_private_directory(path: &Path) -> Result<(), WorkerError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => {
            if let Some(parent) = path.parent() {
                sync_directory(parent)?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(WorkerError::OutputIo),
    }
    // Two independent workers may race to initialize this fixed provider ledger directory.
    prepare_root(path).map(|_| ())
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), WorkerError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path).map_err(|_| WorkerError::OutputIo)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| WorkerError::OutputIo)?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

pub(super) fn write_new_json(path: &Path, value: &impl Serialize) -> Result<(), WorkerError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| WorkerError::OutputIo)?;
    write_new(path, &bytes)
}

fn sync_directory(path: &Path) -> Result<(), WorkerError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| WorkerError::OutputIo)
}

pub(crate) fn read_output(
    root: &Path,
    recorded: &str,
    expected_attempt: &str,
) -> Result<Vec<u8>, WorkerError> {
    let path = Path::new(recorded)
        .strip_prefix(root)
        .map_err(|_| WorkerError::UnsafeOutput)?;
    let components: Vec<_> = path.components().collect();
    if components.len() != 2
        || !matches!(components[0], Component::Normal(_))
        || !matches!(components[1], Component::Normal(name) if name == "output.wav")
    {
        return Err(WorkerError::UnsafeOutput);
    }
    let Some(attempt) = components[0].as_os_str().to_str() else {
        return Err(WorkerError::UnsafeOutput);
    };
    if uuid::Uuid::parse_str(attempt).is_err() || attempt != expected_attempt {
        return Err(WorkerError::UnsafeOutput);
    }
    let file = open_private_output(root, attempt)?;
    let metadata = file.metadata().map_err(|_| WorkerError::OutputIo)?;
    if !metadata.is_file() || metadata.len() > MAX_OUTPUT_BYTES as u64 {
        return Err(WorkerError::UnsafeOutput);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(WorkerError::UnsafeOutput);
        }
    }
    let mut bytes = Vec::new();
    file.take(MAX_OUTPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| WorkerError::OutputIo)?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(WorkerError::UnsafeOutput);
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_private_output(root: &Path, attempt: &str) -> Result<File, WorkerError> {
    use std::{
        ffi::CString,
        os::fd::{AsRawFd, FromRawFd},
        os::unix::fs::OpenOptionsExt,
    };
    let root = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(root)
        .map_err(private_open_error)?;
    let attempt = CString::new(attempt).map_err(|_| WorkerError::UnsafeOutput)?;
    // Directory descriptors prevent an ancestor replacement from redirecting the open.
    let descriptor = unsafe {
        libc::openat(
            root.as_raw_fd(),
            attempt.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        return Err(private_open_error(std::io::Error::last_os_error()));
    }
    let directory = unsafe { File::from_raw_fd(descriptor) };
    let descriptor = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            c"output.wav".as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        return Err(private_open_error(std::io::Error::last_os_error()));
    }
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

#[cfg(unix)]
fn private_open_error(error: std::io::Error) -> WorkerError {
    if matches!(error.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) {
        WorkerError::UnsafeOutput
    } else {
        // Missing bytes, permission denial and I/O faults are unavailable, not proof of
        // malformed audio. Never preserve the OS message, which can include private paths.
        WorkerError::OutputIo
    }
}

#[cfg(not(unix))]
fn open_private_output(_root: &Path, _attempt: &str) -> Result<File, WorkerError> {
    // The local adapter supports Unix only; a weaker pathname open cannot prove equivalence.
    Err(WorkerError::UnsafeOutput)
}

pub(super) fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}
