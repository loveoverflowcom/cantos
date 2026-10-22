//! Development-only private verified-media objects. PostgreSQL owns metadata, never bytes.
//!
//! Files are synchronized before metadata may commit and are never overwritten or removed by
//! a transaction rollback. A crash or rejected metadata transaction may leave a private orphan.
//! Cleanup needs a separate, reference-aware retention policy; this adapter never collects it.
//! This filesystem adapter implements private intake and authorized reads, never publication.

use std::{
    path::Path,
    sync::{Arc, LazyLock},
};

use tokio::sync::Semaphore;

use crate::platform::store::StoreError;

use super::audio_intake::VerifiedAudio;
use super::domain::object_storage::{
    NormalizedByteRange, ObjectIntegrity, ObjectKey, StorageError, MAX_OBJECT_BYTES,
};
use super::image_intake::VerifiedImage;

mod config;
pub mod port;
pub(crate) mod registry;
mod s3;
mod sigv4;

use port::{PrivateStorage, StorageFuture, StorageRead, VerifiedPayload};

// Hashing and durable writes are bounded independently of decoder admission. The blocking
// closure owns its permit, so dropping its caller never admits a replacement while it runs.
static IO_LANES: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));

pub(super) fn bounded_filesystem_io<T: Send + 'static>(
    lanes: Arc<Semaphore>,
    operation: impl FnOnce() -> Result<T, StorageError> + Send + 'static,
) -> StorageFuture<T> {
    Box::pin(async move {
        let permit = lanes
            .try_acquire_owned()
            .map_err(|_| StorageError::Unavailable)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            operation()
        })
        .await
        .map_err(|_| StorageError::Unavailable)?
    })
}

/// Development filesystem target for every verified private payload (audio and image alike).
#[derive(Clone)]
pub(crate) struct FilesystemStorage {
    #[cfg(unix)]
    inner: std::sync::Arc<unix::ObjectDirectories>,
    lanes: Arc<Semaphore>,
}

impl PrivateStorage for FilesystemStorage {
    fn put_if_absent(&self, payload: VerifiedPayload) -> StorageFuture<()> {
        let storage = self.clone();
        bounded_filesystem_io(storage.lanes.clone(), move || {
            let stored = match &payload {
                VerifiedPayload::Audio(audio) => storage.persist_audio(audio),
                VerifiedPayload::Image(image) => storage.persist_image(image),
            }
            .map_err(storage_error)?;
            if stored.object_key != payload.sha256() {
                return Err(StorageError::DigestMismatch);
            }
            Ok(())
        })
    }

    fn verify(&self, key: ObjectKey, integrity: ObjectIntegrity) -> StorageFuture<()> {
        let storage = self.clone();
        bounded_filesystem_io(storage.lanes.clone(), move || {
            storage.verified_file(&key, &integrity).map(|_| ())
        })
    }

    fn open_verified(
        &self,
        key: ObjectKey,
        integrity: ObjectIntegrity,
        range: Option<NormalizedByteRange>,
    ) -> StorageFuture<StorageRead> {
        let storage = self.clone();
        Box::pin(async move {
            use futures_util::StreamExt;
            use std::io::{Seek, SeekFrom};
            use tokio::io::AsyncReadExt;
            use tokio_util::io::ReaderStream;

            let range = range
                .map(Ok)
                .unwrap_or_else(|| {
                    crate::contexts::media::domain::object_storage::NormalizedByteRange::full(
                        integrity.byte_length(),
                    )
                })
                .map_err(|_| StorageError::InvalidIntegrity)?;
            if range.total_byte_length() != integrity.byte_length() {
                return Err(StorageError::LengthMismatch);
            }
            let file = bounded_filesystem_io(storage.lanes.clone(), move || {
                // Hash, metadata checks and named-inode verification retain this exact handle.
                // Never verify-close-reopen: the name could select a different inode afterward.
                let mut file = storage.verified_file(&key, &integrity)?;
                file.seek(SeekFrom::Start(range.first()))
                    .map_err(|_| StorageError::Unavailable)?;
                Ok::<_, StorageError>(file)
            })
            .await?;
            let reader = tokio::fs::File::from_std(file).take(range.byte_length());
            let stream = ReaderStream::with_capacity(reader, 16 * 1024)
                .map(|chunk| chunk.map_err(|_| StorageError::Unavailable));
            Ok(StorageRead {
                stream: Box::pin(stream),
                byte_length: range.byte_length(),
            })
        })
    }
}

