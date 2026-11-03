//! Source-pinned disposable test actor. Explicitly absent from production image packaging.
use cantos_identity::persistence::IdentityStore;
use cantos_store_turso::{clock::SystemClock, registry::Intent};
use std::{fs, io::Read, os::unix::fs::MetadataExt, path::Path, process::ExitCode, sync::Arc};

async fn run() -> Result<(), ()> {
    if std::env::args().len() != 1 {
        return Err(());
    }
    let source = option_env!("CANTOS_TEST_SOURCE_SHA").ok_or(())?;
    if source.len() != 40 || !source.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(());
    }
    let path = Path::new("/fixture/marker.json");
    let named = fs::symlink_metadata(path).map_err(|_| ())?;
    let file = fs::File::open(path).map_err(|_| ())?;
    let meta = file.metadata().map_err(|_| ())?;
    if !named.is_file()
        || !meta.is_file()
        || meta.uid() != 0
        || meta.nlink() != 1
        || meta.mode() & 0o022 != 0
        || meta.len() > 1024
        || named.dev() != meta.dev()
        || named.ino() != meta.ino()
    {
        return Err(());
    }
    let mut bytes = Vec::new();
    file.take(1025).read_to_end(&mut bytes).map_err(|_| ())?;
    let marker: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| ())?;
    let expected = serde_json::json!({"kind":"DISPOSABLE_CANTOS_RECOVERY_TEST", "source_sha":source,
        "database":"/database/identity.db", "mailbox":"recovery-disabled@example.test"});
    if marker != expected {
        return Err(());
    }
    let store = IdentityStore::open_with_recovery_fence(
        "/database/identity.db",
        Intent::Serve,
        Arc::new(SystemClock),
        Some("/fence/current".into()),
    )
    .await
    .map_err(|_| ())?;
    store
        .disable_disposable_recovery_account()
        .await
        .map_err(|_| ())?;
    Ok(())
}

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return ExitCode::FAILURE,
    };
    match runtime.block_on(run()) {
        Ok(()) => {
            println!(
                "{{\"status\":\"PASS\",\"operation\":\"DISABLE_EXISTING_SYNTHETIC_ACCOUNT\"}}"
            );
            ExitCode::SUCCESS
        }
        Err(()) => {
            eprintln!("disposable recovery fixture refused");
            ExitCode::FAILURE
        }
    }
}
