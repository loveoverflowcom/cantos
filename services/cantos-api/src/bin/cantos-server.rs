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
        let subjects = env::var("CANTOS_STUDIO_ADMIN_SUBJECTS")
            .map_err(|_| "Studio requires its operator-provisioned administrator subjects")?;
        let subjects = serde_json::from_str::<Vec<String>>(&subjects)
            .map_err(|_| "Studio administrator subjects must be a JSON array")?;
        let oidc = StudioOidc::new(config)
            .map_err(|_| "OIDC client unavailable")?
            .with_admin_subjects(subjects)
            .map_err(|_| "Studio requires one or two distinct valid administrator subjects")?;
        store = store.with_studio_oidc(oidc);
    } else if env::var_os("CANTOS_STUDIO_OIDC_SECRET").is_some()
        || env::var_os("CANTOS_STUDIO_ADMIN_SUBJECTS").is_some()
    {
        return Err("Studio secret/administrator subjects require explicit issuer".into());
    }
    if let Ok(issuer) = env::var("CANTOS_THEATRE_OIDC_ISSUER") {
        let secret = env::var("CANTOS_THEATRE_OIDC_SECRET")
            .map_err(|_| "Theatre OIDC requires its separate operator-supplied client secret")?;
        // Optional private authority for a private HTTPS issuer; it replaces the public roots for
        // the Theatre issuer's OIDC and onboarding clients only. Never a system trust change.
        let trust = match env::var_os("CANTOS_THEATRE_OIDC_CA_FILE") {
            Some(path) => Some(
                cantos_api::platform::theatre_identity::IssuerTrust::read_file(
                    std::path::Path::new(&path),
                )
                .map_err(|_| "invalid Theatre OIDC private CA bundle")?,
            ),
            None => None,
        };
        let identity = cantos_api::platform::theatre_identity::TheatreIdentity::with_trust(
            &issuer,
            &web_origin,
            secret,
            trust,
        )
        .map_err(|_| "invalid Theatre OIDC configuration")?;
        store = store.with_theatre_identity(identity);
    } else if env::var_os("CANTOS_THEATRE_OIDC_SECRET").is_some()
        || env::var_os("CANTOS_THEATRE_OIDC_CA_FILE").is_some()
    {
        return Err("Theatre secret or private CA requires explicit issuer".into());
    }
    if let Ok(issuer) = env::var("CANTOS_THEATRE_NATIVE_ISSUER") {
        let native = cantos_api::platform::theatre_identity::NativeIdentity::new(&issuer)
            .map_err(|_| "invalid separate native public-client identity configuration")?;
        store = store.with_theatre_native(native);
    }
    if let Ok(issuer) = env::var("CANTOS_ADMIN_OIDC_ISSUER") {
        let secret = env::var("CANTOS_ADMIN_OIDC_SECRET")
            .map_err(|_| "Admin requires its dedicated introspection secret")?;
        let config = cantos_api::platform::admin::AdminConfig::new(&issuer, secret)
            .map_err(|_| "invalid Admin configuration")?;
        let admin = cantos_api::platform::admin::AdminService::new(config)
            .map_err(|_| "invalid Admin client")?;
        store = store.with_admin(admin);
    } else if env::var_os("CANTOS_ADMIN_OIDC_SECRET").is_some() {
        return Err("Admin secret requires explicit issuer".into());
    }
    if let Some(registration) = cantos_api::platform::theatre_registration::config::from_lookup(
        store.clone(),
        &web_origin,
        |name| env::var(name).ok(),
    )
    .map_err(|_| "invalid or incomplete Theatre registration configuration")?
    {
        store = match registration {
            cantos_api::platform::theatre_registration::config::RegistrationMode::Smtp(runtime) => {
                store.with_theatre_registration(runtime)
            }
            cantos_api::platform::theatre_registration::config::RegistrationMode::Firebase(
                runtime,
            ) => store.with_theatre_firebase_enrollment(runtime),
        };
    }
    let state = AppState { store, web_origin };
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Cantos Studio development host: http://{address}");
    axum::serve(
        listener,
        router(state, &dist).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
