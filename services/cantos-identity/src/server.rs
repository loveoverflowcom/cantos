//! The private Unix-socket listener: ownership checks, safe bind, bounded accept, graceful
//! bounded drain. There is no TCP listener and no fallback when peer credentials are missing.

#[cfg(not(unix))]
compile_error!(
    "cantos-identity requires a Unix host: caller authentication uses OS peer credentials"
);

use std::{
    future::Future,
    io,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use crate::rpc::{generated::identity_server::IdentityServer, MAX_MESSAGE_BYTES};
use cantos_store_turso::{clock::SystemClock, registry::Intent};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{UnixListener, UnixStream},
    sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore},
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::server::{Connected, UdsConnectInfo};

use crate::{
    application::{IdentityService, Limits, Settings},
    config::{MigrateConfig, ServeConfig, SocketMode},
    credentials::KdfPool,
    events,
    mail::{Mailer, SmtpMailer, UnconfiguredMailer},
    persistence::{IdentityStore, IdentityStoreError},
    policy,
    rpc::{CallerPolicy, IdentityRpc},
};

/// Connections held at once; each may run a few streams, so this bounds in-flight work.
const MAX_CONNECTIONS: usize = 16;
const STREAMS_PER_CONNECTION: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunError {
    /// The database could not be opened: ownership, schema, epoch or storage.
    Database,
    Configuration,
    /// The socket directory, an existing path or the bind itself was refused.
    Socket,
    /// Another live process answers on this socket path.
    AlreadyServing,
    Serve,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Database => "identity database could not be opened",
            Self::Configuration => "identity configuration was refused",
            Self::Socket => "identity socket could not be prepared",
            Self::AlreadyServing => "another identity process is serving this socket",
            Self::Serve => "identity server stopped with an error",
        })
    }
}

impl std::error::Error for RunError {}

/// Opens (and on first use initializes) the database. Never runs implicitly from `serve`.
pub async fn run_migrate(config: &MigrateConfig) -> Result<(), RunError> {
    IdentityStore::open(&config.database, Intent::Migrate, Arc::new(SystemClock))
        .await
        .map(|_| ())
        .map_err(|error| {
            events::emit("migrate_failed", store_reason(&error));
            RunError::Database
        })
}

fn store_reason(error: &IdentityStoreError) -> &'static str {
    match error {
        IdentityStoreError::Open(_) => "database open refused",
        IdentityStoreError::Uninitialized => "database is not initialized; run migrate",
        IdentityStoreError::CorruptRecord => "database record is invalid",
        _ => "database unavailable",
    }
}

/// Checks the socket's parent directory: it exists, belongs to this user, and grants nothing to
/// others nor write to its group. A directory failing this is never adopted or chmodded.
pub fn check_socket_directory(socket: &Path) -> Result<(), RunError> {
    let parent = socket.parent().ok_or(RunError::Socket)?;
    let metadata = std::fs::symlink_metadata(parent).map_err(|_| RunError::Socket)?;
    if !metadata.is_dir() {
        return Err(RunError::Socket);
    }
    let mode = metadata.mode();
    if mode & 0o007 != 0 || mode & 0o020 != 0 || metadata.uid() != effective_uid_in(parent)? {
        return Err(RunError::Socket);
    }
    Ok(())
}

/// The effective uid of this process, observed as the owner of a file it just created inside
/// the (already checked) socket directory. It also proves the directory is writable here, which
/// binding needs anyway. No unsafe call and no extra dependency.
fn effective_uid_in(directory: &Path) -> Result<u32, RunError> {
    let probe = directory.join(format!(".cantos-identity-probe-{}", std::process::id()));
    let created = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|_| RunError::Socket)?;
    let uid = created.metadata().map_err(|_| RunError::Socket)?.uid();
    drop(created);
    let _ = std::fs::remove_file(&probe);
    Ok(uid)
}

/// Removes the socket on drop only if it is still the exact inode this run created.
pub struct SocketGuard {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        if let Ok(meta) = std::fs::symlink_metadata(&self.path) {
            if meta.file_type().is_socket() && meta.dev() == self.device && meta.ino() == self.inode
            {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

/// Binds the listener. An existing path is replaced only when it is a socket nobody answers on.
pub async fn bind_private_socket(
    path: &Path,
    mode: SocketMode,
) -> Result<(UnixListener, SocketGuard), RunError> {
    check_socket_directory(path)?;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_socket() => {
            match tokio::time::timeout(Duration::from_secs(1), UnixStream::connect(path)).await {
                Ok(Ok(_)) => return Err(RunError::AlreadyServing),
                Ok(Err(error))
                    if matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                    ) =>
                {
                    std::fs::remove_file(path).map_err(|_| RunError::Socket)?;
                }
                _ => return Err(RunError::Socket),
            }
        }
        Ok(_) => return Err(RunError::Socket),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(RunError::Socket),
    }
    let listener = UnixListener::bind(path).map_err(|_| RunError::Socket)?;
    // The checked parent directory is what keeps others out during this instant.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode.bits()))
        .map_err(|_| RunError::Socket)?;
    let meta = std::fs::symlink_metadata(path).map_err(|_| RunError::Socket)?;
    let guard = SocketGuard {
        path: path.to_path_buf(),
        device: meta.dev(),
        inode: meta.ino(),
    };
    Ok((listener, guard))
}

/// A connection plus the capacity permit it holds until it closes.
struct Connection {
    stream: UnixStream,
    _permit: OwnedSemaphorePermit,
}

impl AsyncRead for Connection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, data)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

