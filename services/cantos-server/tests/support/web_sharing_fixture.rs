//! Explicit disposable browser host: real Identity process, separate Turso files, actual supplied
//! audio bytes and production cookie/CORS/private routes. Test-owned synthetic content/accounts;
//! not a publication pipeline, production activation or persistent localhost dataset command.
use super::*;
use cantos_server::adapters::inbound::http::router_with_restricted_media;
use cantos_server::contract::studio::{ViewerAccessChange, ViewerAccessRequest, ViewerAudience};
use std::{path::PathBuf, time::Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "explicit #654 browser qualification; requires prebuilt Identity and supplied 60-second mono Opus"]
async fn web_sharing_browser_host() {
    let _ = supplied_identity();
    let root = PathBuf::from(
        std::env::var_os("CANTOS_WEB_SHARING_FIXTURE").expect("explicit run-owned fixture output"),
    );
    assert!(root.is_absolute() && root.is_dir());
    assert!(!root.join("ready.json").exists(), "new run only");
    let media_path = PathBuf::from(
        std::env::var_os("CANTOS_WEB_SHARING_AUDIO").expect("inspected synthetic Opus fixture"),
    );
    let bytes = std::fs::read(media_path).unwrap();
    assert!(!bytes.is_empty() && bytes.len() <= 32 * 1024 * 1024);
    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let _identity = env.serve().await;
    let fixture = ServerFixture::new().await;
    let bootstrap = compose(&fixture, &env);
    let owner = enroll(&bootstrap, &sink, OWNER, 1).await;
    let reader = enroll(&bootstrap, &sink, SECOND, 2).await;
    let _stranger = enroll(&bootstrap, &sink, "stranger@example.com", 3).await;
    let subject = fixture.subject_of(&owner.actor).await;
    let mut config = config::from_lookup(|name| match name {
        config::SOCKET => Some(env.socket.to_str().unwrap().into()),
        config::PEER_UID => Some(env.uid.to_string()),
        config::PEER_POLICY => Some("direct_peer".into()),
        _ => None,
    })
    .unwrap()
    .unwrap();
    config.studio_subjects = vec![
        cantos_server::platform::identity_auth::decision::Subject::from_hex(&subject).unwrap(),
    ];
    let storage =
        Arc::new(FilesystemPublicationStorage::new(&fixture.dir().join("publication")).unwrap());
    let mut seed = WorkSeed::podcast(
        0x6540,
        "Ánh đèn cuối sân khấu — bản chia sẻ",
        &["Một tiếng chuông trong sương", "Lời hẹn trở về"],
    );
    seed.owner = owner.actor.clone();
    let work = fixture
        .seed_work_with_media(Some(&storage), &seed, Some(&bytes))
        .await;
    fixture.publish_work_metadata(&work).await;
    let control = "d654d654d654d654d654d654d654d654d654d654d654d654d654d654d654d654";
    fixture.studio_caller(&owner.actor, control).await;
    fixture
        .store()
        .viewer_access_execute(
            control,
            &work.work_id,
            ViewerAccessRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_version: 0,
                change: ViewerAccessChange::SetAudience {
                    audience: ViewerAudience::Restricted,
                    confirm_metadata_rights: false,
                },
            },
        )
        .await
        .unwrap();
    let store = fixture
        .store()
        .clone()
        .without_development_sessions()
        .with_identity_auth(IdentityAuth::new(config, true));
    let publication =
        PublicationService::new(store.clone()).with_origin_publication(storage.clone());
    let origin = OriginPlayback::new(
        PublicMediaOrigin::parse("https://localhost:6543").unwrap(),
        GrantKey::new(&[7; 32]).unwrap(),
        storage,
    );
    let scheme = std::env::var("CANTOS_WEB_SHARING_SCHEME").unwrap_or_else(|_| "https".into());
    assert!(matches!(scheme.as_str(), "http" | "https"));
    let studio_origin = format!("{scheme}://localhost:6541");
    let theatre_origin = format!("{scheme}://localhost:6542");
    let origins = SurfaceOrigins::new(&studio_origin, &theatre_origin, None).unwrap();
    let router = router_with_restricted_media(
        AppState {
            store,
            web_origin: theatre_origin.clone(),
        },
        &origins,
        publication,
        origin,
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:6540")
        .await
        .unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    std::fs::write(root.join("ready.json"),serde_json::to_vec_pretty(&json!({"work":work.work_id,"reader_actor":reader.actor,"owner":OWNER,"reader":SECOND,"stranger":"stranger@example.com","publication":work.units[0].chain.release_id,"units":work.units.iter().map(|u|&u.unit_id).collect::<Vec<_>>(),"bytes":bytes.len(),"api":format!("{scheme}://localhost:6543"),"studio":studio_origin,"theatre":theatre_origin,"media_api":"https://localhost:6543","boundary":"test-owned rows, real Identity/cookies/HTTP/bytes; no production activation"})).unwrap()).unwrap();
    tokio::time::timeout(Duration::from_secs(1800), async {
        while !root.join("stop").exists() {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("browser campaign finished within its bound");
    assert_eq!(sink.messages().len(), 3, "sharing sends no invitation");
    server.abort();
}
