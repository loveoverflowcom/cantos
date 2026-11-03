//! Explicit offline operator commands. No RPC endpoint, credential export or exposure grant.
use crate::{
    config::MigrateConfig,
    domain::ids::{InstanceEpoch, OperationId},
    persistence::{registry, IdentityStore, IdentityStoreError},
};
use cantos_store_turso::{
    clock::SystemClock,
    error::{OpenError, StoreFault},
    recovery::{recovery_id, RecoveryFault},
    registry::Intent,
    OpenRequest, OwnedDatabase,
};
use std::{path::PathBuf, sync::Arc};

pub enum Command {
    Observe,
    Enroll,
    Reconcile,
    Export(PathBuf),
    Rotate {
        operation: OperationId,
        expected: InstanceEpoch,
    },
}

impl Command {
    pub fn parse(arguments: &[String]) -> Result<Self, &'static str> {
        match arguments {
            [name] if name == "recovery-observe" => Ok(Self::Observe),
            [name] if name == "recovery-enroll" => Ok(Self::Enroll),
            [name] if name == "recovery-reconcile" => Ok(Self::Reconcile),
            [name, destination] if name == "recovery-export" && PathBuf::from(destination).is_absolute() => Ok(Self::Export(destination.into())),
            [name, operation, epoch] if name == "recovery-rotate" => Ok(Self::Rotate {
                operation: OperationId::from_bytes(recovery_id(operation).map_err(|_| "invalid recovery operation")?).map_err(|_| "invalid recovery operation")?,
                expected: InstanceEpoch::from_bytes(recovery_id(epoch).map_err(|_| "invalid recovery epoch")?).map_err(|_| "invalid recovery epoch")?,
            }),
            _ => Err("usage: recovery-enroll | recovery-reconcile | recovery-export ABSOLUTE_DESTINATION | recovery-rotate OPERATION_HEX EXPECTED_EPOCH_HEX"),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub async fn run(
    config: &MigrateConfig,
    command: Command,
) -> Result<serde_json::Value, &'static str> {
    let fence = config
        .recovery_fence
        .as_ref()
        .ok_or("CANTOS_IDENTITY_RECOVERY_FENCE is required")?;
    let clock = || Arc::new(SystemClock) as Arc<dyn cantos_store_turso::clock::Clock>;
    if matches!(command, Command::Reconcile) {
        let request = OpenRequest::new(
            &config.database,
            registry().map_err(|_| "invalid registry")?,
            Intent::Serve,
            clock(),
        )
        .with_recovery_fence(fence);
        let owner = OwnedDatabase::reconcile_recovery_fence(request)
            .await
            .map_err(|_| "recovery reconciliation refused")?;
        owner
            .check_integrity()
            .await
            .map_err(|_| "recovery integrity refused")?;
        drop(owner);
    }
    let opened = IdentityStore::open_with_recovery_fence(
        &config.database,
        Intent::Serve,
        clock(),
        Some(fence.clone()),
    )
    .await;
    let mut store = match (opened, &command) {
        (Ok(store), _) => store,
        (
            Err(IdentityStoreError::Open(OpenError::Store(StoreFault::Recovery(
                RecoveryFault::Unenrolled,
            )))),
            Command::Enroll,
        ) => IdentityStore::open(&config.database, Intent::Serve, clock())
            .await
            .map_err(|_| "recovery source refused")?
            .enroll_recovery_fence(fence)
            .await
            .map_err(|_| "recovery enrollment refused")?,
        _ => return Err("recovery source is missing, stale, uncertain or unavailable"),
    };
    store
        .check_integrity()
        .await
        .map_err(|_| "recovery integrity refused")?;
    let mut backup = None;
    match command {
        Command::Rotate {
            operation,
            expected,
        } => {
            store = store
                .rotate_restore_epoch(operation, expected)
                .await
                .map_err(|_| {
                    "recovery epoch transition refused; reconcile exact operation before retry"
                })?;
        }
        Command::Export(destination) => {
            let receipt = store
                .recovery_backup(&destination)
                .await
                .map_err(|_| "recovery source export refused")?;
            backup = Some(serde_json::json!({"sha256":hex(&receipt.sha256),"bytes":receipt.bytes}));
        }
        _ => (),
    }
    let observed = store
        .recovery_observation()
        .await
        .map_err(|_| "recovery freshness refused")?;
    Ok(
        serde_json::json!({"owner":"cantos-identity","schema":registry().map_err(|_| "invalid registry")?.latest(),
        "instance":hex(&observed.instance),"revision":observed.revision,"epoch":hex(&store.epoch().to_bytes()),
        "backup":backup,"exposure":"NOT_AUTHORIZED"}),
    )
}
