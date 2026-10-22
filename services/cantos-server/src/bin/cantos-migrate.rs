use cantos_server::platform::turso::{registry, SERVER_EPOCH};
use cantos_server::store::Store;
use cantos_store_turso::registry::Intent;
use std::env;

/// Creates or upgrades `server.db` (the explicit operator action) or, with `--check`, verifies it
/// read-only exactly as the serving process does. The Server never migrates at startup.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let check_only = match env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err("usage: cantos-migrate [--check]  (database file: CANTOS_SERVER_DATABASE)".into()),
    };
    let path = env::var_os("CANTOS_SERVER_DATABASE").ok_or("CANTOS_SERVER_DATABASE is required")?;
    let versions = registry().map_err(|_| "invalid server schema registry")?.latest();
    if check_only {
        Store::open(&path, Intent::Serve).await?;
        println!("server.db epoch {SERVER_EPOCH}, version {versions}: registry and checksums verified (read-only)");
    } else {
        Store::open(&path, Intent::Migrate).await?;
        println!("server.db epoch {SERVER_EPOCH}, version {versions}: created or upgraded, checksums verified");
    }
    Ok(())
}
