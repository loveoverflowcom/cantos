use cantos_api::{
    interfaces::http::{router, AppState},
    platform::postgres::{local_config, Store},
    platform::studio_oidc::{OidcConfig, StudioOidc},
};
use std::{env, net::SocketAddr};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("CANTOS_ENV").ok().as_deref() != Some("development") {
        return Err("this host supports CANTOS_ENV=development only".into());
    }
    let address: SocketAddr = env::var("CANTOS_BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;
    if !address.ip().is_loopback() {
        return Err("development host must bind loopback".into());
    }
    let config =
        local_config(&env::var("DATABASE_URL")?).map_err(|_| "invalid local PostgreSQL config")?;
    // Caller-owned generation enters through authenticated context/submission tools.
    let mut store = Store::new(config).map_err(|_| "database pool unavailable")?;
    if let Some(path) = env::var_os("CANTOS_STORAGE_CONFIG") {
        if env::var_os("CANTOS_AUDIO_ROOT").is_some() {
            return Err("choose CANTOS_STORAGE_CONFIG or CANTOS_AUDIO_ROOT, not both".into());
        }
        store = store
            .with_storage_config(std::path::Path::new(&path))
            .map_err(|_| "invalid private storage configuration or missing secret inputs")?;
    } else if let Some(root) = env::var_os("CANTOS_AUDIO_ROOT") {
        store = store
            .with_audio_storage(std::path::Path::new(&root))
            .map_err(|_| "invalid private development audio storage root")?;
    }
    let dist = env::var("CANTOS_WEB_DIST").unwrap_or_else(|_| "apps/cantos-studio/web/dist".into());
    if store.audio_storage_is_enabled() {
        eprintln!("Private audio intake enabled: API-only host; serve Studio separately through its Trunk proxy");
    }
    let web_origin = env::var("CANTOS_WEB_ORIGIN").unwrap_or_else(|_| format!("http://{address}"));
    if let Ok(issuer) = env::var("CANTOS_STUDIO_OIDC_ISSUER") {
        let secret = env::var("CANTOS_STUDIO_OIDC_SECRET")
            .map_err(|_| "OIDC requires an operator-supplied client secret")?;
        let config = OidcConfig::new(&issuer, &web_origin, secret)
            .map_err(|_| "invalid Studio OIDC config")?;
        let oidc = StudioOidc::new(config).map_err(|_| "OIDC client unavailable")?;
        store = store.with_studio_oidc(oidc);
    } else if env::var_os("CANTOS_STUDIO_OIDC_SECRET").is_some() {
        return Err("OIDC secret requires explicit issuer".into());
    }
    if let Ok(issuer) = env::var("CANTOS_THEATRE_OIDC_ISSUER") {
        let secret = env::var("CANTOS_THEATRE_OIDC_SECRET")
            .map_err(|_| "Theatre OIDC requires its separate operator-supplied client secret")?;
        let identity = cantos_api::platform::theatre_identity::TheatreIdentity::new(
            &issuer,
            &web_origin,
            secret,
        )
        .map_err(|_| "invalid Theatre OIDC configuration")?;
        store = store.with_theatre_identity(identity);
    } else if env::var_os("CANTOS_THEATRE_OIDC_SECRET").is_some() {
        return Err("Theatre secret requires explicit issuer".into());
    }
    if let Ok(issuer) = env::var("CANTOS_THEATRE_NATIVE_ISSUER") {
        let native = cantos_api::platform::theatre_identity::NativeIdentity::new(&issuer)
            .map_err(|_| "invalid separate native public-client identity configuration")?;
        store = store.with_theatre_native(native);
    }
    let state = AppState { store, web_origin };
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Cantos Studio development host: http://{address}");
    axum::serve(listener, router(state, &dist))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
