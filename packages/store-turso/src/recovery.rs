//! Owner-local anti-rollback floor. The private fence is deliberately outside DB snapshots.
//! It proves freshness, not the contents of missing security state. Unknown effects stay closed.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use crate::{error::StoreFault, owner::OwnerLock};

/// Include unchanged in an appended service-owned migration before explicit enrollment.
pub const RECOVERY_SCHEMA: &str = include_str!("recovery_schema.sql");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryFault {
    Missing,
    Malformed,
    Untrusted,
    Pending,
    Mismatch,
    Exhausted,
    AlreadyEnrolled,
    Unenrolled,
    Io,
}

impl From<RecoveryFault> for StoreFault {
    fn from(value: RecoveryFault) -> Self {
        Self::Recovery(value)
    }
}

pub fn recovery_id(value: &str) -> Result<[u8; 16], RecoveryFault> {
    Ok(Token::decode(&format!("{value}:1"))?.instance)
}

/// Root/operator-owned pair exposure barrier. The expected operation is pinned in both
/// service configurations; an old READY file or old unit configuration does not match it.
/// This is not a credential. The trusted UID is fixed by the composition, never file content.
#[derive(Clone, Debug)]
pub struct RecoveryBarrier {
    path: PathBuf,
    operation: [u8; 16],
    trusted_uid: u32,
}

impl RecoveryBarrier {
    pub fn from_hex(
        path: PathBuf,
        operation: &str,
        trusted_uid: u32,
    ) -> Result<Self, RecoveryFault> {
        Self::new(path, recovery_id(operation)?, trusted_uid)
    }
    pub fn new(
        path: PathBuf,
        operation: [u8; 16],
        trusted_uid: u32,
    ) -> Result<Self, RecoveryFault> {
        if operation == [0; 16] || !path.is_absolute() {
            return Err(RecoveryFault::Untrusted);
        }
        Ok(Self {
            path,
            operation,
            trusted_uid,
        })
    }

    pub(crate) fn verify(&self) -> Result<(), RecoveryFault> {
        let parent = self.path.parent().ok_or(RecoveryFault::Untrusted)?;
        if parent
            .canonicalize()
            .map_err(|_| RecoveryFault::Untrusted)?
            != parent
        {
            return Err(RecoveryFault::Untrusted);
        }
        let named = fs::symlink_metadata(&self.path).map_err(|_| RecoveryFault::Missing)?;
        if named.file_type().is_symlink() {
            return Err(RecoveryFault::Untrusted);
        }
        let input = File::open(&self.path).map_err(|_| RecoveryFault::Missing)?;
        let file = input.metadata().map_err(|_| RecoveryFault::Untrusted)?;
        let directory = fs::metadata(parent).map_err(|_| RecoveryFault::Untrusted)?;
        if !file.is_file() || file.len() > 128 {
            return Err(RecoveryFault::Untrusted);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if file.uid() != self.trusted_uid
                || directory.uid() != self.trusted_uid
                || file.mode() & 0o022 != 0
                || directory.mode() & 0o022 != 0
                || file.nlink() != 1
                || named.dev() != file.dev()
                || named.ino() != file.ino()
            {
                return Err(RecoveryFault::Untrusted);
            }
        }
        let mut bytes = Vec::new();
        input
            .take(129)
            .read_to_end(&mut bytes)
            .map_err(|_| RecoveryFault::Io)?;
        if bytes.len() > 128 {
            return Err(RecoveryFault::Malformed);
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| RecoveryFault::Malformed)?;
        let value = text
            .strip_prefix("CANTOS_RECOVERY_BARRIER_V1\n")
            .and_then(|text| text.strip_suffix('\n'))
            .ok_or(RecoveryFault::Malformed)?;
        let (state, operation) = value.split_once(' ').ok_or(RecoveryFault::Malformed)?;
        let expected: String = self
            .operation
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if operation != expected {
            return Err(RecoveryFault::Mismatch);
        }
        match state {
            "ready" => Ok(()),
            "blocked" => Err(RecoveryFault::Pending),
            _ => Err(RecoveryFault::Malformed),
        }
    }
}

/// Nonsecret owner freshness observation. Never a credential or an authentication fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryObservation {
    pub instance: [u8; 16],
    pub revision: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    pub instance: [u8; 16],
    pub revision: i64,
}

