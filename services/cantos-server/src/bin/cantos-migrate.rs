use cantos_server::platform::store::{local_config, migrate, verify_schema, SCHEMA_EPOCH};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let check_only = match env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err("usage: cantos-migrate [--check]".into()),
    };
    let config =
        local_config(&env::var("DATABASE_URL")?).map_err(|_| "invalid local PostgreSQL config")?;
    if check_only {
        verify_schema(&config).await?;
        println!("Schema epoch {SCHEMA_EPOCH} and checksums verified (read-only)");
    } else {
        migrate(&config).await?;
        println!("Schema epoch {SCHEMA_EPOCH} initialized or checksums verified");
    }
    Ok(())
}
