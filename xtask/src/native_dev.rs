//! The one native development composition: `cantos-server` and `cantos-identity` as two child
//! processes, each owning exactly one embedded Turso file under a private development data
//! directory that xtask creates. There is no database server, no container, no provider issuer
//! and no inherited database path: every path below is derived here from the workspace root,
//! and every child environment is built here (with the inherited values that could redirect
//! it removed).
//!
//! Schema changes are the explicit `cantos-migrate` and `cantos-identity migrate` steps; the
//! serving processes never create or upgrade a file. The Server and the operator commands are
//! mutually exclusive owners of `server.db`, so anything that needs the operator (the seed)
//! runs while the Server is stopped.

use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    build::{Profile, Refusal},
    command::CommandSpec,
    config::{
        loopback, App, API_PORT, API_SERVER_BIN, IDENTITY_BIN, SERVER_MIGRATE_BIN,
        SERVER_OPERATOR_BIN,
    },
    workspace::Workspace,
};

/// Under the ignored `target/`: development data is disposable and never reaches Git.
pub const DEV_ROOT: &str = "target/native-dev";
/// `sockaddr_un.sun_path` holds 104 bytes on macOS and 108 on Linux; Identity's own check and
/// the Server's configuration both stop at 100.
const SOCKET_PATH_LIMIT: usize = 100;
/// A signup link must be an `https` URL; no mail is sent unless the caller configures SMTP, so
/// this placeholder is never followed.
pub const DEFAULT_SIGNUP_LINK_BASE: &str = "https://localhost.invalid/signup";
pub const DEFAULT_MEDIA_ORIGIN: &str = "https://localhost:8443";
pub const SEED_RECEIPT: &str = "theatre-seed.json";

/// Variables the composition sets itself. Whatever the caller exported under these names is
/// replaced, never merged.
#[cfg(test)]
const SERVER_OWNED: [&str; 6] = [
    "CANTOS_ENV",
    "CANTOS_BIND_ADDRESS",
    "CANTOS_SERVER_DATABASE",
    "CANTOS_IDENTITY_SOCKET",
    "CANTOS_IDENTITY_PEER_UID",
    "CANTOS_IDENTITY_PEER_POLICY",
];
/// Inherited settings that would change what the Server does or which Identity it trusts.
const SERVER_UNSET: [&str; 9] = [
    "CANTOS_WEB_ORIGIN",
    "CANTOS_BROWSER_API_SURFACE",
    "CANTOS_ADMIN_WEB_ORIGIN",
    "CANTOS_IDENTITY_TRUSTED_PROXIES",
    "CANTOS_IDENTITY_STUDIO_SUBJECTS",
    "CANTOS_STORAGE_CONFIG",
    "CANTOS_AUDIO_ROOT",
    "CANTOS_PUBLICATION_ROOT",
    "CANTOS_PLAYBACK_KEY_FILE",
];

/// Every path of one development composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub root: PathBuf,
    pub server_dir: PathBuf,
    pub server_db: PathBuf,
    pub identity_dir: PathBuf,
    pub identity_db: PathBuf,
    /// Private, short, and outside the repository so the socket path fits `sun_path`.
    pub socket_dir: PathBuf,
    pub socket: PathBuf,
    pub media_dir: PathBuf,
    pub seed_actor: PathBuf,
    pub seed_receipt: PathBuf,
    pub seed_journal: PathBuf,
    pub uid: u32,
}