impl Token {
    pub fn new() -> Result<Self, RecoveryFault> {
        let mut instance = [0; 16];
        File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut instance))
            .map_err(|_| RecoveryFault::Io)?;
        if instance == [0; 16] {
            return Err(RecoveryFault::Io);
        }
        Ok(Self {
            instance,
            revision: 1,
        })
    }

    fn next(self) -> Result<Self, RecoveryFault> {
        Ok(Self {
            instance: self.instance,
            revision: self
                .revision
                .checked_add(1)
                .ok_or(RecoveryFault::Exhausted)?,
        })
    }

    fn encode(self) -> String {
        let instance: String = self
            .instance
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{instance}:{}", self.revision)
    }

    fn decode(value: &str) -> Result<Self, RecoveryFault> {
        let (instance, revision) = value.split_once(':').ok_or(RecoveryFault::Malformed)?;
        if instance.len() != 32
            || !instance
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(RecoveryFault::Malformed);
        }
        let mut bytes = [0; 16];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&instance[index * 2..index * 2 + 2], 16)
                .map_err(|_| RecoveryFault::Malformed)?;
        }
        let parsed = revision
            .parse::<i64>()
            .map_err(|_| RecoveryFault::Malformed)?;
        if bytes == [0; 16] || parsed < 1 || parsed.to_string() != revision {
            return Err(RecoveryFault::Malformed);
        }
        Ok(Self {
            instance: bytes,
            revision: parsed,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Record {
    Committed(Token),
    Pending(Token),
}

impl Record {
    fn bytes(self) -> Vec<u8> {
        let (state, token) = match self {
            Self::Committed(token) => ("committed", token),
            Self::Pending(token) => ("pending", token),
        };
        format!("CANTOS_RECOVERY_FENCE_V1\n{state} {}\n", token.encode()).into_bytes()
    }

    fn decode(bytes: &[u8]) -> Result<Self, RecoveryFault> {
        let value = std::str::from_utf8(bytes).map_err(|_| RecoveryFault::Malformed)?;
        let body = value
            .strip_prefix("CANTOS_RECOVERY_FENCE_V1\n")
            .and_then(|v| v.strip_suffix('\n'))
            .ok_or(RecoveryFault::Malformed)?;
        let (kind, token) = body.split_once(' ').ok_or(RecoveryFault::Malformed)?;
        let token = Token::decode(token)?;
        match kind {
            "committed" => Ok(Self::Committed(token)),
            "pending" => Ok(Self::Pending(token)),
            _ => Err(RecoveryFault::Malformed),
        }
    }
}

pub(crate) struct Fence {
    path: PathBuf,
    current: Mutex<Token>,
    abandoned: Mutex<Option<(Token, Token)>>,
    // Independent lease prevents two restored copies from sharing the same authority.
    _lease: OwnerLock,
}

impl Fence {
    fn acquire(path: &Path, database: &Path, token: Token) -> Result<Arc<Self>, RecoveryFault> {
        let parent = path.parent().ok_or(RecoveryFault::Untrusted)?;
        let canonical = parent
            .canonicalize()
            .map_err(|_| RecoveryFault::Untrusted)?;
        let database_root = database
            .parent()
            .ok_or(RecoveryFault::Untrusted)?
            .canonicalize()
            .map_err(|_| RecoveryFault::Untrusted)?;
        if !path.is_absolute() || canonical != parent || canonical.starts_with(database_root) {
            return Err(RecoveryFault::Untrusted);
        }
        let lease = OwnerLock::acquire(path).map_err(|_| RecoveryFault::Untrusted)?;
        Ok(Arc::new(Self {
            path: path.to_owned(),
            current: Mutex::new(token),
            abandoned: Mutex::new(None),
            _lease: lease,
        }))
    }

    pub fn open(path: &Path, database: &Path, token: Token) -> Result<Arc<Self>, RecoveryFault> {
        let fence = Self::acquire(path, database, token)?;
        fence.verify()?;
        Ok(fence)
    }

    /// Explicit owner reconciliation, never ordinary startup. Only the exact attempted
    /// *next* token proves that the database commit happened; its predecessor is ambiguous.
    pub fn reconcile(
        path: &Path,
        database: &Path,
        token: Token,
    ) -> Result<Arc<Self>, RecoveryFault> {
        let fence = Self::acquire(path, database, token)?;
        match fence.read()? {
            Record::Pending(expected) if expected == token => fence.finish_enrollment(token)?,
            Record::Committed(expected) if expected == token => (),
            _ => return Err(RecoveryFault::Mismatch),
        }
        fence.verify()?;
        Ok(fence)
    }

    pub fn prepare_enrollment(
        path: &Path,
        database: &Path,
    ) -> Result<(Arc<Self>, Token), RecoveryFault> {
        let token = Token::new()?;
        let fence = Self::acquire(path, database, token)?;
        if fs::symlink_metadata(path).is_ok() {
            return Err(RecoveryFault::AlreadyEnrolled);
        }
        fence.publish(None, Record::Pending(token))?;
        Ok((fence, token))
    }

    pub fn token(&self) -> Result<Token, RecoveryFault> {
        self.current
            .lock()
            .map(|token| *token)
            .map_err(|_| RecoveryFault::Pending)
    }

    pub fn verify(&self) -> Result<(), RecoveryFault> {
        match self.read()? {
            Record::Pending(_) => Err(RecoveryFault::Pending),
            Record::Committed(token) if token == self.token()? => Ok(()),
            Record::Committed(_) => Err(RecoveryFault::Mismatch),
        }
    }

    pub fn begin(self: &Arc<Self>) -> Result<FenceWrite, RecoveryFault> {
        self.verify()?;
        let previous = self.token()?;
        let next = previous.next()?;
        self.publish(Some(Record::Committed(previous)), Record::Pending(next))?;
        Ok(FenceWrite {
            fence: self.clone(),
            previous,
            next,
            active: Arc::new(AtomicBool::new(true)),
            commit_attempted: AtomicBool::new(false),
            settled: AtomicBool::new(false),
        })
    }

    pub fn finish_enrollment(&self, token: Token) -> Result<(), RecoveryFault> {
        self.publish(Some(Record::Pending(token)), Record::Committed(token))
    }

    pub fn abandoned(&self) -> Result<Option<(Token, Token)>, RecoveryFault> {
        self.abandoned
            .lock()
            .map(|value| *value)
            .map_err(|_| RecoveryFault::Pending)
    }

    /// Only the SAME owner lifetime remembers that COMMIT was never attempted. Caller has
    /// acknowledged engine ROLLBACK and checked the previous DB token under its writer lease.
    pub fn finish_abandoned(&self, previous: Token, next: Token) -> Result<(), RecoveryFault> {
        let mut ticket = self.abandoned.lock().map_err(|_| RecoveryFault::Pending)?;
        if *ticket != Some((previous, next)) {
            return Err(RecoveryFault::Pending);
        }
        self.publish(Some(Record::Pending(next)), Record::Committed(previous))?;
        *ticket = None;
        Ok(())
    }

    fn read(&self) -> Result<Record, RecoveryFault> {
        let named = fs::symlink_metadata(&self.path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RecoveryFault::Missing
            } else {
                RecoveryFault::Io
            }
        })?;
        if named.file_type().is_symlink() {
            return Err(RecoveryFault::Untrusted);
        }
        let input = File::open(&self.path).map_err(|_| RecoveryFault::Io)?;
        let meta = input.metadata().map_err(|_| RecoveryFault::Io)?;
        if !meta.is_file() || meta.len() > 256 {
            return Err(RecoveryFault::Untrusted);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let directory = fs::metadata(self.path.parent().ok_or(RecoveryFault::Untrusted)?)
                .map_err(|_| RecoveryFault::Io)?;
            if meta.mode() & 0o777 != 0o600
                || meta.uid() != directory.uid()
                || meta.nlink() != 1
                || named.dev() != meta.dev()
                || named.ino() != meta.ino()
            {
                return Err(RecoveryFault::Untrusted);
            }
        }
        let mut bytes = Vec::new();
        input
            .take(257)
            .read_to_end(&mut bytes)
            .map_err(|_| RecoveryFault::Io)?;
        if bytes.len() > 256 {
            return Err(RecoveryFault::Malformed);
        }
        Record::decode(&bytes)
    }

    fn publish(&self, expected: Option<Record>, value: Record) -> Result<(), RecoveryFault> {
        if let Some(expected) = expected {
            if self.read()? != expected {
                return Err(RecoveryFault::Mismatch);
            }
        }
        let parent = self.path.parent().ok_or(RecoveryFault::Untrusted)?;
        let suffix = Token::new()?.encode();
        let temporary = parent.join(format!(".cantos-fence-{suffix}.tmp"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temporary).map_err(|_| RecoveryFault::Io)?;
            file.write_all(&value.bytes())
                .map_err(|_| RecoveryFault::Io)?;
            file.sync_all().map_err(|_| RecoveryFault::Io)?;
            if expected.is_some() {
                fs::rename(&temporary, &self.path).map_err(|_| RecoveryFault::Io)?;
            } else {
                fs::hard_link(&temporary, &self.path).map_err(|_| RecoveryFault::Io)?;
                fs::remove_file(&temporary).map_err(|_| RecoveryFault::Io)?;
            }
            File::open(parent)
                .and_then(|file| file.sync_all())
                .map_err(|_| RecoveryFault::Io)
        })();
        let _ = fs::remove_file(temporary);
        result
    }
}

