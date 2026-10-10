use std::{env, error::Error, net::SocketAddr};

use rand::{RngCore, rngs::OsRng};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

use cantos_server::{auth::token_hash, migrate, router};

#[tokio::main]
async fn main() {
    if run().await.is_err() {
        // Do not expose connection strings, credential values or manuscript-bearing SQL errors.
        eprintln!("Cantos server command failed; check configuration and database availability.");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let database_url = env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("migrate") if args.len() == 1 => { migrate(&pool).await?; }
        Some("create-creator") if args.len() == 1 => {
            let creator = Uuid::new_v4();
            sqlx::query("INSERT INTO creators (id) VALUES ($1)").bind(creator).execute(&pool).await?;
            println!("{creator}");
        }
        Some("issue-token") if args.len() == 3 => {
            let creator: Uuid = args[1].parse()?;
            let hours: i64 = args[2].parse()?;
            if !(1..=720).contains(&hours) { return Err("token expiry must be 1..720 hours".into()); }
            issue_token(&pool, creator, hours).await?;
        }
        Some("revoke-tokens") if args.len() == 2 => {
            let creator: Uuid = args[1].parse()?;
            sqlx::query("UPDATE creator_tokens SET revoked_at = clock_timestamp() WHERE creator_id = $1 AND revoked_at IS NULL")
                .bind(creator).execute(&pool).await?;
        }
        Some("serve") | None if args.len() <= 1 => {
            migrate(&pool).await?;
            let address: SocketAddr = env::var("CANTOS_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".into()).parse()?;
            let listener = tokio::net::TcpListener::bind(address).await?;
            eprintln!("Cantos server listening on {}", listener.local_addr()?);
            axum::serve(listener, router(pool)).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await?;
        }
        _ => return Err("usage: cantos-server [serve|migrate|create-creator|issue-token CREATOR HOURS|revoke-tokens CREATOR]".into()),
    }
    Ok(())
}

async fn issue_token(pool: &PgPool, creator: Uuid, hours: i64) -> Result<(), Box<dyn Error>> {
    let mut bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut bytes)?;
    let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let result = sqlx::query("INSERT INTO creator_tokens (token_hash,creator_id,expires_at) SELECT $1,id,clock_timestamp() + ($3 * interval '1 hour') FROM creators WHERE id = $2 AND NOT disabled")
        .bind(token_hash(&token)).bind(creator).bind(hours as f64).execute(pool).await?;
    if result.rows_affected() != 1 {
        return Err("creator unavailable".into());
    }
    // The operator explicitly requested a credential; this is its only plaintext output.
    println!("{token}");
    Ok(())
}
