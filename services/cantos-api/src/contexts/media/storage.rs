//! Development-only private audio objects. PostgreSQL owns receipts and metadata, never bytes.
//!
//! Files are synchronized before metadata may commit and are never overwritten or removed by
//! a transaction rollback. A crash or rejected metadata transaction may leave a private orphan.
//! Cleanup needs a separate, reference-aware retention policy; this adapter never collects it.
//! This is not an S3/CDN implementation or a publication/delivery path.

use std::path::Path;

use crate::platform::postgres::StoreError;

use super::audio_intake::VerifiedAudio;

const MAX_AUDIO_BYTES: u64 = super::audio_intake::MAX_AUDIO_BYTES as u64;

#[derive(Clone)]
pub(crate) struct AudioStorage {
    #[cfg(unix)]
    inner: std::sync::Arc<unix::PrivateDirectories>,
}

pub(crate) struct StoredAudio {
    /// Internal content identity, not a public URL or a caller-selected filename.
    pub(crate) object_key: String,
}

impl AudioStorage {
    /// The explicit absolute root must be owned by the current Unix user and mode 0700.
    /// Only the final root directory may be created; existing permissions are never widened.
    pub(crate) fn new(root: &Path) -> Result<Self, StoreError> {
        #[cfg(unix)]
        {
            Ok(Self {
                inner: std::sync::Arc::new(unix::PrivateDirectories::new(root)?),
            })
        }
        #[cfg(not(unix))]
        {
            let _ = root;
            Err(StoreError::Unavailable)
        }
    }

    pub(crate) fn persist(&self, verified: &VerifiedAudio) -> Result<StoredAudio, StoreError> {
        #[cfg(unix)]
        {
            self.inner.persist(verified)
        }
        #[cfg(not(unix))]
        {
            let _ = verified;
            Err(StoreError::Unavailable)
        }
    }

    /// Re-open and hash the bounded stored object before returning a receipt or metadata.
    pub(crate) fn verify(
        &self,
        object_key: &str,
        sha256: &str,
        byte_length: u64,
    ) -> Result<(), StoreError> {
        #[cfg(unix)]
        {
            self.inner.verify(object_key, sha256, byte_length, None)
        }
        #[cfg(not(unix))]
        {
            let _ = (object_key, sha256, byte_length);
            Err(StoreError::Unavailable)
        }
    }
}

#[cfg(unix)]
mod unix {
    use std::{
        ffi::{CStr, CString},
        fs::File,
        io::{self, Read, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{ffi::OsStrExt, fs::MetadataExt},
        },
        path::{Component, Path},
    };

    use sha2::{Digest, Sha256};

    use super::{StoreError, StoredAudio, VerifiedAudio, MAX_AUDIO_BYTES};

    pub(super) struct PrivateDirectories {
        // Pin both directory inodes: object operations never traverse a mutable absolute path.
        root: File,
        objects: File,
    }

    impl PrivateDirectories {
        pub(super) fn new(root: &Path) -> Result<Self, StoreError> {
            if !root.is_absolute() {
                return Err(StoreError::InvalidRequest);
            }
            let mut names = Vec::new();
            for component in root.components() {
                match component {
                    Component::RootDir => {}
                    Component::Normal(name) => names.push(
                        CString::new(name.as_bytes()).map_err(|_| StoreError::InvalidRequest)?,
                    ),
                    _ => return Err(StoreError::InvalidRequest),
                }
            }
            let root_name = names.pop().ok_or(StoreError::InvalidRequest)?;
            let slash = CString::new("/").map_err(|_| StoreError::InvalidRequest)?;
            let mut parent = open_directory(libc::AT_FDCWD, &slash)
                .map_err(|_| StoreError::Unavailable)?;
            for name in names {
                parent = open_directory(parent.as_raw_fd(), &name)
                    .map_err(|_| StoreError::InvalidRequest)?;
            }
            let root = open_or_create_directory(&parent, &root_name)?;
            check_private_directory(&root).map_err(|_| StoreError::InvalidRequest)?;
            let objects_name = CString::new("objects").map_err(|_| StoreError::InvalidRequest)?;
            let objects = open_or_create_directory(&root, &objects_name)?;
            check_private_directory(&objects).map_err(|_| StoreError::InvalidRequest)?;
            Ok(Self { root, objects })
        }

