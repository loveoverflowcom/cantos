use cantos_api::platform::postgres::{local_config, migrate};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config =
        local_config(&env::var("DATABASE_URL")?).map_err(|_| "invalid local PostgreSQL config")?;
    migrate(&config)
        .await
        .map_err(|_| "migration failed; no partial transaction accepted")?;
    println!("Migrations applied or checksums verified");
    Ok(())
}