pub(crate) struct FenceWrite {
    fence: Arc<Fence>,
    previous: Token,
    pub next: Token,
    active: Arc<AtomicBool>,
    commit_attempted: AtomicBool,
    settled: AtomicBool,
}

impl FenceWrite {
    pub fn previous(&self) -> Token {
        self.previous
    }

    pub fn commit_attempted(&self) {
        self.commit_attempted.store(true, Ordering::Release);
    }
    pub fn proof(&self) -> WriteProof {
        WriteProof {
            fence: self.fence.clone(),
            next: self.next,
            active: self.active.clone(),
        }
    }

    pub fn commit(&self) -> Result<(), RecoveryFault> {
        self.fence.publish(
            Some(Record::Pending(self.next)),
            Record::Committed(self.next),
        )?;
        *self
            .fence
            .current
            .lock()
            .map_err(|_| RecoveryFault::Pending)? = self.next;
        self.settled.store(true, Ordering::Release);
        Ok(())
    }

    /// Only called after an acknowledged engine rollback. Drop deliberately leaves Pending.
    pub fn rollback(&self) -> Result<(), RecoveryFault> {
        self.fence.publish(
            Some(Record::Pending(self.next)),
            Record::Committed(self.previous),
        )?;
        self.settled.store(true, Ordering::Release);
        Ok(())
    }
}