        pub(super) fn persist(&self, verified: &VerifiedAudio) -> Result<StoredAudio, StoreError> {
            let metadata = verified.metadata();
            self.persist_bytes(verified.bytes(), &metadata.sha256, metadata.byte_length)
        }

        // Kept private: tests exercise storage semantics without forging audio-admission proof.
        fn persist_bytes(
            &self,
            bytes: &[u8],
            sha256: &str,
            byte_length: u64,
        ) -> Result<StoredAudio, StoreError> {
            if byte_length != bytes.len() as u64
                || byte_length == 0
                || byte_length > MAX_AUDIO_BYTES
                || !valid_digest(sha256)
                || format!("{:x}", Sha256::digest(bytes)) != sha256
            {
                return Err(StoreError::CorruptRevision);
            }
            self.check_directories()?;
            let key = sha256.to_owned();
            let object_name = CString::new(key.as_str()).map_err(|_| StoreError::CorruptRevision)?;
            let pending_name = CString::new(format!(".pending-{}", uuid::Uuid::new_v4()))
                .map_err(|_| StoreError::Unavailable)?;
            let mut pending = open_file(
                self.objects.as_raw_fd(),
                &pending_name,
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                0o600,
            )
            .map_err(|_| StoreError::Unavailable)?;
            let result = (|| {
                pending
                    .write_all(bytes)
                    .map_err(|_| StoreError::Unavailable)?;
                // Make a fully written, read-only inode durable before publishing its name.
                // SAFETY: pending owns this open descriptor; fchmod does not retain it.
                if unsafe { libc::fchmod(pending.as_raw_fd(), 0o400) } != 0 {
                    return Err(StoreError::Unavailable);
                }
                pending.sync_all().map_err(|_| StoreError::Unavailable)?;
                // linkat is atomic and fails with EEXIST rather than replacing prior bytes.
                // SAFETY: the owned directory descriptor and C strings remain valid.
                let published = unsafe {
                    libc::linkat(
                        self.objects.as_raw_fd(),
                        pending_name.as_ptr(),
                        self.objects.as_raw_fd(),
                        object_name.as_ptr(),
                        0,
                    )
                };
                if published != 0
                    && io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
                {
                    return Err(StoreError::Unavailable);
                }
                // Even a duplicate must independently verify the existing exact bytes.
                self.verify(&key, &key, byte_length, Some(bytes))?;
                self.objects
                    .sync_all()
                    .map_err(|_| StoreError::Unavailable)?;
                Ok(StoredAudio { object_key: key })
            })();
            // Delete only this request's staging name. Never unlink the immutable object name.
            // SAFETY: the descriptor and C string remain owned and valid during unlinkat.
            let removed =
                unsafe { libc::unlinkat(self.objects.as_raw_fd(), pending_name.as_ptr(), 0) };
            if removed != 0 {
                return Err(StoreError::Unavailable);
            }
            self.objects
                .sync_all()
                .map_err(|_| StoreError::Unavailable)?;
            result
        }

