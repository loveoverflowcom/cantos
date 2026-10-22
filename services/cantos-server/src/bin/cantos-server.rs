use cantos_server::{
    adapters::inbound::http::{server_router, AppState},
    application::Store,
    platform::host::{host_settings, retired_provider_input, HostMode},
    platform::postgres::{local_config, verify_schema},
};
use std::{env, net::SocketAddr};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Provider authentication (Kanidm/OIDC, legacy native client, Firebase/SMTP registration) is
    // retired: refuse its inputs instead of silently running without the path they configured.
    if let Some(name) = retired_provider_input(
        env::vars_os().filter_map(|(name, _)| name.into_string().ok()),
    ) {
        return Err(format!(
            "{name} configures a retired provider authentication path; unset it (Identity v1 via CANTOS_IDENTITY_* is the only session authority)"
        )
        .into());
    }
    let host = host_settings(|name| env::var(name).ok()).map_err(|error| error.message())?;
    let address = host.bind();
    let config =
        local_config(&env::var("DATABASE_URL")?).map_err(|_| "invalid local PostgreSQL config")?;
    verify_schema(&config).await?;
    // Caller-owned generation enters through authenticated context/submission tools.
    let mut store = Store::new(config).map_err(|_| "database pool unavailable")?;
    if !host.development_sessions() {
        store = store.without_development_sessions();
    }
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
    if store.audio_storage_is_enabled() && host.mode() == HostMode::Development {
        eprintln!("Private audio intake enabled: API-only host; serve Studio separately through its Trunk proxy");
    }
    let web_origin = host.web_origin().to_owned();
    // Email/password sessions from a verified cantos-identity fact. Optional and explicit: with no
    // socket configured, none of the Identity routes or credentials exist.
    if let Some(config) =
        cantos_server::platform::identity_auth::config::from_lookup(|name| env::var(name).ok())
            .map_err(|_| "invalid or incomplete Identity password-session configuration")?
    {
        // One cookie spelling for the whole process: refuse a mix of https and http surfaces.
        let secure = host.origins().studio().starts_with("https://");
        if secure != host.origins().theatre().starts_with("https://") {
            return Err(
                "Identity cookies need the Studio and Theatre origins to share a scheme".into(),
            );
        }
        store = store.with_identity_auth(
            cantos_server::platform::identity_auth::IdentityAuth::new(config, secure),
        );
    }
    let public_media =
        cantos_server::contexts::media::public_media::from_lookup(|name| env::var(name).ok())
            .map_err(|error| error.message())?;
    let state = AppState { store, web_origin };
    let public_media = public_media.map(|runtime| {
        let publication =
            cantos_server::contexts::media::PublicationService::new(state.store.clone())
                .with_origin_publication(runtime.storage);
        (publication, runtime.playback)
    });
    let app = server_router(state, host.origins(), host.web_dist(), public_media);
    let listener = tokio::net::TcpListener::bind(address).await?;
    match host.mode() {
        HostMode::Development => eprintln!("Cantos Server development host: http://{address}"),
        HostMode::Production => eprintln!("Cantos API production host listening on {address}"),
    }
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

/// Ctrl-C for a foreground development host; SIGTERM for a container or service manager.
async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
}