impl Drop for FenceWrite {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        if !self.commit_attempted.load(Ordering::Acquire) && !self.settled.load(Ordering::Acquire) {
            if let Ok(mut ticket) = self.fence.abandoned.lock() {
                *ticket = Some((self.previous, self.next));
            }
        }
    }
}

pub(crate) struct WriteProof {
    fence: Arc<Fence>,
    next: Token,
    active: Arc<AtomicBool>,
}

impl WriteProof {
    pub fn verify(&self) -> Result<(), RecoveryFault> {
        if self.active.load(Ordering::Acquire) && self.fence.read()? == Record::Pending(self.next) {
            Ok(())
        } else {
            Err(RecoveryFault::Pending)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_records_reject_ambiguity_and_overflow() {
        let token = Token {
            instance: [1; 16],
            revision: 7,
        };
        for record in [Record::Committed(token), Record::Pending(token)] {
            assert_eq!(Record::decode(&record.bytes()), Ok(record));
        }
        for value in [
            "",
            "00:1",
            "00000000000000000000000000000000:1",
            "01010101010101010101010101010101:0",
            "01010101010101010101010101010101:01",
            "01010101010101010101010101010101:-1",
            "01010101010101010101010101010101:9223372036854775808",
        ] {
            assert_eq!(Token::decode(value), Err(RecoveryFault::Malformed));
        }
        assert_eq!(
            Token {
                revision: i64::MAX,
                ..token
            }
            .next(),
            Err(RecoveryFault::Exhausted)
        );
        let mut extra = Record::Committed(token).bytes();
        extra.push(b'\n');
        assert_eq!(Record::decode(&extra), Err(RecoveryFault::Malformed));
    }
}