        pub(super) fn verify(
            &self,
            object_key: &str,
            sha256: &str,
            byte_length: u64,
            expected_bytes: Option<&[u8]>,
        ) -> Result<(), StoreError> {
            if !valid_digest(object_key)
                || object_key != sha256
                || byte_length == 0
                || byte_length > MAX_AUDIO_BYTES
            {
                return Err(StoreError::CorruptRevision);
            }
            self.check_directories()?;
            let name = CString::new(object_key).map_err(|_| StoreError::CorruptRevision)?;
            let mut file = open_file(self.objects.as_raw_fd(), &name, libc::O_RDONLY, 0)
                .map_err(object_open_error)?;
            let before = file.metadata().map_err(|_| StoreError::Unavailable)?;
            // O_NONBLOCK prevents a substituted FIFO from blocking before this type check.
            if !before.is_file()
                || before.uid() != effective_uid()
                || before.mode() & 0o7777 != 0o400
                || before.len() != byte_length
            {
                return Err(StoreError::CorruptRevision);
            }
            let mut hash = Sha256::new();
            let mut length = 0_u64;
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                let read = file.read(&mut buffer).map_err(|_| StoreError::Unavailable)?;
                if read == 0 {
                    break;
                }
                let next = length + read as u64;
                if next > byte_length || next > MAX_AUDIO_BYTES {
                    return Err(StoreError::CorruptRevision);
                }
                if let Some(expected) = expected_bytes {
                    if expected.get(length as usize..next as usize) != Some(&buffer[..read]) {
                        return Err(StoreError::CorruptRevision);
                    }
                }
                hash.update(&buffer[..read]);
                length = next;
            }
            let after = file.metadata().map_err(|_| StoreError::Unavailable)?;
            if length != byte_length
                || format!("{:x}", hash.finalize()) != sha256
                || after.len() != before.len()
                || after.mode() != before.mode()
                || after.uid() != before.uid()
                || after.mtime() != before.mtime()
                || after.mtime_nsec() != before.mtime_nsec()
            {
                return Err(StoreError::CorruptRevision);
            }
            // Also sync an equal existing object: accepting a duplicate must not assume the
            // original writer reached its durability boundary. Link/unlink changes ctime, so
            // ctime is deliberately not an integrity oracle for concurrent equal uploads.
            file.sync_all().map_err(|_| StoreError::Unavailable)?;
            let named = open_file(self.objects.as_raw_fd(), &name, libc::O_RDONLY, 0)
                .map_err(object_open_error)?;
            let named = named.metadata().map_err(|_| StoreError::Unavailable)?;
            if named.dev() != before.dev()
                || named.ino() != before.ino()
                || named.len() != after.len()
                || named.mode() != after.mode()
                || named.uid() != after.uid()
                || named.mtime() != after.mtime()
                || named.mtime_nsec() != after.mtime_nsec()
            {
                return Err(StoreError::CorruptRevision);
            }
            Ok(())
        }

