use cantos_server::platform::turso::{registry, SERVER_EPOCH};
use cantos_server::store::Store;
use cantos_store_turso::{clock::SystemClock, registry::Intent, OpenRequest, OwnedDatabase};
use std::{env, sync::Arc};

/// Creates or upgrades `server.db` (the explicit operator action) or, with `--check`, verifies it
/// read-only exactly as the serving process does. The Server never migrates at startup.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let path = env::var_os("CANTOS_SERVER_DATABASE").ok_or("CANTOS_SERVER_DATABASE is required")?;
    let fence = env::var_os("CANTOS_SERVER_RECOVERY_FENCE").map(std::path::PathBuf::from);
    if arguments
        .first()
        .is_some_and(|argument| argument.starts_with("--recovery-"))
    {
        let fence = fence
            .as_ref()
            .ok_or("CANTOS_SERVER_RECOVERY_FENCE is required")?;
        let request = || {
            OpenRequest::new(
                &path,
                registry().expect("static registry"),
                Intent::Serve,
                Arc::new(SystemClock),
            )
            .with_recovery_fence(fence)
        };
        let owner = match arguments.as_slice() {
            [command] if command == "--recovery-observe" => OwnedDatabase::open(request()).await?,
            [command] if command == "--recovery-reconcile" => OwnedDatabase::reconcile_recovery_fence(request()).await?,
            [command] if command == "--recovery-enroll" => match OwnedDatabase::open(request()).await {
                Ok(owner) => owner,
                Err(cantos_store_turso::error::OpenError::Store(cantos_store_turso::error::StoreFault::Recovery(cantos_store_turso::recovery::RecoveryFault::Unenrolled))) => {
                    OwnedDatabase::open(OpenRequest::new(&path, registry()?, Intent::Serve, Arc::new(SystemClock))).await?
                        .enroll_recovery_fence(fence).await?
                },
                Err(error) => return Err(error.into()),
            },
            [command] if command == "--recovery-retire-sessions" => OwnedDatabase::open(request()).await?,
            [command, destination] if command == "--recovery-export" && std::path::Path::new(destination).is_absolute() => OwnedDatabase::open(request()).await?,
            _ => return Err("usage: --recovery-enroll | --recovery-reconcile | --recovery-export ABSOLUTE_DESTINATION | --recovery-retire-sessions".into()),
        };
        owner.check_integrity().await?;
        let mut backup = None;
        if arguments[0] == "--recovery-export" {
            let receipt = owner.backup_to(std::path::Path::new(&arguments[1])).await?;
            backup = Some(serde_json::json!({"sha256":hex(&receipt.sha256),"bytes":receipt.bytes}));
        }
        if arguments[0] == "--recovery-retire-sessions" {
            cantos_server::platform::identity_auth::turso::retire_restored_sessions(&owner).await?;
        }
        let observed = owner.recovery_observation().await?;
        let retired =
            cantos_server::platform::identity_auth::turso::all_sessions_retired(&owner).await?;
        println!(
            "{}",
            serde_json::json!({"owner":"cantos-server","schema":registry()?.latest(),
            "instance":hex(&observed.instance),"revision":observed.revision,"backup":backup,
            "all_prior_sessions_retired":retired,"exposure":"NOT_AUTHORIZED"})
        );
        return Ok(());
    }
    let check_only = match arguments.as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => {
            return Err(
                "usage: cantos-migrate [--check]  (database file: CANTOS_SERVER_DATABASE)".into(),
            )
        }
    };
    let versions = registry()
        .map_err(|_| "invalid server schema registry")?
        .latest();
    if check_only {
        let mut request =
            OpenRequest::new(&path, registry()?, Intent::Serve, Arc::new(SystemClock));
        request.recovery_fence = fence;
        let owner = OwnedDatabase::open(request).await?;
        owner.check_integrity().await?;
        println!("server.db epoch {SERVER_EPOCH}, version {versions}: registry and checksums verified (read-only)");
    } else {
        Store::open_with_recovery(&path, Intent::Migrate, fence, None).await?;
        println!("server.db epoch {SERVER_EPOCH}, version {versions}: created or upgraded, checksums verified");
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
