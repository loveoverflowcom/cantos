//! #333/#335 candidate journey: the **real** `cantos-identity` executable over its authenticated
//! Unix socket with its own file-backed `identity.db`, a run-owned loopback SMTP sink, the
//! Server's production Identity RPC client, checked facts and decisions (`IdentityAuth`), and the
//! uncomposed Turso Actor Registry and session adapters on a real `server.db` this process owns.
//!
//! Nothing seeds a session or fabricates an `AuthenticationFact`: every credential is earned by
//! signing up through the mailed link and logging in with the chosen password. This is not a
//! Server HTTP or E2E journey: no router, cookie transport or production Server binary runs, and
//! the full HTTP matrix lives in `identity_password_turso.rs`.
//!
//! The Identity executable is never built here; supply the prebuilt artifact:
//!
//! ```sh
//! CANTOS_TEST_IDENTITY_BINARY=/absolute/path/to/cantos-identity \
//!   cargo test -p cantos-server --test identity_auth_candidate
//! ```
#[path = "support/identity_process.rs"]
mod process;

use cantos_server::platform::{
    identity::{
        domain::{ActorId, ActorRecord, ExternalIdentityKey},
        IdentityRegistry, TursoIdentityRegistry,
    },
    identity_auth::{
        client::IdentityClient,
        config,
        credential::{Channel, Surface},
        decision::Subject,
        turso::TursoSessions,
        AuthFailure, IdentityAuth, SignupCommand,
    },
    turso::ServerDatabase,
};
use cantos_store_turso::{
    clock::{Clock, SystemClock},
    registry::Intent,
};
use process::{IdentityEnv, Instance, RunDir, SmtpSink};
use std::{net::IpAddr, sync::Arc};

const OWNER: &str = "owner@example.com";
const PASSWORD: &str = "a correct horse battery";
const THEATRE: Channel = Channel::Web(Surface::Theatre);
const STUDIO: Channel = Channel::Web(Surface::Studio);
const NATIVE: Channel = Channel::Native;

fn unhex<const N: usize>(text: &str) -> [u8; N] {
    assert_eq!(text.len(), N * 2, "{text}");
    let mut out = [0; N];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap();
    }
    out
}