        fn check_directories(&self) -> Result<(), StoreError> {
            check_private_directory(&self.root).map_err(|_| StoreError::Unavailable)?;
            check_private_directory(&self.objects).map_err(|_| StoreError::Unavailable)?;
            let name = CString::new("objects").map_err(|_| StoreError::Unavailable)?;
            let current = open_directory(self.root.as_raw_fd(), &name)
                .map_err(|_| StoreError::Unavailable)?;
            let pinned = self.objects.metadata().map_err(|_| StoreError::Unavailable)?;
            let current = current.metadata().map_err(|_| StoreError::Unavailable)?;
            if pinned.dev() != current.dev() || pinned.ino() != current.ino() {
                return Err(StoreError::Unavailable);
            }
            Ok(())
        }
    }

    fn valid_digest(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    fn effective_uid() -> u32 {
        // SAFETY: geteuid takes no arguments and has no memory access preconditions.
        unsafe { libc::geteuid() }
    }

    fn check_private_directory(directory: &File) -> io::Result<()> {
        let metadata = directory.metadata()?;
        if !metadata.is_dir()
            || metadata.uid() != effective_uid()
            || metadata.mode() & 0o7777 != 0o700
        {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        Ok(())
    }

    fn open_or_create_directory(parent: &File, name: &CStr) -> Result<File, StoreError> {
        match open_directory(parent.as_raw_fd(), name) {
            Ok(directory) => Ok(directory),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // SAFETY: parent and the NUL-terminated child name are valid for this call.
                let created = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
                if created != 0
                    && io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
                {
                    return Err(StoreError::Unavailable);
                }
                let directory = open_directory(parent.as_raw_fd(), name)
                    .map_err(|_| StoreError::InvalidRequest)?;
                parent.sync_all().map_err(|_| StoreError::Unavailable)?;
                Ok(directory)
            }
            Err(_) => Err(StoreError::InvalidRequest),
        }
    }

    fn open_directory(parent: libc::c_int, name: &CStr) -> io::Result<File> {
        open_file(parent, name, libc::O_RDONLY | libc::O_DIRECTORY, 0)
    }

    fn open_file(
        parent: libc::c_int,
        name: &CStr,
        flags: libc::c_int,
        mode: libc::mode_t,
    ) -> io::Result<File> {
        // SAFETY: the directory descriptor and NUL-terminated name are valid for openat.
        let descriptor = unsafe {
            libc::openat(
                parent,
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                mode,
            )
        };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat returned a fresh owned descriptor, now transferred exactly once.
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }

    fn object_open_error(error: io::Error) -> StoreError {
        match error.raw_os_error() {
            Some(
                libc::ENOENT
                | libc::ELOOP
                | libc::ENOTDIR
                | libc::EACCES
                | libc::EPERM
                | libc::ENXIO,
            ) => StoreError::CorruptRevision,
            _ => StoreError::Unavailable,
        }
    }

    #[cfg(test)]
    mod tests {
        use std::{
            fs::{self, Permissions},
            os::unix::fs::{symlink, PermissionsExt},
            path::PathBuf,
            sync::{Arc, Barrier},
            thread,
        };

        use super::*;

        struct Scratch(PathBuf);

        impl Scratch {
            fn new() -> Self {
                // Resolve the test parent's platform alias; the configured storage root itself
                // still must have no symlink components.
                let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
                Self(parent.join(format!("cantos-audio-storage-{}", uuid::Uuid::new_v4())))
            }

            fn storage(&self) -> PrivateDirectories {
                PrivateDirectories::new(&self.0).unwrap()
            }

            fn object(&self, digest: &str) -> PathBuf {
                self.0.join("objects").join(digest)
            }
        }

        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        fn digest(bytes: &[u8]) -> String {
            format!("{:x}", Sha256::digest(bytes))
        }

        #[test]
        fn immutable_object_survives_duplicate_and_adapter_restart() {
            let scratch = Scratch::new();
            let storage = scratch.storage();
            let bytes = b"synthetic storage boundary bytes";
            let expected = digest(bytes);
            let first = storage
                .persist_bytes(bytes, &expected, bytes.len() as u64)
                .unwrap();
            let inode = fs::metadata(scratch.object(&expected)).unwrap().ino();
            assert_eq!(first.object_key, expected);
            assert_eq!(fs::read(scratch.object(&expected)).unwrap().as_slice(), &bytes[..]);
            assert_eq!(fs::metadata(&scratch.0).unwrap().mode() & 0o7777, 0o700);
            assert_eq!(
                fs::metadata(scratch.0.join("objects")).unwrap().mode() & 0o7777,
                0o700
            );
            assert_eq!(
                fs::metadata(scratch.object(&expected)).unwrap().mode() & 0o7777,
                0o400
            );
            drop(storage);
            let restarted = scratch.storage();
            let second = restarted
                .persist_bytes(bytes, &expected, bytes.len() as u64)
                .unwrap();
            assert_eq!(first.object_key, second.object_key);
            assert_eq!(fs::metadata(scratch.object(&expected)).unwrap().ino(), inode);
            assert_eq!(fs::read_dir(scratch.0.join("objects")).unwrap().count(), 1);
            restarted
                .verify(&expected, &expected, bytes.len() as u64, None)
                .unwrap();
        }

        #[test]
        fn concurrent_equal_uploads_preserve_one_complete_object() {
            let scratch = Scratch::new();
            let storage = Arc::new(scratch.storage());
            let bytes = vec![42_u8; 1024 * 1024];
            let expected = digest(&bytes);
            let barrier = Arc::new(Barrier::new(4));
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let storage = storage.clone();
                    let bytes = bytes.clone();
                    let expected = expected.clone();
                    let barrier = barrier.clone();
                    thread::spawn(move || {
                        barrier.wait();
                        storage.persist_bytes(&bytes, &expected, bytes.len() as u64)
                    })
                })
                .collect();
            for handle in handles {
                assert_eq!(handle.join().unwrap().unwrap().object_key, expected);
            }
            assert_eq!(fs::read(scratch.object(&expected)).unwrap(), bytes);
            assert_eq!(fs::read_dir(scratch.0.join("objects")).unwrap().count(), 1);
        }

        #[test]
        fn tampered_existing_bytes_are_rejected_and_never_replaced() {
            let scratch = Scratch::new();
            let storage = scratch.storage();
            let bytes = b"accepted object";
            let expected = digest(bytes);
            storage
                .persist_bytes(bytes, &expected, bytes.len() as u64)
                .unwrap();
            let path = scratch.object(&expected);
            fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
            fs::write(&path, b"tampered object").unwrap();
            fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
            assert!(matches!(
                storage.verify(&expected, &expected, bytes.len() as u64, None),
                Err(StoreError::CorruptRevision)
            ));
            assert!(matches!(
                storage.persist_bytes(bytes, &expected, bytes.len() as u64),
                Err(StoreError::CorruptRevision)
            ));
            assert_eq!(fs::read(&path).unwrap().as_slice(), b"tampered object");
            assert_eq!(fs::read_dir(scratch.0.join("objects")).unwrap().count(), 1);
        }

        #[test]
        fn missing_truncated_writable_and_symlink_objects_fail_closed() {
            let scratch = Scratch::new();
            let storage = scratch.storage();
            let bytes = b"synthetic object";
            let expected = digest(bytes);
            let path = scratch.object(&expected);
            assert!(matches!(
                storage.verify(&expected, &expected, bytes.len() as u64, None),
                Err(StoreError::CorruptRevision)
            ));
            storage
                .persist_bytes(bytes, &expected, bytes.len() as u64)
                .unwrap();
            fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
            assert!(matches!(
                storage.verify(&expected, &expected, bytes.len() as u64, None),
                Err(StoreError::CorruptRevision)
            ));
            fs::write(&path, &bytes[..4]).unwrap();
            fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
            assert!(matches!(
                storage.verify(&expected, &expected, bytes.len() as u64, None),
                Err(StoreError::CorruptRevision)
            ));
            fs::remove_file(&path).unwrap();
            let outside = scratch.0.join("outside");
            fs::write(&outside, bytes).unwrap();
            symlink(&outside, &path).unwrap();
            assert!(matches!(
                storage.verify(&expected, &expected, bytes.len() as u64, None),
                Err(StoreError::CorruptRevision)
            ));
        }

        #[test]
        fn invalid_keys_and_lengths_never_select_a_filesystem_path() {
            let scratch = Scratch::new();
            let storage = scratch.storage();
            let expected = digest(b"synthetic object");
            let uppercase = expected.to_uppercase();
            for key in ["../outside", "/tmp/outside", "objects/key", &uppercase] {
                assert!(matches!(
                    storage.verify(key, key, 1, None),
                    Err(StoreError::CorruptRevision)
                ));
            }
            for length in [0, MAX_AUDIO_BYTES + 1, u64::MAX] {
                assert!(matches!(
                    storage.verify(&expected, &expected, length, None),
                    Err(StoreError::CorruptRevision)
                ));
            }
            assert!(matches!(
                storage.persist_bytes(b"wrong digest", &expected, 12),
                Err(StoreError::CorruptRevision)
            ));
            assert_eq!(fs::read_dir(scratch.0.join("objects")).unwrap().count(), 0);
        }

        #[test]
        fn root_and_objects_must_be_private_owned_directories_without_symlinks() {
            assert!(matches!(
                PrivateDirectories::new(Path::new("relative-root")),
                Err(StoreError::InvalidRequest)
            ));
            let scratch = Scratch::new();
            fs::create_dir(&scratch.0).unwrap();
            fs::set_permissions(&scratch.0, Permissions::from_mode(0o755)).unwrap();
            assert!(matches!(
                PrivateDirectories::new(&scratch.0),
                Err(StoreError::InvalidRequest)
            ));
            fs::set_permissions(&scratch.0, Permissions::from_mode(0o700)).unwrap();
            let linked = scratch.0.join("linked-root");
            symlink(&scratch.0, &linked).unwrap();
            assert!(matches!(
                PrivateDirectories::new(&linked),
                Err(StoreError::InvalidRequest)
            ));
            assert!(matches!(
                PrivateDirectories::new(&linked.join("child")),
                Err(StoreError::InvalidRequest)
            ));
            symlink(&scratch.0, scratch.0.join("objects")).unwrap();
            assert!(matches!(
                PrivateDirectories::new(&scratch.0),
                Err(StoreError::InvalidRequest)
            ));
        }

        #[test]
        fn replacing_the_objects_directory_cannot_redirect_pinned_storage() {
            let scratch = Scratch::new();
            let storage = scratch.storage();
            fs::rename(scratch.0.join("objects"), scratch.0.join("original-objects")).unwrap();
            fs::create_dir(scratch.0.join("objects")).unwrap();
            fs::set_permissions(scratch.0.join("objects"), Permissions::from_mode(0o700)).unwrap();
            assert!(matches!(
                storage.persist_bytes(b"bytes", &digest(b"bytes"), 5),
                Err(StoreError::Unavailable)
            ));
            assert_eq!(fs::read_dir(scratch.0.join("objects")).unwrap().count(), 0);
            assert_eq!(
                fs::read_dir(scratch.0.join("original-objects")).unwrap().count(),
                0
            );
        }

        #[test]
        fn partial_private_staging_orphans_are_never_accepted_or_collected_on_restart() {
            let scratch = Scratch::new();
            let storage = scratch.storage();
            let bytes = b"complete object";
            let expected = digest(bytes);
            let orphan = scratch.0.join("objects/.pending-crashed");
            fs::write(&orphan, &bytes[..4]).unwrap();
            fs::set_permissions(&orphan, Permissions::from_mode(0o600)).unwrap();
            drop(storage);
            let restarted = scratch.storage();
            assert!(matches!(
                restarted.verify(&expected, &expected, bytes.len() as u64, None),
                Err(StoreError::CorruptRevision)
            ));
            restarted
                .persist_bytes(bytes, &expected, bytes.len() as u64)
                .unwrap();
            assert_eq!(fs::read(&orphan).unwrap().as_slice(), &bytes[..4]);
            assert_eq!(fs::read_dir(scratch.0.join("objects")).unwrap().count(), 2);
        }

        #[test]
        fn substituted_writerless_fifo_and_directory_are_rejected_without_reading() {
            let scratch = Scratch::new();
            let storage = scratch.storage();
            let expected = digest(b"synthetic object");
            let path = scratch.object(&expected);
            let name = CString::new(path.as_os_str().as_bytes()).unwrap();
            // SAFETY: name is a valid NUL-terminated test-local path.
            assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o400) }, 0);
            assert!(matches!(
                storage.verify(&expected, &expected, 16, None),
                Err(StoreError::CorruptRevision)
            ));
            fs::remove_file(&path).unwrap();
            fs::create_dir(&path).unwrap();
            assert!(matches!(
                storage.verify(&expected, &expected, 16, None),
                Err(StoreError::CorruptRevision)
            ));
        }
    }
}
