//! Run-owned Auth Emulator lifecycle. No CLI, installer, credential discovery or cloud path.
//! Identity owns the separate native proof/credential protocol; emulator readiness is not auth.

use std::{
    io::{BufRead, BufReader, Read},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::{process::Supervisor, run_files, workspace::Workspace};

pub const VERSION: &str = "14.2.2";
const MODULE_VARIABLE: &str = "CANTOS_FIREBASE_AUTH_MODULE";
pub const CLOUD_VARIABLES: &[&str] = &[
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_CREDENTIALS",
    "GOOGLE_OAUTH_ACCESS_TOKEN",
    "CLOUDSDK_AUTH_CREDENTIAL_FILE_OVERRIDE",
    "CLOUDSDK_AUTH_ACCESS_TOKEN",
    "CLOUDSDK_CONFIG",
    "FIREBASE_TOKEN",
    "FIREBASE_CONFIG",
    "FIREBASE_PROJECT",
    "GCLOUD_PROJECT",
    "GOOGLE_CLOUD_PROJECT",
    "FIREBASE_AUTH_EMULATOR_HOST",
    "FIREBASE_AUTH_EMULATOR_URL",
];

/// A credential-bearing environment refuses the local composition rather than silently using
/// it. Only names enter the diagnostic, including unknown future provider-specific variables.
pub fn reject_inherited_configuration(names: impl IntoIterator<Item = String>) -> Result<()> {
    for name in names {
        if CLOUD_VARIABLES.contains(&name.as_str())
            || name.starts_with("CANTOS_IDENTITY_SMTP_")
            || name.starts_with("CANTOS_IDENTITY_FIREBASE_")
            || name.starts_with("CANTOS_THEATRE_REGISTRATION_FIREBASE_")
        {
            bail!("local Firebase composition refuses inherited {name}; unset it in this shell; no cloud fallback is available");
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Installed {
    node: PathBuf,
    module: PathBuf,
}

fn executable(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|path| path.join(name))
        .find(|path| path.is_file())
}

impl Installed {
    /// Filesystem inspection only; never invokes firebase, npx, an installer or a login.
    pub fn inspect() -> Result<Self> {
        let node = executable("node").context("Firebase Auth Emulator needs an installed Node.js executable on PATH; xtask never installs tools")?;
        let module = std::env::var_os(MODULE_VARIABLE).map(PathBuf::from).or_else(|| {
            let cli = executable("firebase")?.canonicalize().ok()?;
            Some(cli.parent()?.parent()?.join("emulator/auth/server.js"))
        }).context("Firebase Auth Emulator is missing: prepare firebase-tools 14.2.2 separately and set CANTOS_FIREBASE_AUTH_MODULE to its absolute lib/emulator/auth/server.js; no download or cloud fallback occurs")?;
        let module = module.canonicalize().context(
            "CANTOS_FIREBASE_AUTH_MODULE must name the installed lib/emulator/auth/server.js",
        )?;
        let package = module
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(|root| root.join("package.json"))
            .context("invalid installed Firebase Auth module layout")?;
        let metadata: serde_json::Value = serde_json::from_slice(
            &std::fs::read(package).context("read installed Firebase tooling metadata")?,
        )
        .context("invalid installed Firebase tooling metadata")?;
        if module.file_name().and_then(|name| name.to_str()) != Some("server.js")
            || metadata["name"] != "firebase-tools"
            || metadata["version"] != VERSION
        {
            bail!("Auth Emulator requires reviewed firebase-tools 14.2.2; Hosting/App Distribution have a separate CLI version contract; xtask never upgrades either");
        }
        Ok(Self { node, module })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ready {
    host: IpAddr,
    port: u16,
    project: String,
    version: String,
    outbound_guard: bool,
}

/// Only a freshly allocated private directory is ever removed. Drop this AFTER the supervisor
/// has stopped/reaped its child; no export/import or shared emulator registry is used.
pub struct Run {
    directory: PathBuf,
    pub address: SocketAddr,
    pub project: String,
    pub key_file: PathBuf,
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

struct StartingChild(Option<Child>);
impl Drop for StartingChild {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Run {
    pub fn start(
        workspace: &Workspace,
        installed: &Installed,
        supervisor: &mut Supervisor,
    ) -> Result<Self> {
        let directory = run_files::create_run(workspace.root(), "target/auth-emulator", &[])
            .map_err(anyhow::Error::msg)?;
        let project = format!(
            "demo-cantos-{}",
            &run_files::sha256_hex(directory.id.as_bytes())[..12]
        );
        let mut run = Self {
            key_file: directory.path.join("proof-envelope.key"),
            directory: directory.path,
            address: SocketAddr::from(([127, 0, 0, 1], 1)),
            project,
        };
        let mut key = [0u8; 32];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut key))
            .context("local envelope-key entropy unavailable")?;
        run_files::write_private(
            &run_files::RunDir {
                id: directory.id,
                path: run.directory.clone(),
            },
            "proof-envelope.key",
            &key,
        )
        .map_err(anyhow::Error::msg)?;
        key.fill(0);
        let mut command = Command::new(&installed.node);
        command
            .arg(workspace.path("infra/firebase/auth-emulator/launcher.cjs"))
            .arg(&installed.module)
            .arg(&run.project)
            .env_clear()
            .env("XDG_CONFIG_HOME", run.directory.join("config"))
            .env(
                "FIREBASE_EMULATORS_PATH",
                run.directory.join("unused-download-cache"),
            )
            .current_dir(&run.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child =
            StartingChild(Some(command.spawn().context(
                "could not start installed Auth Emulator; no fallback was attempted",
            )?));
        let stdout = child
            .0
            .as_mut()
            .and_then(|child| child.stdout.take())
            .context("Auth Emulator readiness pipe unavailable")?;
        let (send, receive) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = Vec::new();
            let result = (&mut reader).take(4097).read_until(b'\n', &mut line);
            let ready = result.ok().filter(|size| *size <= 4096).map(|_| line);
            let _ = send.send(ready);
            // Never retain provider links/tokens or diagnostic bodies.
            let _ = std::io::copy(&mut reader, &mut std::io::sink());
        });
        let line = receive.recv_timeout(Duration::from_secs(10)).ok().flatten();
        let failure = line
            .as_ref()
            .and_then(|line| serde_json::from_slice::<serde_json::Value>(line).ok());
        if failure
            .as_ref()
            .and_then(|value| value["failure"].as_str())
            .is_some_and(|failure| matches!(failure, "EPERM" | "EACCES"))
        {
            bail!("Auth Emulator loopback listener was denied by this execution environment; run this local-only command with loopback process access; no cloud fallback was attempted");
        }
        let ready = line.and_then(|line| serde_json::from_slice::<Ready>(&line).ok())
            .context("Auth Emulator failed its 10s readiness deadline; check Node.js and the pinned 14.2.2 module; provider output was suppressed to protect actions")?;
        if ready.host != IpAddr::V4(Ipv4Addr::LOCALHOST)
            || ready.port == 0
            || ready.project != run.project
            || ready.version != VERSION
            || !ready.outbound_guard
        {
            bail!("Auth Emulator returned invalid readiness metadata");
        }
        run.address = SocketAddr::new(ready.host, ready.port);
        let owned = child.0.take().context("Auth Emulator child missing")?;
        supervisor.adopt("Firebase Auth Emulator", owned);
        Ok(run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post(
        address: SocketAddr,
        path: &str,
        body: serde_json::Value,
        owner: bool,
    ) -> (u16, serde_json::Value) {
        use std::{io::Write, net::TcpStream};
        let body = serde_json::to_vec(&body).unwrap();
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(3)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let owner = if owner {
            "Authorization: Bearer owner\r\n"
        } else {
            ""
        };
        write!(stream, "POST {path} HTTP/1.1\r\nHost: {address}\r\n{owner}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
        stream.write_all(&body).unwrap();
        let mut response = Vec::new();
        stream.take(65537).read_to_end(&mut response).unwrap();
        assert!(response.len() <= 65536, "bounded emulator response");
        let response = String::from_utf8(response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
        let bytes = if headers
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
        {
            let mut remaining = body.as_bytes();
            let mut bytes = Vec::new();
            loop {
                let end = remaining
                    .windows(2)
                    .position(|value| value == b"\r\n")
                    .unwrap();
                let size =
                    usize::from_str_radix(std::str::from_utf8(&remaining[..end]).unwrap(), 16)
                        .unwrap();
                remaining = &remaining[end + 2..];
                if size == 0 {
                    break;
                }
                bytes.extend_from_slice(&remaining[..size]);
                assert!(remaining[size..].starts_with(b"\r\n"));
                remaining = &remaining[size + 2..];
            }
            bytes
        } else {
            body.as_bytes().to_vec()
        };
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[test]
    fn cloud_and_mail_configuration_is_refused_by_name_without_values() {
        for name in CLOUD_VARIABLES.iter().copied().chain([
            "CANTOS_IDENTITY_SMTP_PASSWORD",
            "CANTOS_IDENTITY_FIREBASE_FUTURE_SECRET",
            "CANTOS_THEATRE_REGISTRATION_FIREBASE_SERVICE_ACCOUNT_FILE",
        ]) {
            let error = reject_inherited_configuration([name.to_owned()]).unwrap_err();
            assert!(error.to_string().contains(name));
        }
        reject_inherited_configuration(["PATH".into(), MODULE_VARIABLE.into()]).unwrap();
    }

    #[test]
    #[ignore = "requires prebuilt Identity Firebase integration-test executable and installed Auth Emulator; no build/install"]
    fn native_identity_signup_uses_the_owned_emulator_and_real_private_rpc() {
        let binary = std::env::var_os("CANTOS_TEST_FIREBASE_TEST_BINARY")
            .map(PathBuf::from)
            .expect(
                "build the Identity Firebase test first, then pass its absolute executable path",
            );
        assert!(binary.is_absolute() && binary.is_file());
        let workspace = Workspace::discover().unwrap();
        let installed = Installed::inspect().unwrap();
        let run;
        let mut supervisor = Supervisor::new().unwrap();
        run = Run::start(&workspace, &installed, &mut supervisor).unwrap();
        let status = Command::new(binary)
            .args([
                "--exact",
                "real_emulator_pending_proof_confirmation_password_login_and_restart",
                "--ignored",
                "--nocapture",
            ])
            .env_clear()
            .env("CANTOS_TEST_FIREBASE_PROJECT", &run.project)
            .env("CANTOS_TEST_FIREBASE_ENDPOINT", run.address.to_string())
            .env("CANTOS_TEST_FIREBASE_KEY_FILE", &run.key_file)
            .current_dir(workspace.root())
            .status()
            .unwrap();
        supervisor.shutdown();
        assert!(
            status.success(),
            "real Identity/Firebase RPC integration failed; no mock success was substituted"
        );
    }

    #[test]
    #[ignore = "requires explicitly prepared firebase-tools 14.2.2 and Node.js; no install"]
    fn installed_emulator_is_isolated_supervised_and_has_an_empty_action_inventory() {
        let workspace = Workspace::discover().unwrap();
        let installed = Installed::inspect().unwrap();
        let directory;
        let address;
        {
            // Declaration order ensures the supervisor stops before the private directory drops.
            let run;
            let mut supervisor = Supervisor::new().unwrap();
            run = Run::start(&workspace, &installed, &mut supervisor).unwrap();
            directory = run.directory.clone();
            address = run.address;
            let (status, body) = crate::probe::http_get(
                address,
                &format!("/emulator/v1/projects/{}/oobCodes", run.project),
            )
            .unwrap();
            assert_eq!(status, 200);
            let body: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(body["oobCodes"], serde_json::json!([]));
            // External action API only. This is deliberately NOT a native signup/session test.
            let email = "recipient@example.test";
            let uid = "dedicated-emulator-boundary-fixture";
            let (status, created) = post(
                address,
                &format!(
                    "/identitytoolkit.googleapis.com/v1/projects/{}/accounts",
                    run.project
                ),
                serde_json::json!({"localId": uid, "email": email, "emailVerified": false}),
                true,
            );
            assert_eq!(status, 200);
            assert!(created["localId"] == uid);
            let endpoint = |action| {
                format!("/identitytoolkit.googleapis.com/v1/accounts:{action}?key=synthetic-fixture-key")
            };
            // The emulator accepts synthetic JSON custom tokens; never production evidence.
            let (status, signed_in) = post(
                address,
                &endpoint("signInWithCustomToken"),
                serde_json::json!({"token": serde_json::json!({"uid": uid}).to_string(), "returnSecureToken": true}),
                false,
            );
            assert_eq!(status, 200);
            let token = signed_in["idToken"].as_str().unwrap();
            let (status, sent) = post(
                address,
                &endpoint("sendOobCode"),
                serde_json::json!({"requestType": "VERIFY_EMAIL", "idToken": token,
                    "continueUrl": "http://127.0.0.1:8082/?operation=public-locator"}),
                false,
            );
            assert_eq!(status, 200);
            assert!(
                sent.get("oobCode").is_none(),
                "normal send does not return the proof"
            );
            let (status, inventory) = crate::probe::http_get(
                address,
                &format!("/emulator/v1/projects/{}/oobCodes", run.project),
            )
            .unwrap();
            assert_eq!(status, 200);
            let inventory: serde_json::Value = serde_json::from_str(&inventory).unwrap();
            let actions = inventory["oobCodes"].as_array().unwrap();
            assert_eq!(actions.len(), 1);
            let action = actions
                .iter()
                .find(|action| action["email"] == email && action["requestType"] == "VERIFY_EMAIL")
                .unwrap();
            let code = action["oobCode"].as_str().unwrap();
            let (status, inspected) = post(
                address,
                &endpoint("resetPassword"),
                serde_json::json!({"oobCode": code}),
                false,
            );
            assert_eq!(status, 200);
            assert!(inspected["requestType"] == "VERIFY_EMAIL" && inspected["email"] == email);
            let (status, consumed) = post(
                address,
                &endpoint("update"),
                serde_json::json!({"oobCode": code}),
                false,
            );
            assert_eq!(status, 200);
            assert!(consumed["localId"] == uid && consumed["email"] == email);
            let (status, _) = post(
                address,
                &endpoint("update"),
                serde_json::json!({"oobCode": code}),
                false,
            );
            assert_eq!(status, 400, "the exact action cannot be consumed twice");
            supervisor.shutdown();
        }
        assert!(!crate::probe::accepts_connections(address));
        assert!(!directory.exists());
    }
}
