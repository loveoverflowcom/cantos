use cantos_server::{
    adapters::inbound::http::{server_router, AppState},
    application::Store,
    platform::host::{host_settings, HostMode},
    platform::postgres::{local_config, verify_schema},
    platform::studio_oidc::{OidcConfig, StudioOidc},
};
use std::{env, net::SocketAddr};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let api_origin = match env::var("CANTOS_API_ORIGIN") {
        Ok(value) => {
            cantos_contracts::web_api::ApiOrigin::parse(&value)
                .map_err(|_| "invalid CANTOS_API_ORIGIN")?;
            if host.mode() == HostMode::Production && !value.starts_with("https://") {
                return Err("production CANTOS_API_ORIGIN must be HTTPS".into());
            }
            Some(value)
        }
        Err(env::VarError::NotPresent) => None,
        Err(_) => return Err("invalid CANTOS_API_ORIGIN".into()),
    };
    if let Ok(issuer) = env::var("CANTOS_STUDIO_OIDC_ISSUER") {
        let secret = env::var("CANTOS_STUDIO_OIDC_SECRET")
            .map_err(|_| "OIDC requires an operator-supplied client secret")?;
        let config = OidcConfig::new(&issuer, &web_origin, secret)
            .map_err(|_| "invalid Studio OIDC config")?;
        let config = match api_origin.as_deref() {
            Some(origin) => config
                .with_api_callback_origin(origin)
                .map_err(|_| "invalid Studio API callback origin")?,
            None => config,
        };
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
                cantos_server::platform::theatre_identity::IssuerTrust::read_file(
                    std::path::Path::new(&path),
                )
                .map_err(|_| "invalid Theatre OIDC private CA bundle")?,
            ),
            None => None,
        };
        let identity = cantos_server::platform::theatre_identity::TheatreIdentity::with_api_origin(
            &issuer,
            host.origins().theatre(),
            secret,
            trust,
            api_origin.as_deref(),
        )
        .map_err(|_| "invalid Theatre OIDC configuration")?;
        store = store.with_theatre_identity(identity);
    } else if env::var_os("CANTOS_THEATRE_OIDC_SECRET").is_some()
        || env::var_os("CANTOS_THEATRE_OIDC_CA_FILE").is_some()
    {
        return Err("Theatre secret or private CA requires explicit issuer".into());
    }
    if let Ok(issuer) = env::var("CANTOS_THEATRE_NATIVE_ISSUER") {
        let development_callback =
            match env::var("CANTOS_THEATRE_NATIVE_DEVELOPMENT_CALLBACK").as_deref() {
                Err(env::VarError::NotPresent) => false,
                Ok("admit") => true,
                _ => {
                    return Err(
                        "CANTOS_THEATRE_NATIVE_DEVELOPMENT_CALLBACK must be exactly `admit`".into(),
                    )
                }
            };
        let native = cantos_server::platform::theatre_identity::NativeIdentity::with_callbacks(
            &issuer,
            development_callback,
        )
        .map_err(|_| "invalid separate native public-client identity configuration")?;
        store = store.with_theatre_native(native);
    }
    if let Ok(issuer) = env::var("CANTOS_ADMIN_OIDC_ISSUER") {
        let secret = env::var("CANTOS_ADMIN_OIDC_SECRET")
            .map_err(|_| "Admin requires its dedicated introspection secret")?;
        let config = cantos_server::platform::admin::AdminConfig::new(&issuer, secret)
            .map_err(|_| "invalid Admin configuration")?;
        let admin = cantos_server::platform::admin::AdminService::new(config)
            .map_err(|_| "invalid Admin client")?;
        store = store.with_admin(admin);
    } else if env::var_os("CANTOS_ADMIN_OIDC_SECRET").is_some() {
        return Err("Admin secret requires explicit issuer".into());
    }
    if let Some(registration) = cantos_server::platform::theatre_registration::config::from_lookup(
        store.clone(),
        host.origins().theatre(),
        |name| env::var(name).ok(),
    )
    .map_err(|_| "invalid or incomplete Theatre registration configuration")?
    {
        store = match registration {
            cantos_server::platform::theatre_registration::config::RegistrationMode::Smtp(
                runtime,
            ) => store.with_theatre_registration(runtime),
            cantos_server::platform::theatre_registration::config::RegistrationMode::Firebase(
                runtime,
            ) => store.with_theatre_firebase_enrollment(runtime),
        };
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