impl Connected for Connection {
    type ConnectInfo = UdsConnectInfo;
    fn connect_info(&self) -> Self::ConnectInfo {
        self.stream.connect_info()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Readiness {
    pub mail_configured: bool,
}

/// Opens the store in serve mode and composes the service. Blocking work (one dummy hash)
/// happens here, before the socket exists.
pub async fn compose(config: &ServeConfig) -> Result<(Arc<IdentityService>, Readiness), RunError> {
    let clock: Arc<dyn cantos_store_turso::clock::Clock> = Arc::new(SystemClock);
    let store = IdentityStore::open(&config.database, Intent::Serve, Arc::clone(&clock))
        .await
        .map_err(|error| {
            events::emit("open_failed", store_reason(&error));
            RunError::Database
        })?;
    let mail_configured = config.smtp.is_some();
    let mailer: Arc<dyn Mailer> = match &config.smtp {
        Some(smtp) => Arc::new(SmtpMailer::new(smtp.clone()).map_err(|_| RunError::Configuration)?),
        None => Arc::new(UnconfiguredMailer),
    };
    let settings = Settings {
        link: config.link.clone(),
        kdf: KdfPool::new(
            policy::KDF_CONCURRENCY,
            policy::KDF_QUEUE,
            policy::KDF_QUEUE_DEADLINE,
        ),
        limits: Limits::standard(),
    };
    let service = tokio::task::spawn_blocking({
        let store = Arc::new(store);
        move || IdentityService::new(store, clock, mailer, settings)
    })
    .await
    .map_err(|_| RunError::Serve)?
    .map_err(|_| RunError::Serve)?;
    Ok((Arc::new(service), Readiness { mail_configured }))
}

// ACCEPTANCE-PENDING(footprint/supervision): release-binary size, idle/peak RSS under concurrent
// hashing, and behavior under a real service supervisor are not measured; tests ran debug builds.
/// Serves until `shutdown` resolves, then stops accepting and drains for at most
/// [`policy::SHUTDOWN_DRAIN`]. `ready` runs once the socket accepts connections.
pub async fn run_serve(
    config: ServeConfig,
    ready: impl FnOnce(Readiness),
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), RunError> {
    let caller_policy = CallerPolicy::new(config.server_uids.iter().copied())
        .map_err(|_| RunError::Configuration)?;
    let (service, readiness) = compose(&config).await?;
    let (listener, _guard) = bind_private_socket(&config.socket, config.socket_mode).await?;
    let (connections, incoming) = mpsc::channel::<io::Result<Connection>>(MAX_CONNECTIONS);
    let capacity = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let allowed = caller_policy.clone();
    let acceptor = tokio::spawn(async move {
        loop {
            let Ok(permit) = Arc::clone(&capacity).acquire_owned().await else {
                return;
            };
            let Ok((stream, _)) = listener.accept().await else {
                // Descriptor exhaustion or a transient accept error must not spin the executor.
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            };
            // Drop non-allowlisted peers before any bytes are read. The handler checks again.
            let permitted = stream
                .peer_cred()
                .is_ok_and(|credential| allowed.permits_uid(credential.uid()));
            if !permitted {
                events::emit("connection_refused", "peer is not an allowed caller");
                continue;
            }
            if connections
                .send(Ok(Connection {
                    stream,
                    _permit: permit,
                }))
                .await
                .is_err()
            {
                return;
            }
        }
    });
    let rpc = IdentityServer::new(IdentityRpc::new(service, caller_policy))
        .max_decoding_message_size(MAX_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_MESSAGE_BYTES);
    let (stop, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .timeout(policy::RPC_REQUEST_DEADLINE)
            .concurrency_limit_per_connection(STREAMS_PER_CONNECTION)
            .max_concurrent_streams(policy::RPC_CONCURRENT_STREAMS)
            .add_service(rpc)
            .serve_with_incoming_shutdown(ReceiverStream::new(incoming), async {
                let _ = stopped.await;
            }),
    );
    ready(readiness);
    let mut server = server;
    tokio::select! {
        () = shutdown => {}
        // A listener that ended on its own is a failure, not something to wait out silently.
        ended = &mut server => {
            acceptor.abort();
            events::emit("server_stopped", "the listener ended without a shutdown request");
            return match ended {
                Ok(Ok(())) => Ok(()),
                _ => Err(RunError::Serve),
            };
        }
    }
    acceptor.abort();
    let _ = stop.send(());
    let drained = tokio::time::timeout(policy::SHUTDOWN_DRAIN, server).await;
    match drained {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(_) => Err(RunError::Serve),
        Err(_) => {
            events::emit("drain_expired", "in-flight requests were abandoned");
            Ok(())
        }
    }
}