impl Layout {
    /// Pure: derives paths from the workspace root, the caller's uid and the temp directory.
    pub fn resolve(workspace: &Workspace, uid: u32, temp: &Path) -> Result<Self, Refusal> {
        let root = workspace.path(DEV_ROOT);
        let tag: String = Sha256::digest(root.as_os_str().as_encoded_bytes())
            .iter()
            .take(4)
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let socket_dir = temp.join(format!("cantos-dev-{uid}-{tag}"));
        let socket = socket_dir.join("identity.sock");
        if socket.as_os_str().len() > SOCKET_PATH_LIMIT {
            return Err(Refusal::Usage(format!(
                "the Identity socket path ({} bytes) would exceed {SOCKET_PATH_LIMIT}; \
                 point TMPDIR at a shorter private directory",
                socket.as_os_str().len()
            )));
        }
        Ok(Self {
            server_dir: root.join("server"),
            server_db: root.join("server/server.db"),
            identity_dir: root.join("identity"),
            identity_db: root.join("identity/identity.db"),
            media_dir: root.join("media"),
            seed_actor: root.join("seed-actor"),
            seed_receipt: root.join(SEED_RECEIPT),
            seed_journal: root.join("theatre-seed.journal.json"),
            root,
            socket_dir,
            socket,
            uid,
        })
    }

    /// The owner's own uid. Identity's private RPC is a Unix socket, so the composition is
    /// Unix-only.
    pub fn for_current_user(workspace: &Workspace) -> Result<Self, Refusal> {
        #[cfg(unix)]
        {
            // SAFETY: geteuid has no preconditions and cannot fail.
            let uid = unsafe { libc::geteuid() };
            Self::resolve(workspace, uid, &std::env::temp_dir())
        }
        #[cfg(not(unix))]
        {
            let _ = workspace;
            Err(Refusal::Unsupported {
                subject: "native development composition".into(),
                reason: "Identity's private RPC is a Unix socket; run/seed need a Unix host".into(),
            })
        }
    }

    /// The directories xtask creates (0700) before any process opens a file in them.
    pub fn private_directories(&self) -> [&Path; 4] {
        [
            &self.root,
            &self.server_dir,
            &self.identity_dir,
            &self.socket_dir,
        ]
    }

    pub fn media_private(&self) -> PathBuf {
        self.media_dir.join("private")
    }

    pub fn media_publication(&self) -> PathBuf {
        self.media_dir.join("publication")
    }

    pub fn media_key(&self) -> PathBuf {
        self.media_dir.join("playback.key")
    }
}