/// The production configuration parser, fed what an operator would set.
fn auth(env: &IdentityEnv, studio: &[Subject]) -> IdentityAuth {
    let roster = serde_json::to_string(
        &studio
            .iter()
            .map(|subject| subject.hex())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let config = config::from_lookup(|name| match name {
        config::SOCKET => Some(env.socket.to_str().unwrap().to_owned()),
        config::PEER_UID => Some(env.uid.to_string()),
        config::PEER_POLICY => Some("direct_peer".to_owned()),
        config::STUDIO_SUBJECTS if !studio.is_empty() => Some(roster.clone()),
        _ => None,
    })
    .unwrap()
    .expect("the Identity inputs are complete");
    IdentityAuth::new(config, true)
}

async fn open(run: &RunDir, intent: Intent) -> (ServerDatabase, TursoSessions) {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let database = ServerDatabase::open(run.path("server.db"), intent, clock)
        .await
        .expect("the test process owns server.db");
    let sessions = TursoSessions::new(database.clone());
    (database, sessions)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_identity_actor_session_journey() {
    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::supplied(&run, sink.port);
    env.migrate().await;
    let mut identity: Option<Instance> = Some(env.serve().await);
    let (database, sessions) = open(&run, Intent::Migrate).await;
    let theatre = auth(&env, &[]);
    let client: IpAddr = "127.0.0.1".parse().unwrap();

    // Signup through the mailed link. Until the explicit completion, no password works.
    assert_eq!(theatre.start_signup(OWNER, client).await, Ok(900));
    let mail = sink.wait_for(1).await.remove(0);
    assert_eq!(mail.to, [OWNER]);
    let (base, link) = mail.link();
    assert_eq!(base, "https://theatre.example/signup");
    assert_eq!(
        theatre
            .login(&sessions, THEATRE, OWNER, PASSWORD, client)
            .await
            .err(),
        Some(AuthFailure::Unauthenticated)
    );
    let command = || SignupCommand {
        operation: unhex(&link.operation),
        proof: unhex(&link.proof),
        secret: unhex(&link.secret),
        password: PASSWORD.to_owned(),
    };
    assert_eq!(
        theatre.complete_signup(&sessions, command(), client).await,
        Ok(false)
    );
    // A lost completion response is retried as the identical command and binds nothing new.
    assert_eq!(
        theatre.complete_signup(&sessions, command(), client).await,
        Ok(true)
    );

    // Password authentication through Identity, then a session on the exact subject's actor.
    assert_eq!(
        theatre
            .login(&sessions, THEATRE, OWNER, "a wrong horse battery", client)
            .await
            .err(),
        Some(AuthFailure::Unauthenticated)
    );
    let web = theatre
        .login(&sessions, THEATRE, OWNER, PASSWORD, client)
        .await
        .expect("the mailbox owner logs in");
    let record = theatre
        .authenticate(&sessions, &web.secrets.access, THEATRE)
        .await
        .expect("a fresh Identity read admits the session");
    assert_eq!(record.actor_id, web.actor_id);
    assert_eq!(record.session_id, web.session_id);
    assert_eq!(record.authenticated_at, web.authenticated_at);
    let subject = record.observed.subject;
    // The fact's security snapshot is what Identity reports now, over the same private RPC.
    let state = IdentityClient::lazy(env.socket.clone(), env.uid)
        .security_state(subject)
        .await
        .expect("Identity answers for the subject");
    assert!(state.active);
    assert_eq!(
        (state.subject, state.version, state.epoch),
        (subject, record.observed.version, record.observed.epoch)
    );
    // The binding is the exact Identity subject, never an email.
    let registry = TursoIdentityRegistry::new(database.clone());
    let key = ExternalIdentityKey::new("cantos-identity", subject.hex()).unwrap();
    let bound = ActorRecord {
        id: ActorId::new(web.actor_id.clone()).unwrap(),
        active: true,
    };
    assert_eq!(registry.lookup(&key).await, Ok(Some(bound.clone())));
    assert_eq!(
        registry
            .lookup(&ExternalIdentityKey::new("cantos-identity", OWNER).unwrap())
            .await,
        Ok(None)
    );
    // A Web cookie never opens another channel.
    for channel in [STUDIO, NATIVE] {
        assert_eq!(
            theatre
                .authenticate(&sessions, &web.secrets.access, channel)
                .await
                .err(),
            Some(AuthFailure::Unauthenticated)
        );
    }

    // Web renewal keeps the same secret and the absolute deadline.
    let (renewed, lifetime) = theatre
        .renew_web(&sessions, &web.secrets.access, THEATRE)
        .await
        .expect("the cookie renews");
    assert_eq!(lifetime.absolute, web.lifetime.absolute);
    assert!(lifetime.idle >= web.lifetime.idle);
    assert_eq!(renewed.observed, record.observed);

    // Native: a second login of the same subject reuses its actor; rotation spends once.
    let native = theatre
        .login(&sessions, NATIVE, OWNER, PASSWORD, client)
        .await
        .expect("the native login succeeds");
    assert_eq!(native.actor_id, web.actor_id);
    let refresh = native.secrets.refresh.clone().expect("a native pair");
    let (actor, authenticated_at, rotated_lifetime, next) = theatre
        .refresh_native(&sessions, &refresh, &native.session_id)
        .await
        .expect("the refresh secret rotates");
    assert_eq!(actor, native.actor_id);
    assert_eq!(authenticated_at, native.authenticated_at);
    assert_eq!(rotated_lifetime.absolute, native.lifetime.absolute);
    assert_eq!(
        theatre
            .authenticate(&sessions, &native.secrets.access, NATIVE)
            .await
            .err(),
        Some(AuthFailure::Unauthenticated)
    );
    assert!(theatre
        .authenticate(&sessions, &next.access, NATIVE)
        .await
        .is_ok());
    // Replaying the spent refresh secret ends the whole family.
    assert_eq!(
        theatre
            .refresh_native(&sessions, &refresh, &native.session_id)
            .await
            .err(),
        Some(AuthFailure::Unauthenticated)
    );
    assert_eq!(
        theatre
            .authenticate(&sessions, &next.access, NATIVE)
            .await
            .err(),
        Some(AuthFailure::Unauthenticated)
    );

    // Studio needs the Server roster; with it, the same actor gets a Studio-only cookie.
    assert_eq!(
        theatre
            .login(&sessions, STUDIO, OWNER, PASSWORD, client)
            .await
            .err(),
        Some(AuthFailure::Forbidden)
    );
    let studio = auth(&env, &[subject]);
    let creator = studio
        .login(&sessions, STUDIO, OWNER, PASSWORD, client)
        .await
        .expect("a rostered subject enters Studio");
    assert_eq!(creator.actor_id, web.actor_id);
    assert!(studio
        .authenticate(&sessions, &creator.secrets.access, STUDIO)
        .await
        .is_ok());
    assert_eq!(
        theatre
            .authenticate(&sessions, &creator.secrets.access, STUDIO)
            .await
            .err(),
        Some(AuthFailure::Unauthenticated),
        "without the roster the Studio session is not admitted"
    );

    // Logout is local and durable; refresh cleanup ends another native family.
    theatre
        .logout(&sessions, &web.secrets.access, THEATRE)
        .await
        .expect("logout");
    assert_eq!(
        theatre
            .authenticate(&sessions, &web.secrets.access, THEATRE)
            .await
            .err(),
        Some(AuthFailure::Unauthenticated)
    );
    assert_eq!(
        theatre
            .session_id_of(&sessions, &web.secrets.access, THEATRE)
            .await,
        Ok(Some(web.session_id.clone()))
    );
    let device = theatre
        .login(&sessions, NATIVE, OWNER, PASSWORD, client)
        .await
        .expect("another device logs in");
    let device_refresh = device.secrets.refresh.clone().unwrap();
    theatre
        .revoke_refresh(&sessions, &device_refresh, &device.session_id)
        .await
        .expect("refresh cleanup");
    assert_eq!(
        theatre
            .refresh_native(&sessions, &device_refresh, &device.session_id)
            .await
            .err(),
        Some(AuthFailure::Unauthenticated)
    );

    // An Identity outage fails closed without revoking; the session resumes afterwards.
    identity.take().unwrap().terminate().await;
    assert_eq!(
        studio
            .authenticate(&sessions, &creator.secrets.access, STUDIO)
            .await
            .err(),
        Some(AuthFailure::Unavailable)
    );
    identity = Some(env.serve().await);
    assert!(studio
        .authenticate(&sessions, &creator.secrets.access, STUDIO)
        .await
        .is_ok());

    // Reopen server.db: every revoked credential stays refused and the live one stays live.
    drop(registry);
    drop(sessions);
    drop(database);
    let (database, sessions) = open(&run, Intent::Serve).await;
    for (token, channel) in [
        (web.secrets.access.as_str(), THEATRE),
        (native.secrets.access.as_str(), NATIVE),
        (next.access.as_str(), NATIVE),
        (device.secrets.access.as_str(), NATIVE),
    ] {
        assert_eq!(
            theatre.authenticate(&sessions, token, channel).await.err(),
            Some(AuthFailure::Unauthenticated)
        );
    }
    assert_eq!(
        theatre
            .refresh_native(&sessions, &refresh, &native.session_id)
            .await
            .err(),
        Some(AuthFailure::Unauthenticated)
    );
    assert_eq!(
        studio
            .authenticate(&sessions, &creator.secrets.access, STUDIO)
            .await
            .expect("the live Studio session survives reopen")
            .actor_id,
        web.actor_id
    );
    assert_eq!(
        TursoIdentityRegistry::new(database.clone())
            .lookup(&key)
            .await,
        Ok(Some(bound))
    );
    identity.take().unwrap().terminate().await;
}