/// The original synchronous compatibility surface uses the shared StoreError vocabulary.
/// New capability consumers receive the storage port's typed failures directly.
pub(super) fn storage_error(error: StoreError) -> StorageError {
    match error {
        StoreError::Storage(error) => error,
        StoreError::NotFound => StorageError::NotFound,
        StoreError::Forbidden | StoreError::Unauthenticated => StorageError::Forbidden,
        StoreError::InvalidRequest | StoreError::InvalidRequestFields(_) => {
            StorageError::InvalidIntegrity
        }
        StoreError::CorruptRevision => StorageError::DigestMismatch,
        StoreError::OperationReused => StorageError::Conflict,
        StoreError::Unavailable
        | StoreError::InvalidScript(_)
        | StoreError::EvidenceUnavailable
        | StoreError::StaleRevision(_)
        | StoreError::ProposalAlreadySubmitted
        | StoreError::StaleProductionInputs
        | StoreError::ProductionBlocked(_)
        | StoreError::AudioRejected(_)
        | StoreError::AudioInspectionUnavailable(_) => StorageError::Unavailable,
    }
}

/// A durably written private object of any verified media kind.
pub(crate) struct StoredObject {
    /// Internal content identity, not a public URL or a caller-selected filename.
    pub(crate) object_key: String,
}

impl FilesystemStorage {
    /// The explicit absolute root must be owned by the current Unix user and mode 0700.
    /// Only the final root directory may be created; existing permissions are never widened.
    pub(crate) fn new(root: &Path) -> Result<Self, StoreError> {
        Self::with_lanes(root, IO_LANES.clone())
    }

    /// Independent test lanes exercise the actual fail-fast bound without competing with
    /// unrelated parallel tests for the process-wide production lanes.
    #[cfg(test)]
    pub(crate) fn with_test_lanes(root: &Path, lanes: Arc<Semaphore>) -> Result<Self, StoreError> {
        Self::with_lanes(root, lanes)
    }