/// Creates each directory with mode 0700, or proves an existing one already is exactly that:
/// a real directory (never a symlink), owned by this user, with no group/other access. An
/// existing directory that is not is refused, not repaired: it may be shared or inherited.
#[cfg(unix)]
pub fn ensure_private_directories(layout: &Layout) -> anyhow::Result<()> {
    use anyhow::{bail, Context};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    if let Some(parent) = layout.root.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    for directory in layout.private_directories() {
        match std::fs::symlink_metadata(directory) {
            Ok(metadata) => {
                if !metadata.file_type().is_dir()
                    || metadata.uid() != layout.uid
                    || metadata.permissions().mode() & 0o777 != 0o700
                {
                    bail!(
                        "{} exists but is not a private (0700) directory owned by this user; \
                         refusing to reuse it. Remove it yourself if it is stale",
                        directory.display()
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::DirBuilder::new()
                    .mode(0o700)
                    .create(directory)
                    .with_context(|| format!("create {}", directory.display()))?;
                // The process umask may only narrow a mode, but verify rather than assume.
                let created = std::fs::symlink_metadata(directory)?;
                if created.permissions().mode() & 0o777 != 0o700 {
                    bail!("{} was not created with mode 0700", directory.display());
                }
            }
            Err(error) => {
                return Err(error).with_context(|| format!("inspect {}", directory.display()))
            }
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn ensure_private_directories(_: &Layout) -> anyhow::Result<()> {
    anyhow::bail!("the native development composition needs a Unix host")
}

fn binary(workspace: &Workspace, profile: Profile, name: &str) -> String {
    workspace
        .path("target")
        .join(profile.cargo_dir())
        .join(name)
        .display()
        .to_string()
}

/// `cantos-migrate`: creates or upgrades `server.db`, the only action that does.
pub fn migrate_server(workspace: &Workspace, layout: &Layout, profile: Profile) -> CommandSpec {
    CommandSpec::new(
        binary(workspace, profile, SERVER_MIGRATE_BIN),
        workspace.root(),
    )
    .env(
        "CANTOS_SERVER_DATABASE",
        layout.server_db.display().to_string(),
    )
}

/// `cantos-identity migrate`: creates or upgrades `identity.db`.
pub fn migrate_identity(workspace: &Workspace, layout: &Layout, profile: Profile) -> CommandSpec {
    CommandSpec::new(binary(workspace, profile, IDENTITY_BIN), workspace.root())
        .args(["migrate"])
        .env(
            "CANTOS_IDENTITY_DATABASE",
            layout.identity_db.display().to_string(),
        )
}

/// `cantos-operator ARGS`: owns `server.db` while it runs, so the Server must be stopped.
pub fn operator(
    workspace: &Workspace,
    layout: &Layout,
    profile: Profile,
    args: &[&str],
) -> CommandSpec {
    CommandSpec::new(
        binary(workspace, profile, SERVER_OPERATOR_BIN),
        workspace.root(),
    )
    .args(args.iter().copied())
    .env(
        "CANTOS_SERVER_DATABASE",
        layout.server_db.display().to_string(),
    )
}

/// `cantos-identity serve`. Only the allowlisted caller uid (this user) may open the socket,
/// the socket is owner-only, and the signup link is the caller's explicit value or a placeholder.
pub fn identity_serve(
    workspace: &Workspace,
    layout: &Layout,
    profile: Profile,
    signup_link_base: Option<&str>,
) -> CommandSpec {
    CommandSpec::new(binary(workspace, profile, IDENTITY_BIN), workspace.root())
        .args(["serve"])
        .env(
            "CANTOS_IDENTITY_DATABASE",
            layout.identity_db.display().to_string(),
        )
        .env(
            "CANTOS_IDENTITY_SOCKET",
            layout.socket.display().to_string(),
        )
        .env("CANTOS_IDENTITY_SOCKET_MODE", "0600")
        .env("CANTOS_IDENTITY_SERVER_UIDS", layout.uid.to_string())
        .env(
            "CANTOS_IDENTITY_SIGNUP_LINK_BASE",
            signup_link_base.unwrap_or(DEFAULT_SIGNUP_LINK_BASE),
        )
}

/// Whether the Server is composed with Identity. The seed starts it without: the development
/// token it uses needs no Identity session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WithIdentity {
    Yes,
    No,
}

/// `cantos-server`: development host, loopback, both Web origins, its own `server.db`.
pub fn server_serve(
    workspace: &Workspace,
    layout: &Layout,
    profile: Profile,
    studio_origin: &str,
    identity: WithIdentity,
) -> CommandSpec {
    let mut command =
        CommandSpec::new(binary(workspace, profile, API_SERVER_BIN), workspace.root());
    for name in SERVER_UNSET {
        command = command.unset(name);
    }
    command = command
        .env("CANTOS_ENV", "development")
        .env("CANTOS_BIND_ADDRESS", loopback(API_PORT).to_string())
        .env("CANTOS_STUDIO_WEB_ORIGIN", studio_origin)
        .env("CANTOS_THEATRE_WEB_ORIGIN", App::Theatre.web_origin())
        .env(
            "CANTOS_SERVER_DATABASE",
            layout.server_db.display().to_string(),
        );
    if identity == WithIdentity::Yes {
        command = command
            .env(
                "CANTOS_IDENTITY_SOCKET",
                layout.socket.display().to_string(),
            )
            .env("CANTOS_IDENTITY_PEER_UID", layout.uid.to_string())
            .env("CANTOS_IDENTITY_PEER_POLICY", "direct_peer");
    } else {
        command = command
            .unset("CANTOS_IDENTITY_SOCKET")
            .unset("CANTOS_IDENTITY_PEER_UID")
            .unset("CANTOS_IDENTITY_PEER_POLICY");
    }
    command
}

/// Names of every variable [`server_serve`] sets or removes; the inventory test compares them.
#[cfg(test)]
fn server_variables() -> Vec<&'static str> {
    SERVER_OWNED
        .iter()
        .copied()
        .chain(SERVER_UNSET)
        .chain(["CANTOS_STUDIO_WEB_ORIGIN", "CANTOS_THEATRE_WEB_ORIGIN"])
        .collect()
}

/// The exact HTTPS origin a local TLS front end serves for media grants; never the API's own
/// HTTP port. Ported from the retired development host script.
pub fn local_media_origin(configured: Option<&str>) -> Result<String, Refusal> {
    let origin = configured.unwrap_or(DEFAULT_MEDIA_ORIGIN);
    let valid = url::Url::parse(origin).ok().is_some_and(|url| {
        url.scheme() == "https"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
            && url.port().is_some_and(|port| port != API_PORT)
            && origin
                == format!(
                    "https://{}:{}",
                    url.host_str().unwrap_or_default(),
                    url.port().unwrap_or_default()
                )
    });
    if valid {
        Ok(origin.to_owned())
    } else {
        Err(Refusal::Usage(format!(
            "CANTOS_LOCAL_MEDIA_ORIGIN must be exactly https://localhost:<port> or \
             https://127.0.0.1:<port>, a TLS front end for the loopback API, not its HTTP port {API_PORT}"
        )))
    }
}

/// Media variables for the Server, only when a seed receipt recorded private/publication roots
/// and a playback key that still sit directly under this composition's media directory.
pub fn media_environment(
    layout: &Layout,
    media_origin: Option<&str>,
) -> Result<Vec<(&'static str, String)>, Refusal> {
    let Ok(text) = std::fs::read_to_string(&layout.seed_receipt) else {
        return Ok(Vec::new());
    };
    let receipt: Value = serde_json::from_str(&text).map_err(|_| {
        Refusal::Usage(
            "invalid Theatre seed receipt; run `cargo xtask seed theatre --reset`".into(),
        )
    })?;
    let expected = [
        ("private_root", layout.media_private()),
        ("publication_root", layout.media_publication()),
        ("playback_key_file", layout.media_key()),
    ];
    let mut values = Vec::new();
    for (field, path) in &expected {
        match receipt.get(field).and_then(Value::as_str) {
            Some(recorded) if Path::new(recorded) == path && path.exists() => {
                values.push(path.display().to_string());
            }
            Some(_) => {
                return Err(Refusal::Usage(format!(
                    "the seed receipt names a {field} outside this composition's media directory; \
                     run `cargo xtask seed theatre --reset`"
                )))
            }
            // An older catalog-only receipt leaves public delivery unavailable.
            None => return Ok(Vec::new()),
        }
    }
    let origin = local_media_origin(media_origin)?;
    Ok(vec![
        ("CANTOS_AUDIO_ROOT", values[0].clone()),
        ("CANTOS_PUBLICATION_ROOT", values[1].clone()),
        ("CANTOS_PLAYBACK_KEY_FILE", values[2].clone()),
        ("CANTOS_PUBLIC_MEDIA_ORIGIN", origin),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> Workspace {
        Workspace::discover().unwrap()
    }

    fn layout() -> Layout {
        Layout::resolve(&workspace(), 501, Path::new("/tmp")).unwrap()
    }

    fn env_of(command: &CommandSpec, key: &str) -> Option<String> {
        command
            .env
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn each_service_owns_one_file_in_its_own_private_directory() {
        let layout = layout();
        let root = workspace().path(DEV_ROOT);
        assert_eq!(layout.server_db, root.join("server/server.db"));
        assert_eq!(layout.identity_db, root.join("identity/identity.db"));
        assert_ne!(layout.server_dir, layout.identity_dir);
        assert!(layout.server_db.starts_with(&layout.server_dir));
        assert!(layout.identity_db.starts_with(&layout.identity_dir));
        // The socket directory is private to the composition and outside both database dirs.
        assert!(layout.socket.starts_with(&layout.socket_dir));
        assert!(!layout.socket_dir.starts_with(&root));
        assert_eq!(layout.private_directories().len(), 4);
    }

    #[test]
    fn the_socket_directory_is_per_checkout_and_per_user_and_must_fit_sun_path() {
        let a = Layout::resolve(&workspace(), 501, Path::new("/tmp")).unwrap();
        let other_user = Layout::resolve(&workspace(), 502, Path::new("/tmp")).unwrap();
        assert_ne!(a.socket_dir, other_user.socket_dir);
        assert!(a.socket.as_os_str().len() <= 100);
        let long = format!("/{}", "x".repeat(90));
        assert!(matches!(
            Layout::resolve(&workspace(), 501, Path::new(&long)),
            Err(Refusal::Usage(message)) if message.contains("socket path")
        ));
    }

    #[test]
    fn migrations_are_the_explicit_owner_binaries_with_one_database_variable_each() {
        let (workspace, layout) = (workspace(), layout());
        let server = migrate_server(&workspace, &layout, Profile::Dev);
        assert_eq!(
            server.display(workspace.root()),
            format!(
                "CANTOS_SERVER_DATABASE={} target/debug/cantos-migrate",
                layout.server_db.display()
            )
        );
        let identity = migrate_identity(&workspace, &layout, Profile::Release);
        assert_eq!(
            identity.display(workspace.root()),
            format!(
                "CANTOS_IDENTITY_DATABASE={} target/release/cantos-identity migrate",
                layout.identity_db.display()
            )
        );
        for command in [&server, &identity] {
            assert!(command.env.iter().all(|(key, _)| key != "DATABASE_URL"));
        }
    }

    #[test]
    fn identity_serves_a_private_owner_only_socket_for_exactly_this_uid() {
        let (workspace, layout) = (workspace(), layout());
        let command = identity_serve(&workspace, &layout, Profile::Dev, None);
        assert_eq!(command.args, ["serve"]);
        assert_eq!(
            env_of(&command, "CANTOS_IDENTITY_SOCKET_MODE").as_deref(),
            Some("0600")
        );
        assert_eq!(
            env_of(&command, "CANTOS_IDENTITY_SERVER_UIDS").as_deref(),
            Some("501")
        );
        assert_eq!(
            env_of(&command, "CANTOS_IDENTITY_SIGNUP_LINK_BASE").as_deref(),
            Some(DEFAULT_SIGNUP_LINK_BASE)
        );
        assert!(DEFAULT_SIGNUP_LINK_BASE.starts_with("https://"));
        assert_eq!(
            env_of(
                &identity_serve(&workspace, &layout, Profile::Dev, Some("https://x.test/s")),
                "CANTOS_IDENTITY_SIGNUP_LINK_BASE"
            )
            .as_deref(),
            Some("https://x.test/s")
        );
    }

    #[test]
    fn the_server_points_at_its_own_file_and_the_identity_socket_with_the_identity_peer_uid() {
        let (workspace, layout) = (workspace(), layout());
        let server = server_serve(
            &workspace,
            &layout,
            Profile::Dev,
            "http://127.0.0.1:8081",
            WithIdentity::Yes,
        );
        assert_eq!(
            env_of(&server, "CANTOS_SERVER_DATABASE").as_deref(),
            Some(layout.server_db.to_str().unwrap())
        );
        assert_eq!(
            env_of(&server, "CANTOS_IDENTITY_SOCKET").as_deref(),
            Some(layout.socket.to_str().unwrap())
        );
        assert_eq!(
            env_of(&server, "CANTOS_IDENTITY_PEER_UID").as_deref(),
            Some("501")
        );
        assert_eq!(
            env_of(&server, "CANTOS_IDENTITY_PEER_POLICY").as_deref(),
            Some("direct_peer")
        );
        assert_eq!(
            env_of(&server, "CANTOS_ENV").as_deref(),
            Some("development")
        );
        assert_eq!(
            env_of(&server, "CANTOS_BIND_ADDRESS").as_deref(),
            Some("127.0.0.1:8080")
        );
        // `direct_peer` rejects a proxy list; an inherited roster or list must not apply.
        for name in [
            "CANTOS_IDENTITY_TRUSTED_PROXIES",
            "CANTOS_IDENTITY_STUDIO_SUBJECTS",
        ] {
            assert!(
                server.env_remove.iter().any(|removed| removed == name),
                "{name}"
            );
        }
    }

    #[test]
    fn the_seed_server_has_no_identity_and_cannot_inherit_one() {
        let (workspace, layout) = (workspace(), layout());
        let server = server_serve(
            &workspace,
            &layout,
            Profile::Dev,
            "http://127.0.0.1:8081",
            WithIdentity::No,
        );
        assert_eq!(env_of(&server, "CANTOS_IDENTITY_SOCKET"), None);
        for name in [
            "CANTOS_IDENTITY_SOCKET",
            "CANTOS_IDENTITY_PEER_UID",
            "CANTOS_IDENTITY_PEER_POLICY",
        ] {
            assert!(
                server.env_remove.iter().any(|removed| removed == name),
                "{name}"
            );
        }
    }

    #[test]
    fn no_retired_provider_or_database_input_reaches_any_child() {
        let (workspace, layout) = (workspace(), layout());
        let commands = [
            migrate_server(&workspace, &layout, Profile::Dev),
            migrate_identity(&workspace, &layout, Profile::Dev),
            identity_serve(&workspace, &layout, Profile::Dev, None),
            operator(&workspace, &layout, Profile::Dev, &["create-creator"]),
            server_serve(
                &workspace,
                &layout,
                Profile::Dev,
                "http://127.0.0.1:8081",
                WithIdentity::Yes,
            ),
        ];
        for command in commands {
            for (key, _) in &command.env {
                for retired in [
                    "DATABASE_URL",
                    "OIDC",
                    "KANIDM",
                    "FIREBASE",
                    "NATIVE_ISSUER",
                    "POSTGRES",
                ] {
                    assert!(!key.to_ascii_uppercase().contains(retired), "{key}");
                }
            }
        }
    }

    #[test]
    fn the_media_origin_is_a_https_loopback_front_end_never_the_api_port() {
        assert_eq!(local_media_origin(None).unwrap(), DEFAULT_MEDIA_ORIGIN);
        assert_eq!(
            local_media_origin(Some("https://127.0.0.1:9443")).unwrap(),
            "https://127.0.0.1:9443"
        );
        for bad in [
            "http://localhost:8443",
            "https://localhost:8080",
            "https://localhost",
            "https://example.test:8443",
            "https://localhost:8443/path",
            "https://user@localhost:8443",
        ] {
            assert!(local_media_origin(Some(bad)).is_err(), "{bad}");
        }
    }

    struct Scratch(PathBuf);
    impl Scratch {
        fn new(tag: &str) -> Self {
            let path = Path::new("/tmp").join(format!(
                "nd-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("Cargo.toml"), "[workspace]\n").unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(unix)]
    #[test]
    fn private_directories_are_created_0700_reused_when_exact_and_never_repaired() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("dirs");
        let workspace = Workspace::fixture(&scratch.0).unwrap();
        let uid = unsafe { libc::geteuid() };
        let layout = Layout::resolve(&workspace, uid, &scratch.0.join("t")).unwrap();
        std::fs::create_dir_all(scratch.0.join("t")).unwrap();
        ensure_private_directories(&layout).unwrap();
        for directory in layout.private_directories() {
            let mode = std::fs::metadata(directory).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "{}", directory.display());
        }
        // A second run reuses exactly-private directories.
        ensure_private_directories(&layout).unwrap();
        // A wider existing directory is refused and left untouched.
        std::fs::set_permissions(&layout.identity_dir, std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let error = ensure_private_directories(&layout).unwrap_err();
        assert!(
            error.to_string().contains("not a private (0700) directory"),
            "{error}"
        );
        assert_eq!(
            std::fs::metadata(&layout.identity_dir)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_data_directory_is_refused() {
        let scratch = Scratch::new("link");
        let workspace = Workspace::fixture(&scratch.0).unwrap();
        let uid = unsafe { libc::geteuid() };
        let layout = Layout::resolve(&workspace, uid, &scratch.0.join("t")).unwrap();
        std::fs::create_dir_all(scratch.0.join("t")).unwrap();
        std::fs::create_dir_all(scratch.0.join("elsewhere")).unwrap();
        std::fs::create_dir_all(&layout.root).unwrap();
        std::os::unix::fs::symlink(scratch.0.join("elsewhere"), layout.root.join("server"))
            .unwrap();
        // The root itself must be created by us before the link check reaches `server`.
        std::fs::set_permissions(
            &layout.root,
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        let error = ensure_private_directories(&layout).unwrap_err();
        assert!(error.to_string().contains("refusing to reuse"), "{error}");
    }

    fn receipt_layout(scratch: &Scratch) -> Layout {
        let workspace = Workspace::fixture(&scratch.0).unwrap();
        let layout = Layout::resolve(&workspace, 501, &scratch.0.join("t")).unwrap();
        std::fs::create_dir_all(layout.media_private()).unwrap();
        std::fs::create_dir_all(layout.media_publication()).unwrap();
        std::fs::write(layout.media_key(), [7u8; 32]).unwrap();
        layout
    }

    #[test]
    fn media_is_enabled_only_by_a_receipt_whose_roots_are_this_compositions_own() {
        let scratch = Scratch::new("media");
        let layout = receipt_layout(&scratch);
        assert_eq!(media_environment(&layout, None).unwrap(), []);
        let receipt = serde_json::json!({
            "private_root": layout.media_private(),
            "publication_root": layout.media_publication(),
            "playback_key_file": layout.media_key(),
        });
        std::fs::write(&layout.seed_receipt, receipt.to_string()).unwrap();
        let env = media_environment(&layout, None).unwrap();
        let names: Vec<_> = env.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "CANTOS_AUDIO_ROOT",
                "CANTOS_PUBLICATION_ROOT",
                "CANTOS_PLAYBACK_KEY_FILE",
                "CANTOS_PUBLIC_MEDIA_ORIGIN"
            ]
        );
        let foreign = serde_json::json!({
            "private_root": "/somewhere/else/private",
            "publication_root": layout.media_publication(),
            "playback_key_file": layout.media_key(),
        });
        std::fs::write(&layout.seed_receipt, foreign.to_string()).unwrap();
        assert!(matches!(
            media_environment(&layout, None),
            Err(Refusal::Usage(message)) if message.contains("outside this composition")
        ));
        // An older catalog-only receipt leaves public delivery unavailable instead of guessing.
        std::fs::write(&layout.seed_receipt, "{}").unwrap();
        assert_eq!(media_environment(&layout, None).unwrap(), []);
        std::fs::write(&layout.seed_receipt, "not json").unwrap();
        assert!(media_environment(&layout, None).is_err());
    }

    #[test]
    fn the_server_variables_xtask_controls_are_declared_in_the_inventory() {
        let manifest = std::fs::read_to_string(workspace().path("services/manifest.toml")).unwrap();
        let declared = |name: &str| manifest.contains(&format!("\"{name}\""));
        for name in server_variables() {
            // Inherited conveniences removed for hygiene are not inputs of the Server.
            let hygiene_only = matches!(name, "CANTOS_WEB_ORIGIN" | "CANTOS_BROWSER_API_SURFACE");
            assert!(
                hygiene_only || declared(name),
                "{name} is not in services/manifest.toml"
            );
        }
    }
}