    fn with_lanes(root: &Path, lanes: Arc<Semaphore>) -> Result<Self, StoreError> {
        #[cfg(unix)]
        {
            Ok(Self {
                inner: std::sync::Arc::new(unix::ObjectDirectories::new(root)?),
                lanes,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = (root, lanes);
            Err(StoreError::Unavailable)
        }
    }

    pub(crate) fn persist_audio(
        &self,
        verified: &VerifiedAudio,
    ) -> Result<StoredObject, StoreError> {
        #[cfg(unix)]
        {
            self.inner.persist_audio(verified)
        }
        #[cfg(not(unix))]
        {
            let _ = verified;
            Err(StoreError::Unavailable)
        }
    }

    fn persist_image(&self, verified: &VerifiedImage) -> Result<StoredObject, StoreError> {
        #[cfg(unix)]
        {
            self.inner.persist_image(verified)
        }
        #[cfg(not(unix))]
        {
            let _ = verified;
            Err(StoreError::Unavailable)
        }
    }

    fn verified_file(
        &self,
        key: &ObjectKey,
        integrity: &ObjectIntegrity,
    ) -> Result<std::fs::File, StorageError> {
        if key.as_str() != integrity.sha256() {
            return Err(StorageError::DigestMismatch);
        }
        #[cfg(unix)]
        {
            self.inner.verified_file(
                key.as_str(),
                integrity.sha256(),
                integrity.byte_length(),
                None,
            )
        }
        #[cfg(not(unix))]
        {
            Err(StorageError::Unavailable)
        }
    }
}

#[cfg(unix)]
pub(super) mod unix {
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

    use super::{
        storage_error, StorageError, StoreError, StoredObject, VerifiedAudio, VerifiedImage,
        MAX_OBJECT_BYTES,
    };

    pub(crate) struct ObjectDirectories {
        // Pin both directory inodes: object operations never traverse a mutable absolute path.
        root: File,
        objects: File,
    }

    impl ObjectDirectories {
        pub(crate) fn new(root: &Path) -> Result<Self, StoreError> {
            Self::for_class(root, false)
        }

        pub(crate) fn publication(root: &Path) -> Result<Self, StoreError> {
            Self::for_class(root, true)
        }

        fn for_class(root: &Path, publication: bool) -> Result<Self, StoreError> {
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
            let mut parent =
                open_directory(libc::AT_FDCWD, &slash).map_err(|_| StoreError::Unavailable)?;
            for name in names {
                parent = open_directory(parent.as_raw_fd(), &name)
                    .map_err(|_| StoreError::InvalidRequest)?;
            }
            let root = open_or_create_directory(&parent, &root_name)?;
            check_private_directory(&root).map_err(|_| StoreError::InvalidRequest)?;
            // One atomically created class marker prevents concurrent constructors from
            // admitting the same empty root into both capabilities. Unmarked legacy private
            // roots may be adopted only by the private adapter.
            let class = if publication {
                b"publication".as_slice()
            } else {
                b"private".as_slice()
            };
            let marker_name =
                CString::new(".cantos-storage-class-v1").map_err(|_| StoreError::InvalidRequest)?;
            let marker = match open_file(root.as_raw_fd(), &marker_name, libc::O_RDONLY, 0) {
                Ok(marker) => marker,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    if publication {
                        let objects =
                            CString::new("objects").map_err(|_| StoreError::InvalidRequest)?;
                        match open_directory(root.as_raw_fd(), &objects) {
                            Ok(_) => return Err(StoreError::InvalidRequest),
                            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                            Err(_) => return Err(StoreError::InvalidRequest),
                        }
                    }
                    match open_file(
                        root.as_raw_fd(),
                        &marker_name,
                        libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                        0o600,
                    ) {
                        Ok(mut created) => {
                            created
                                .write_all(class)
                                .map_err(|_| StoreError::Unavailable)?;
                            // SAFETY: created owns this descriptor for the entire call.
                            if unsafe { libc::fchmod(created.as_raw_fd(), 0o400) } != 0 {
                                return Err(StoreError::Unavailable);
                            }
                            created.sync_all().map_err(|_| StoreError::Unavailable)?;
                            root.sync_all().map_err(|_| StoreError::Unavailable)?;
                        }
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                        Err(_) => return Err(StoreError::InvalidRequest),
                    }
                    open_file(root.as_raw_fd(), &marker_name, libc::O_RDONLY, 0)
                        .map_err(|_| StoreError::InvalidRequest)?
                }
                Err(_) => return Err(StoreError::InvalidRequest),
            };
            let metadata = marker.metadata().map_err(|_| StoreError::Unavailable)?;
            let mut contents = Vec::new();
            if !metadata.is_file()
                || metadata.uid() != effective_uid()
                || metadata.mode() & 0o7777 != 0o400
                || metadata.len() != class.len() as u64
            {
                return Err(StoreError::InvalidRequest);
            }
            marker
                .take(12)
                .read_to_end(&mut contents)
                .map_err(|_| StoreError::Unavailable)?;
            if contents != class {
                return Err(StoreError::InvalidRequest);
            }
            let objects_name = CString::new("objects").map_err(|_| StoreError::InvalidRequest)?;
            let objects = open_or_create_directory(&root, &objects_name)?;
            check_private_directory(&objects).map_err(|_| StoreError::InvalidRequest)?;
            Ok(Self { root, objects })
        }

        pub(crate) fn persist_audio(
            &self,
            verified: &VerifiedAudio,
        ) -> Result<StoredObject, StoreError> {
            let metadata = verified.metadata();
            self.persist_bytes(verified.bytes(), &metadata.sha256, metadata.byte_length)
        }

        pub(super) fn persist_image(
            &self,
            verified: &VerifiedImage,
        ) -> Result<StoredObject, StoreError> {
            let metadata = verified.metadata();
            self.persist_bytes(verified.bytes(), metadata.sha256(), metadata.byte_length())
        }

        // Kept private: tests exercise storage semantics without forging media-admission proof.
        #[cfg(test)]
        pub(crate) fn inject_test_bytes(
            &self,
            bytes: &[u8],
            sha: &str,
        ) -> Result<StoredObject, StoreError> {
            self.persist_bytes(bytes, sha, bytes.len() as u64)
        }

        fn persist_bytes(
            &self,
            bytes: &[u8],
            sha256: &str,
            byte_length: u64,
        ) -> Result<StoredObject, StoreError> {
            if byte_length != bytes.len() as u64
                || byte_length == 0
                || byte_length > MAX_OBJECT_BYTES
                || !valid_digest(sha256)
                || format!("{:x}", Sha256::digest(bytes)) != sha256
            {
                return Err(StoreError::CorruptRevision);
            }
            self.check_directories()?;
            let key = sha256.to_owned();
            let object_name =
                CString::new(key.as_str()).map_err(|_| StoreError::CorruptRevision)?;
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
                if published != 0 && io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
                {
                    return Err(StoreError::Unavailable);
                }
                // Even a duplicate must independently verify the existing exact bytes.
                self.verify(&key, &key, byte_length, Some(bytes))?;
                self.objects
                    .sync_all()
                    .map_err(|_| StoreError::Unavailable)?;
                Ok(StoredObject { object_key: key })
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
            self.verified_file(object_key, sha256, byte_length, expected_bytes)
                .map(|_| ())
                .map_err(|error| match error {
                    StorageError::Unavailable | StorageError::Timeout | StorageError::Cancelled => {
                        StoreError::Unavailable
                    }
                    StorageError::InvalidTarget
                    | StorageError::InvalidKey
                    | StorageError::InvalidIntegrity
                    | StorageError::DigestMismatch
                    | StorageError::LengthMismatch
                    | StorageError::Conflict
                    | StorageError::NotFound
                    | StorageError::Forbidden
                    | StorageError::UnsupportedCapability(_) => StoreError::CorruptRevision,
                })
        }

        pub(crate) fn verified_file(
            &self,
            object_key: &str,
            sha256: &str,
            byte_length: u64,
            expected_bytes: Option<&[u8]>,
        ) -> Result<File, StorageError> {
            if !valid_digest(object_key)
                || object_key != sha256
                || byte_length == 0
                || byte_length > MAX_OBJECT_BYTES
            {
                return Err(StorageError::InvalidIntegrity);
            }
            self.check_directories().map_err(storage_error)?;
            let name = CString::new(object_key).map_err(|_| StorageError::InvalidKey)?;
            let mut file = open_file(self.objects.as_raw_fd(), &name, libc::O_RDONLY, 0)
                .map_err(object_open_error)?;
            let before = file.metadata().map_err(|_| StorageError::Unavailable)?;
            // O_NONBLOCK prevents a substituted FIFO from blocking before this type check.
            if !before.is_file()
                || before.uid() != effective_uid()
                || before.mode() & 0o7777 != 0o400
            {
                return Err(StorageError::DigestMismatch);
            }
            if before.len() != byte_length {
                return Err(StorageError::LengthMismatch);
            }
            let mut hash = Sha256::new();
            let mut length = 0_u64;
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                let read = file
                    .read(&mut buffer)
                    .map_err(|_| StorageError::Unavailable)?;
                if read == 0 {
                    break;
                }
                let next = length + read as u64;
                if next > byte_length || next > MAX_OBJECT_BYTES {
                    return Err(StorageError::LengthMismatch);
                }
                if let Some(expected) = expected_bytes {
                    if expected.get(length as usize..next as usize) != Some(&buffer[..read]) {
                        return Err(StorageError::Conflict);
                    }
                }
                hash.update(&buffer[..read]);
                length = next;
            }
            let after = file.metadata().map_err(|_| StorageError::Unavailable)?;
            if length != byte_length || after.len() != before.len() {
                return Err(StorageError::LengthMismatch);
            }
            if format!("{:x}", hash.finalize()) != sha256 {
                return Err(StorageError::DigestMismatch);
            }
            if after.mode() != before.mode()
                || after.uid() != before.uid()
                || after.mtime() != before.mtime()
                || after.mtime_nsec() != before.mtime_nsec()
            {
                return Err(StorageError::Conflict);
            }
            // Also sync an equal existing object: accepting a duplicate must not assume the
            // original writer reached its durability boundary. Link/unlink changes ctime, so
            // ctime is deliberately not an integrity oracle for concurrent equal uploads.
            file.sync_all().map_err(|_| StorageError::Unavailable)?;
            let named = open_file(self.objects.as_raw_fd(), &name, libc::O_RDONLY, 0)
                .map_err(object_open_error)?;
            let named = named.metadata().map_err(|_| StorageError::Unavailable)?;
            if named.dev() != before.dev()
                || named.ino() != before.ino()
                || named.len() != after.len()
                || named.mode() != after.mode()
                || named.uid() != after.uid()
                || named.mtime() != after.mtime()
                || named.mtime_nsec() != after.mtime_nsec()
            {
                return Err(StorageError::Conflict);
            }
            Ok(file)
        }

        fn check_directories(&self) -> Result<(), StoreError> {
            check_private_directory(&self.root).map_err(|_| StoreError::Unavailable)?;
            check_private_directory(&self.objects).map_err(|_| StoreError::Unavailable)?;
            let name = CString::new("objects").map_err(|_| StoreError::Unavailable)?;
            let current = open_directory(self.root.as_raw_fd(), &name)
                .map_err(|_| StoreError::Unavailable)?;
            let pinned = self
                .objects
                .metadata()
                .map_err(|_| StoreError::Unavailable)?;
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
                if created != 0 && io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
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
                libc::c_uint::from(mode),
            )
        };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat returned a fresh owned descriptor, now transferred exactly once.
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }

    fn object_open_error(error: io::Error) -> StorageError {
        match error.raw_os_error() {
            Some(libc::ENOENT) => StorageError::NotFound,
            Some(libc::ELOOP | libc::ENOTDIR | libc::EACCES | libc::EPERM | libc::ENXIO) => {
                StorageError::DigestMismatch
            }
            _ => StorageError::Unavailable,
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
                Self(parent.join(format!("cantos-private-storage-{}", uuid::Uuid::new_v4())))
            }

            fn storage(&self) -> ObjectDirectories {
                ObjectDirectories::new(&self.0).unwrap()
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

        /// The adapter under test with its own production-sized lanes, so parallel tests never
        /// observe each other's permits as `Unavailable`.
        fn isolated_storage(scratch: &Scratch) -> super::super::FilesystemStorage {
            super::super::FilesystemStorage::with_test_lanes(
                &scratch.0,
                Arc::new(tokio::sync::Semaphore::new(4)),
            )
            .unwrap()
        }

        fn async_storage(scratch: &Scratch, bytes: &[u8]) -> super::super::FilesystemStorage {
            let storage = isolated_storage(scratch);
            storage
                .inner
                .persist_bytes(bytes, &digest(bytes), bytes.len() as u64)
                .unwrap();
            storage
        }

        async fn collect_read(mut read: super::super::port::StorageRead) -> Vec<u8> {
            use futures_util::StreamExt;
            let mut bytes = Vec::new();
            while let Some(chunk) = read.stream.next().await {
                let chunk = chunk.unwrap();
                assert!(chunk.len() <= 16 * 1024);
                bytes.extend_from_slice(&chunk);
            }
            assert_eq!(bytes.len() as u64, read.byte_length);
            bytes
        }

        #[tokio::test]
        async fn inspected_images_survive_conditional_replay_restart_and_exact_private_reads() {
            use super::super::{ObjectIntegrity, ObjectKey, PrivateStorage};
            use crate::contexts::media::{
                domain::{image::ImageMime, object_storage::ByteRange},
                image_intake::ImageAdmission,
                storage::port::VerifiedPayload,
            };

            let scratch = Scratch::new();
            let storage = isolated_storage(&scratch);
            for (bytes, mime) in [
                (
                    include_bytes!("image_intake/fixtures/python-rgb-2x3.png").as_slice(),
                    ImageMime::Png,
                ),
                (
                    include_bytes!("image_intake/fixtures/pillow-rgb-2x3.jpg").as_slice(),
                    ImageMime::Jpeg,
                ),
            ] {
                for _ in 0..2 {
                    let image = ImageAdmission::from_test_lanes(std::sync::Arc::new(
                        tokio::sync::Semaphore::new(1),
                    ))
                    .unwrap()
                    .inspect(bytes.to_vec(), mime)
                    .await
                    .unwrap();
                    storage
                        .put_if_absent(VerifiedPayload::Image(image))
                        .await
                        .unwrap();
                }
                let key = ObjectKey::for_bytes(bytes);
                let integrity = ObjectIntegrity::for_bytes(bytes).unwrap();
                let restarted = isolated_storage(&scratch);
                restarted
                    .verify(key.clone(), integrity.clone())
                    .await
                    .unwrap();
                let read = restarted
                    .open_verified(key.clone(), integrity.clone(), None)
                    .await
                    .unwrap();
                assert_eq!(collect_read(read).await, bytes);
                let range = ByteRange::parse("bytes=3-10")
                    .unwrap()
                    .normalize(bytes.len() as u64)
                    .unwrap();
                let read = restarted
                    .open_verified(key, integrity, Some(range))
                    .await
                    .unwrap();
                assert_eq!(collect_read(read).await, &bytes[3..11]);
                assert_eq!(fs::read(scratch.object(&digest(bytes))).unwrap(), bytes);
                assert_eq!(
                    fs::metadata(scratch.object(&digest(bytes)))
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o7777,
                    0o400
                );
            }
            assert_eq!(fs::read_dir(scratch.0.join("objects")).unwrap().count(), 2);
        }

        #[tokio::test]
        async fn replay_of_an_inspected_image_rejects_corruption_without_overwriting_it() {
            use super::super::PrivateStorage;
            use crate::contexts::media::{
                domain::image::ImageMime, image_intake::ImageAdmission,
                storage::port::VerifiedPayload,
            };

            let bytes = include_bytes!("image_intake/fixtures/python-rgb-2x3.png").as_slice();
            let scratch = Scratch::new();
            let storage = isolated_storage(&scratch);
            let image = ImageAdmission::from_test_lanes(std::sync::Arc::new(
                tokio::sync::Semaphore::new(1),
            ))
            .unwrap()
            .inspect(bytes.to_vec(), ImageMime::Png)
            .await
            .unwrap();
            storage
                .put_if_absent(VerifiedPayload::Image(image))
                .await
                .unwrap();
            let object = scratch.object(&digest(bytes));
            fs::set_permissions(&object, Permissions::from_mode(0o600)).unwrap();
            let mut corrupt = bytes.to_vec();
            corrupt[bytes.len() - 1] ^= 1;
            fs::write(&object, &corrupt).unwrap();
            fs::set_permissions(&object, Permissions::from_mode(0o400)).unwrap();

            let image = ImageAdmission::from_test_lanes(std::sync::Arc::new(
                tokio::sync::Semaphore::new(1),
            ))
            .unwrap()
            .inspect(bytes.to_vec(), ImageMime::Png)
            .await
            .unwrap();
            assert_eq!(
                storage.put_if_absent(VerifiedPayload::Image(image)).await,
                Err(StorageError::DigestMismatch)
            );
            assert_eq!(fs::read(&object).unwrap(), corrupt);
            assert_eq!(fs::read_dir(scratch.0.join("objects")).unwrap().count(), 1);
        }

        #[tokio::test]
        async fn cancelled_blocking_io_keeps_its_lane_until_the_operation_actually_finishes() {
            use super::super::bounded_filesystem_io;
            use std::{sync::mpsc, time::Duration};
            use tokio::sync::{oneshot, Semaphore};

            // An isolated pool tests the production boundary without occupying other tests'
            // global lanes. Observable channels fence the crash point; no timing sleep is used.
            let lanes = Arc::new(Semaphore::new(1));
            let (started, has_started) = oneshot::channel();
            let (release, proceed) = mpsc::channel();
            let worker = tokio::spawn(bounded_filesystem_io(lanes.clone(), move || {
                let _ = started.send(());
                proceed.recv().map_err(|_| StorageError::Unavailable)?;
                Ok(())
            }));
            has_started.await.unwrap();
            worker.abort();
            assert!(worker.await.unwrap_err().is_cancelled());
            assert_eq!(lanes.available_permits(), 0);
            assert_eq!(
                bounded_filesystem_io(lanes.clone(), || Ok(())).await,
                Err(StorageError::Unavailable)
            );
            release.send(()).unwrap();
            let returned =
                tokio::time::timeout(Duration::from_secs(2), lanes.clone().acquire_owned())
                    .await
                    .unwrap()
                    .unwrap();
            drop(returned);
            assert_eq!(lanes.available_permits(), 1);
        }

        #[tokio::test]
        async fn private_port_reads_full_first_middle_final_and_suffix_ranges_in_bounded_chunks() {
            use super::super::{ObjectIntegrity, ObjectKey, PrivateStorage};
            use crate::contexts::media::domain::object_storage::ByteRange;

            let scratch = Scratch::new();
            let bytes: Vec<_> = (0..50_000).map(|index| (index % 251) as u8).collect();
            let storage = async_storage(&scratch, &bytes);
            let key = ObjectKey::for_bytes(&bytes);
            let integrity = ObjectIntegrity::for_bytes(&bytes).unwrap();
            let read = storage
                .open_verified(key.clone(), integrity.clone(), None)
                .await
                .unwrap();
            assert_eq!(collect_read(read).await, bytes);
            for (raw, expected) in [
                ("bytes=0-1023", &bytes[..1024]),
                ("bytes=20000-21023", &bytes[20_000..21_024]),
                ("bytes=48976-", &bytes[48_976..]),
                ("bytes=-1024", &bytes[48_976..]),
            ] {
                let range = ByteRange::parse(raw)
                    .unwrap()
                    .normalize(bytes.len() as u64)
                    .unwrap();
                let read = storage
                    .open_verified(key.clone(), integrity.clone(), Some(range))
                    .await
                    .unwrap();
                assert_eq!(collect_read(read).await, expected);
            }
            assert!(storage
                .temporary_read(key, integrity, 60)
                .await
                .unwrap()
                .is_none());
        }

        #[tokio::test]
        async fn verified_stream_retains_the_hashed_inode_when_the_object_name_is_replaced() {
            use super::super::{ObjectIntegrity, ObjectKey, PrivateStorage};

            let scratch = Scratch::new();
            let bytes = b"the accepted immutable audio bytes";
            let storage = async_storage(&scratch, bytes);
            let key = ObjectKey::for_bytes(bytes);
            let integrity = ObjectIntegrity::for_bytes(bytes).unwrap();
            let read = storage
                .open_verified(key.clone(), integrity.clone(), None)
                .await
                .unwrap();
            let path = scratch.object(key.as_str());
            fs::rename(&path, scratch.0.join("original-inode")).unwrap();
            fs::write(&path, vec![b'x'; bytes.len()]).unwrap();
            fs::set_permissions(&path, Permissions::from_mode(0o400)).unwrap();
            assert_eq!(collect_read(read).await, bytes);
            assert_eq!(
                PrivateStorage::verify(&storage, key, integrity).await,
                Err(StorageError::DigestMismatch)
            );
        }

        #[tokio::test]
        async fn cancelled_stream_does_not_delete_or_mutate_the_private_object() {
            use super::super::{ObjectIntegrity, ObjectKey, PrivateStorage};
            use futures_util::StreamExt;

            let scratch = Scratch::new();
            let bytes = vec![42; 50_000];
            let storage = async_storage(&scratch, &bytes);
            let key = ObjectKey::for_bytes(&bytes);
            let integrity = ObjectIntegrity::for_bytes(&bytes).unwrap();
            let inode = fs::metadata(scratch.object(key.as_str())).unwrap().ino();
            let mut read = storage
                .open_verified(key.clone(), integrity.clone(), None)
                .await
                .unwrap();
            assert!(read.stream.next().await.unwrap().unwrap().len() <= 16 * 1024);
            drop(read);
            assert_eq!(
                fs::metadata(scratch.object(key.as_str())).unwrap().ino(),
                inode
            );
            assert_eq!(fs::read(scratch.object(key.as_str())).unwrap(), bytes);
            assert_eq!(
                PrivateStorage::verify(&storage, key, integrity).await,
                Ok(())
            );
        }

        #[tokio::test]
        async fn private_port_rejects_missing_wrong_length_and_another_objects_normalized_range() {
            use super::super::{NormalizedByteRange, ObjectIntegrity, ObjectKey, PrivateStorage};

            let scratch = Scratch::new();
            let bytes = b"exact accepted bytes";
            let storage = async_storage(&scratch, bytes);
            let key = ObjectKey::for_bytes(bytes);
            let integrity = ObjectIntegrity::for_bytes(bytes).unwrap();
            let missing = ObjectIntegrity::for_bytes(b"absent bytes").unwrap();
            assert_eq!(
                PrivateStorage::verify(&storage, ObjectKey::for_bytes(b"absent bytes"), missing)
                    .await,
                Err(StorageError::NotFound)
            );
            assert_eq!(
                PrivateStorage::verify(
                    &storage,
                    key.clone(),
                    ObjectIntegrity::new(key.as_str(), bytes.len() as u64 + 1).unwrap(),
                )
                .await,
                Err(StorageError::LengthMismatch)
            );
            let other_range = NormalizedByteRange::full(bytes.len() as u64 + 1).unwrap();
            assert!(matches!(
                storage
                    .open_verified(key, integrity, Some(other_range))
                    .await,
                Err(StorageError::LengthMismatch)
            ));
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
            assert_eq!(
                fs::read(scratch.object(&expected)).unwrap().as_slice(),
                &bytes[..]
            );
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
            assert_eq!(
                fs::metadata(scratch.object(&expected)).unwrap().ino(),
                inode
            );
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
            for length in [0, MAX_OBJECT_BYTES + 1, u64::MAX] {
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
                ObjectDirectories::new(Path::new("relative-root")),
                Err(StoreError::InvalidRequest)
            ));
            let scratch = Scratch::new();
            fs::create_dir(&scratch.0).unwrap();
            fs::set_permissions(&scratch.0, Permissions::from_mode(0o755)).unwrap();
            assert!(matches!(
                ObjectDirectories::new(&scratch.0),
                Err(StoreError::InvalidRequest)
            ));
            fs::set_permissions(&scratch.0, Permissions::from_mode(0o700)).unwrap();
            let linked = scratch.0.join("linked-root");
            symlink(&scratch.0, &linked).unwrap();
            assert!(matches!(
                ObjectDirectories::new(&linked),
                Err(StoreError::InvalidRequest)
            ));
            assert!(matches!(
                ObjectDirectories::new(&linked.join("child")),
                Err(StoreError::InvalidRequest)
            ));
            symlink(&scratch.0, scratch.0.join("objects")).unwrap();
            assert!(matches!(
                ObjectDirectories::new(&scratch.0),
                Err(StoreError::InvalidRequest)
            ));
        }

        #[test]
        fn replacing_the_objects_directory_cannot_redirect_pinned_storage() {
            let scratch = Scratch::new();
            let storage = scratch.storage();
            fs::rename(
                scratch.0.join("objects"),
                scratch.0.join("original-objects"),
            )
            .unwrap();
            fs::create_dir(scratch.0.join("objects")).unwrap();
            fs::set_permissions(scratch.0.join("objects"), Permissions::from_mode(0o700)).unwrap();
            assert!(matches!(
                storage.persist_bytes(b"bytes", &digest(b"bytes"), 5),
                Err(StoreError::Unavailable)
            ));
            assert_eq!(fs::read_dir(scratch.0.join("objects")).unwrap().count(), 0);
            assert_eq!(
                fs::read_dir(scratch.0.join("original-objects"))
                    .unwrap()
                    .count(),
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

/// Checked host configuration must keep publication roots outside every private filesystem
/// target. A distinct object-storage class marker also refuses same-root aliasing at open.
pub fn validate_publication_separation(
    publication: &Path,
    private_root: Option<&Path>,
    private_config: Option<&Path>,
) -> Result<(), StorageError> {
    let separate = |private: &Path| {
        if publication.starts_with(private) || private.starts_with(publication) {
            Err(StorageError::InvalidTarget)
        } else {
            Ok(())
        }
    };
    if let Some(root) = private_root {
        separate(root)?;
    }
    if let Some(path) = private_config {
        for target in config::StorageConfig::load(path)?.targets.values() {
            if let config::TargetConfig::Filesystem(root) = target {
                separate(root)?;
            }
        }
    }
    Ok(())
}
