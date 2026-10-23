//! #335 journeys: the production router and a real file-backed server.db, talking to the **real**
//! `cantos-identity` executable over its private Unix socket with a real file-backed Turso
//! database, real Argon2id and a real SMTP session into a run-owned sink.
//!
//! Nothing here seeds a session or mocks a verifier: every credential is earned by signing up
//! through the mailed link and logging in with the chosen password. Mail never leaves the
//! machine. Run through the ordinary Rust gate; the existing Identity process fixture owns its build.
#[path = "support/identity_process.rs"]
mod process;
#[path = "support/identity_world.rs"]
mod world;

use axum::http::{Method, StatusCode};
use cantos_identity::{
    domain::ids::{OperationId, SubjectId},
    persistence::IdentityStore,
};
use cantos_server::platform::identity_auth::decision::Subject;
use cantos_store_turso::{clock::SystemClock, registry::Intent};
use process::{IdentityEnv, Instance, RunDir, SmtpSink};
use serde_json::json;
use std::sync::Arc;
use world::{App, Database, FixtureQueries, Reply};

const ORIGIN: &str = "https://theatre.example";
const STUDIO_ORIGIN: &str = "https://studio.example";
const PASSWORD: &str = "a correct horse battery";
const OWNER: &str = "owner@example.com";
const OTHER: &str = "other@example.com";
const THEATRE: &str = "/api/v1/theatre";
const STUDIO: &str = "/api/v1/studio";
const THEATRE_COOKIE: &str = "__Secure-cantos_id_theatre";
const STUDIO_COOKIE: &str = "__Secure-cantos_id_studio";

struct World {
    db: Database,
    app: App,
    run: RunDir,
    env: IdentityEnv,
    sink: SmtpSink,
    identity: Option<Instance>,
    origin: &'static str,
}

impl World {
    async fn start(origin: &'static str) -> Self {
        Self::start_with(origin, &[]).await
    }

    async fn start_with(origin: &'static str, studio: &[Subject]) -> Self {
        let db = Database::new().await;
        let run = RunDir::new();
        let sink = SmtpSink::start().await;
        let env = IdentityEnv::new(&run, sink.port);
        env.migrate().await;
        let identity = Some(env.serve().await);
        let app = App::new(&db, origin, env.socket.clone(), env.uid, studio);
        Self {
            db,
            app,
            run,
            env,
            sink,
            identity,
            origin,
        }
    }

    /// The same database and Identity, a Server restarted with this Studio roster.
    fn restart_server(&mut self, studio: &[Subject]) {
        self.app = App::new(
            &self.db,
            self.origin,
            self.env.socket.clone(),
            self.env.uid,
            studio,
        );
    }

    fn origin_header(&self) -> [(&'static str, &'static str); 1] {
        [("origin", self.origin)]
    }

    /// Signup through the mailed link, exactly as a mailbox owner would do it.
    async fn signup(&self, email: &str, password: &str) {
        let before = self.sink.messages().len();
        let started = self
            .app
            .post(
                &format!("{THEATRE}/identity/signup"),
                &self.origin_header(),
                json!({"email": email}),
            )
            .await;
        assert_eq!(started.status, StatusCode::ACCEPTED, "{:?}", started.json);
        assert_eq!(
            started.json,
            json!({"accepted": true, "proof_ttl_seconds": 900})
        );
        let mail = self.sink.wait_for(before + 1).await.remove(before);
        assert_eq!(mail.to, [email]);
        let (base, link) = mail.link();
        assert_eq!(base, "https://theatre.example/signup");
        let done = self
            .app
            .post(
                &format!("{THEATRE}/identity/signup/complete"),
                &self.origin_header(),
                json!({
                    "operation_id": link.operation, "proof_id": link.proof,
                    "proof_secret": link.secret, "password": password,
                    "explicit_confirmation": true,
                }),
            )
            .await;
        assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
        assert_eq!(done.json, json!({"status": "completed"}));
    }

    async fn login_web(&self, surface: &str, email: &str, password: &str) -> Reply {
        self.app
            .post(
                &format!("/api/v1/{surface}/identity/login"),
                &self.origin_header(),
                json!({"email": email, "password": password}),
            )
            .await
    }

    async fn login_native(&self, email: &str, password: &str) -> Reply {
        self.app
            .post(
                &format!("{THEATRE}/native/identity/login"),
                &[],
                json!({"email": email, "password": password}),
            )
            .await
    }

    async fn subject_of(&self, email_owner_session: &str) -> Subject {
        let row = self
            .db
            .admin
            .query_one(
                "SELECT subject FROM identity_sessions WHERE id=?1",
                &[&email_owner_session],
            )
            .await
            .unwrap();
        Subject::from_hex(&row.try_get::<_, String>(0).unwrap()).unwrap()
    }

    async fn session_revoked(&self, session: &str) -> Option<String> {
        self.db
            .admin
            .query_one(
                "SELECT revoked_reason FROM identity_sessions WHERE id=?1",
                &[&session],
            )
            .await
            .unwrap()
            .try_get(0)
            .unwrap()
    }
}

fn cookie_header(name: &str, value: &str) -> String {
    format!("{name}={value}")
}

async fn library(world: &World, headers: &[(&str, &str)]) -> Reply {
    world
        .app
        .get(&format!("{THEATRE}/library/continue"), headers)
        .await
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

// ---- Web cookie journey --------------------------------------------------------------------------

#[tokio::test]
async fn signup_through_the_mailed_link_then_password_login_gives_an_httponly_cookie_session() {
    let w = World::start(ORIGIN).await;

    // Starting a signup (or polling for it) grants nothing: there is no account to log in to.
    let before = w.login_web("theatre", OWNER, PASSWORD).await;
    assert_eq!(before.status, StatusCode::UNAUTHORIZED);
    w.signup(OWNER, PASSWORD).await;

    let login = w.login_web("theatre", OWNER, PASSWORD).await;
    assert_eq!(login.status, StatusCode::OK, "{:?}", login.json);
    // The credential reaches the browser only as an HttpOnly, Secure, host-only, Strict cookie
    // scoped to this surface's path; the body carries nothing a script could replay.
    let raw = login.set_cookies().remove(0);
    let (name, secret) = login.cookie();
    assert_eq!(name, THEATRE_COOKIE);
    assert!(secret.starts_with("ci1_t_") && secret.len() == 6 + 64);
    for attribute in [
        "HttpOnly",
        "Secure",
        "SameSite=Strict",
        "Path=/api/v1/theatre",
    ] {
        assert!(raw.contains(attribute), "{raw}");
    }
    assert!(!raw.contains("Domain"), "{raw}");
    let body = login.json.to_string();
    assert!(!body.contains(&secret) && !body.contains("ci1_"), "{body}");
    assert_eq!(login.headers["cache-control"], "no-store");
    let session = login.json["session_id"].as_str().unwrap().to_owned();
    assert_eq!(login.headers["x-cantos-theatre-session"], session.as_str());
    let actor = login.json["actor_id"].as_str().unwrap().to_owned();

    // The protected API admits the cookie; the session view reports the same facts.
    let cookie = cookie_header(&name, &secret);
    let account = library(&w, &[("cookie", &cookie)]).await;
    assert_eq!(account.status, StatusCode::OK, "{:?}", account.json);
    let view = w
        .app
        .get(
            &format!("{THEATRE}/identity/session"),
            &[("cookie", &cookie)],
        )
        .await;
    assert_eq!(view.status, StatusCode::OK);
    assert_eq!(view.json["actor_id"], json!(actor));
    assert_eq!(view.json["session_id"], json!(session));

    // The database holds only a digest, never the secret.
    let stored: Vec<u8> =
        w.db.admin
            .query_one("SELECT access_hash FROM identity_session_tokens", &[])
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
    assert_eq!(stored.len(), 32);
    assert!(!stored
        .windows(8)
        .any(|window| secret.as_bytes().starts_with(window)));

    // Logout is local and durable; the same cookie is dead and the cookie is cleared.
    let out = w
        .app
        .send(
            Method::DELETE,
            &format!("{THEATRE}/identity/session"),
            &[("cookie", &cookie), ("origin", ORIGIN)],
            None,
        )
        .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert!(out.set_cookies()[0].contains("Max-Age=0"));
    assert_eq!(
        library(&w, &[("cookie", &cookie)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(w.session_revoked(&session).await.as_deref(), Some("logout"));
}

#[tokio::test]
async fn wrong_credentials_and_unknown_accounts_get_one_generic_denial_without_a_session() {
    let w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let wrong = w.login_web("theatre", OWNER, "not the password").await;
    let unknown = w.login_web("theatre", "nobody@example.com", PASSWORD).await;
    for denial in [&wrong, &unknown] {
        assert_eq!(denial.status, StatusCode::UNAUTHORIZED);
        assert!(denial.set_cookies().is_empty());
    }
    assert_eq!(wrong.json, unknown.json);
    let count: i64 =
        w.db.admin
            .query_one("SELECT count(*) FROM identity_sessions", &[])
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn signup_completion_is_replay_safe_and_never_issues_a_session() {
    let w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let mail = &w.sink.messages()[0];
    let (_, link) = mail.link();
    let body = |password: &str| {
        json!({
            "operation_id": link.operation, "proof_id": link.proof,
            "proof_secret": link.secret, "password": password,
            "explicit_confirmation": true,
        })
    };
    let path = format!("{THEATRE}/identity/signup/complete");
    // A lost response is retried as the identical request: the original account, as history.
    let replay = w.app.post(&path, &w.origin_header(), body(PASSWORD)).await;
    assert_eq!(replay.status, StatusCode::OK);
    assert_eq!(replay.json, json!({"status": "replayed"}));
    assert!(replay.set_cookies().is_empty());
    // Another password, or a missing confirmation, is refused.
    let changed = w
        .app
        .post(&path, &w.origin_header(), body("someone else's choice"))
        .await;
    assert_eq!(changed.status, StatusCode::UNAUTHORIZED);
    let mut unconfirmed = body(PASSWORD);
    unconfirmed["explicit_confirmation"] = json!(false);
    let refused = w.app.post(&path, &w.origin_header(), unconfirmed).await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    // Exactly one account exists and one actor is bound to it.
    let bound: i64 =
        w.db.admin
            .query_one(
                "SELECT count(*) FROM actor_external_identities WHERE issuer='cantos-identity'",
                &[],
            )
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
    assert_eq!(bound, 1);
}

#[tokio::test]
async fn web_and_native_credentials_of_one_account_resolve_to_the_same_actor() {
    let w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let web = w.login_web("theatre", OWNER, PASSWORD).await;
    let native = w.login_native(OWNER, PASSWORD).await;
    assert_eq!(web.status, StatusCode::OK);
    assert_eq!(native.status, StatusCode::OK, "{:?}", native.json);
    assert_eq!(web.field("actor_id"), native.field("actor_id"));
    assert_ne!(web.json["session_id"], native.json["session_id"]);
    // A second account is a different actor; nothing links by email.
    w.signup(OTHER, "another long password").await;
    let other = w.login_native(OTHER, "another long password").await;
    assert_ne!(other.json["actor_id"], native.json["actor_id"]);
}

// ---- Native Bearer journey -----------------------------------------------------------------------

#[tokio::test]
async fn native_bearer_session_renews_rotates_and_a_spent_refresh_revokes_the_family() {
    let w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let login = w.login_native(OWNER, PASSWORD).await;
    assert_eq!(login.status, StatusCode::OK, "{:?}", login.json);
    assert!(
        login.set_cookies().is_empty(),
        "native never receives a cookie"
    );
    assert_eq!(login.json["token_type"], "Bearer");
    let access = login.json["access_token"].as_str().unwrap().to_owned();
    let refresh = login.json["refresh_token"].as_str().unwrap().to_owned();
    let session = login.json["session_id"].as_str().unwrap().to_owned();
    assert!(access.starts_with("ci1_n_") && refresh.starts_with("ci1_r_"));
    // The reused Theatre policy: 120 s access, 600 s idle, 900 s absolute.
    let issued = |key: &str| login.json[key].as_u64().unwrap();
    let access_window = issued("expires_at") - issued("authenticated_at");
    assert!((120..=125).contains(&access_window), "{access_window}");
    assert!(issued("refresh_expires_at") > issued("expires_at"));
    assert!(issued("absolute_expires_at") >= issued("refresh_expires_at"));

    let auth = bearer(&access);
    assert_eq!(
        library(&w, &[("authorization", &auth)]).await.status,
        StatusCode::OK
    );
    let view = w
        .app
        .get(
            &format!("{THEATRE}/native/identity/session"),
            &[("authorization", &auth)],
        )
        .await;
    assert_eq!(view.status, StatusCode::OK);
    assert_eq!(view.json["session_id"], json!(session));

    // Renewal: a new pair for the same session and actor; the old access stops admitting.
    let renewed = w
        .app
        .post(
            &format!("{THEATRE}/native/identity/refresh"),
            &[],
            json!({"refresh_token": refresh, "session_id": session}),
        )
        .await;
    assert_eq!(renewed.status, StatusCode::OK, "{:?}", renewed.json);
    assert_eq!(renewed.field("session_id"), login.field("session_id"));
    assert_eq!(renewed.field("actor_id"), login.field("actor_id"));
    assert_eq!(
        renewed.field("absolute_expires_at"),
        login.field("absolute_expires_at")
    );
    assert_eq!(
        renewed.field("authenticated_at"),
        login.field("authenticated_at")
    );
    let next_access = renewed.json["access_token"].as_str().unwrap().to_owned();
    assert_ne!(next_access, access);
    assert_eq!(
        library(&w, &[("authorization", &auth)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    let next_auth = bearer(&next_access);
    assert_eq!(
        library(&w, &[("authorization", &next_auth)]).await.status,
        StatusCode::OK
    );

    // Replaying the spent refresh secret revokes the whole family, including the winner.
    let replay = w
        .app
        .post(
            &format!("{THEATRE}/native/identity/refresh"),
            &[],
            json!({"refresh_token": refresh, "session_id": session}),
        )
        .await;
    assert_eq!(replay.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        library(&w, &[("authorization", &next_auth)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(w.session_revoked(&session).await.as_deref(), Some("replay"));
    // Cleanup by a spent secret stays possible and idempotent.
    let revoke = w
        .app
        .post(
            &format!("{THEATRE}/native/identity/revoke"),
            &[],
            json!({"refresh_token": refresh, "session_id": session}),
        )
        .await;
    assert_eq!(revoke.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn a_native_bearer_write_needs_no_origin_and_is_admitted_as_the_account() {
    let w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let native = w.login_native(OWNER, PASSWORD).await;
    let auth = bearer(native.json["access_token"].as_str().unwrap());
    let publication = "00000000-0000-4000-8000-000000000001";
    let path = format!("{THEATRE}/progress/{publication}");
    // The unknown publication is a domain answer ("not found"), proving the write reached the
    // Library as the authenticated listener instead of stopping at admission.
    let write = w
        .app
        .send(
            Method::PUT,
            &path,
            &[("authorization", &auth)],
            Some(json!({"unrelated": true})),
        )
        .await;
    assert_ne!(write.status, StatusCode::UNAUTHORIZED, "{:?}", write.json);
    assert_ne!(write.status, StatusCode::FORBIDDEN, "{:?}", write.json);
    assert_ne!(
        write.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{:?}",
        write.json
    );
    // Without any credential the same write is refused at admission.
    let anonymous = w
        .app
        .send(Method::PUT, &path, &[], Some(json!({"unrelated": true})))
        .await;
    assert!(
        [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN].contains(&anonymous.status),
        "{:?}",
        anonymous.status
    );
}

#[tokio::test]
async fn native_logout_is_durable_and_a_wrong_kind_of_credential_is_refused_everywhere() {
    let w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let web = w.login_web("theatre", OWNER, PASSWORD).await;
    let (cookie_name, cookie_secret) = web.cookie();
    let cookie = cookie_header(&cookie_name, &cookie_secret);
    let native = w.login_native(OWNER, PASSWORD).await;
    let access = native.json["access_token"].as_str().unwrap().to_owned();
    let refresh = native.json["refresh_token"].as_str().unwrap().to_owned();
    let auth = bearer(&access);

    // Mixed credentials are refused outright, never resolved by preferring one.
    let mixed = library(&w, &[("cookie", &cookie), ("authorization", &auth)]).await;
    assert_eq!(mixed.status, StatusCode::UNAUTHORIZED);
    // A cookie secret is not a Bearer, and a Bearer or refresh secret is not a cookie.
    let cookie_as_bearer = bearer(&cookie_secret);
    let refresh_as_bearer = bearer(&refresh);
    let access_as_cookie = cookie_header(THEATRE_COOKIE, &access);
    let refresh_as_cookie = cookie_header(THEATRE_COOKIE, &refresh);
    for (header, value, label) in [
        (
            "authorization",
            &cookie_as_bearer,
            "cookie secret as Bearer",
        ),
        ("authorization", &refresh_as_bearer, "refresh as Bearer"),
        ("cookie", &access_as_cookie, "Bearer as cookie"),
        ("cookie", &refresh_as_cookie, "refresh as cookie"),
    ] {
        assert_eq!(
            library(&w, &[(header, value.as_str())]).await.status,
            StatusCode::UNAUTHORIZED,
            "{label}"
        );
    }
    // The web session endpoint never accepts a Bearer; the native one never a cookie.
    let on_web = w
        .app
        .get(
            &format!("{THEATRE}/identity/session"),
            &[("authorization", &auth)],
        )
        .await;
    assert_eq!(on_web.status, StatusCode::UNAUTHORIZED);
    let on_native = w
        .app
        .get(
            &format!("{THEATRE}/native/identity/session"),
            &[("cookie", &cookie)],
        )
        .await;
    assert_eq!(on_native.status, StatusCode::UNAUTHORIZED);
    let with_origin = w
        .app
        .get(
            &format!("{THEATRE}/native/identity/session"),
            &[("authorization", &auth), ("origin", ORIGIN)],
        )
        .await;
    assert_eq!(with_origin.status, StatusCode::UNAUTHORIZED);

    let out = w
        .app
        .send(
            Method::DELETE,
            &format!("{THEATRE}/native/identity/session"),
            &[("authorization", &auth)],
            None,
        )
        .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert_eq!(
        library(&w, &[("authorization", &auth)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    // The web session of the same account is a different family and is untouched.
    assert_eq!(
        library(&w, &[("cookie", &cookie)]).await.status,
        StatusCode::OK
    );
}

// ---- CSRF, Origin and surface isolation ---------------------------------------------------------

#[tokio::test]
async fn cookie_mutations_need_the_exact_origin_and_browser_signals_never_reach_native_routes() {
    let w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let body = json!({"email": OWNER, "password": PASSWORD});
    let login = format!("{THEATRE}/identity/login");
    for origin in [
        None,
        Some("https://evil.example"),
        Some("https://sibling.theatre.example"),
        Some("http://theatre.example"),
        Some("https://theatre.example:444"),
    ] {
        let headers: Vec<(&str, &str)> = origin.map(|o| ("origin", o)).into_iter().collect();
        let reply = w.app.post(&login, &headers, body.clone()).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{origin:?}");
        assert!(reply.set_cookies().is_empty());
    }
    let ok = w.app.post(&login, &w.origin_header(), body.clone()).await;
    assert_eq!(ok.status, StatusCode::OK);
    let (name, secret) = ok.cookie();
    let cookie = cookie_header(&name, &secret);
    // Renew and logout with a cookie are CSRF-gated like login.
    for path in ["identity/session/renew"] {
        let reply = w
            .app
            .post(
                &format!("{THEATRE}/{path}"),
                &[("cookie", &cookie)],
                json!(null),
            )
            .await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{path} without Origin");
    }
    let forged_logout = w
        .app
        .send(
            Method::DELETE,
            &format!("{THEATRE}/identity/session"),
            &[("cookie", &cookie), ("origin", "https://evil.example")],
            None,
        )
        .await;
    assert_eq!(forged_logout.status, StatusCode::FORBIDDEN);
    assert_eq!(
        library(&w, &[("cookie", &cookie)]).await.status,
        StatusCode::OK
    );
    // A cookie-authenticated Library mutation without Origin is still a CSRF candidate: the
    // Bearer-only exemption must not extend to a cookie request.
    let forged_write = w
        .app
        .send(
            Method::PUT,
            &format!("{THEATRE}/progress/00000000-0000-4000-8000-000000000001"),
            &[("cookie", &cookie)],
            Some(json!({})),
        )
        .await;
    assert_eq!(
        forged_write.status,
        StatusCode::UNAUTHORIZED,
        "{:?}",
        forged_write.json
    );
    // Native routes refuse every browser signal, including a perfectly valid Origin.
    for headers in [
        vec![("origin", ORIGIN)],
        vec![("cookie", cookie.as_str())],
        vec![("sec-fetch-site", "cross-site")],
        vec![("sec-fetch-mode", "cors")],
    ] {
        let reply = w
            .app
            .post(
                &format!("{THEATRE}/native/identity/login"),
                &headers,
                body.clone(),
            )
            .await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{headers:?}");
    }
    // Every reply is no-store, and unknown fields are refused (strict DTOs).
    let strict = w
        .app
        .post(
            &login,
            &w.origin_header(),
            json!({"email": OWNER, "password": PASSWORD, "x": 1}),
        )
        .await;
    assert_eq!(strict.status, StatusCode::BAD_REQUEST);
    assert_eq!(strict.headers["cache-control"], "no-store");
}

#[tokio::test]
async fn a_cookie_of_one_surface_is_never_authority_on_another_or_with_provider_cookies() {
    let w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let theatre = w.login_web("theatre", OWNER, PASSWORD).await;
    let (_, theatre_secret) = theatre.cookie();
    // A Theatre secret presented under the Studio cookie name, on a Studio route.
    let forged = cookie_header(STUDIO_COOKIE, &theatre_secret);
    let studio_head = format!("{STUDIO}/scripts/00000000-0000-4000-8000-000000000001/head");
    assert_eq!(
        w.app.get(&studio_head, &[("cookie", &forged)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    // And under the Theatre name with another surface's tag.
    assert_eq!(
        w.app
            .get(
                &format!("{THEATRE}/identity/session"),
                &[(
                    "cookie",
                    &cookie_header(THEATRE_COOKIE, &theatre_secret.replace("ci1_t_", "ci1_s_"))
                )],
            )
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    // The plain (non-Secure) spelling is a different cookie and is refused on an https host.
    assert_eq!(
        w.app
            .get(
                &format!("{THEATRE}/identity/session"),
                &[(
                    "cookie",
                    &cookie_header("cantos_id_theatre", &theatre_secret)
                )],
            )
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    // Two cookies of the same name are ambiguous and refused.
    let doubled = format!("{THEATRE_COOKIE}={theatre_secret}; {THEATRE_COOKIE}={theatre_secret}");
    assert_eq!(
        w.app
            .get(
                &format!("{THEATRE}/identity/session"),
                &[("cookie", &doubled)]
            )
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    // A provider-bound Theatre cookie alongside ours is a mixed request.
    let mixed = format!(
        "{THEATRE_COOKIE}={theatre_secret}; __Secure-cantos_theatre={}",
        "0".repeat(64)
    );
    assert_eq!(
        library(&w, &[("cookie", &mixed)]).await.status,
        StatusCode::UNAUTHORIZED
    );
}

// ---- account switch and late responses ----------------------------------------------------------

#[tokio::test]
async fn a_late_request_from_the_previous_account_is_fenced_and_a_new_login_retires_the_old_cookie()
{
    let w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    w.signup(OTHER, "another long password").await;
    let a = w.login_web("theatre", OWNER, PASSWORD).await;
    let (name, secret_a) = a.cookie();
    let session_a = a.json["session_id"].as_str().unwrap().to_owned();
    let cookie_a = cookie_header(&name, &secret_a);

    // The same browser logs in as B: A's cookie is replaced, and A's family is retired.
    let b = w
        .app
        .post(
            &format!("{THEATRE}/identity/login"),
            &[("origin", ORIGIN), ("cookie", &cookie_a)],
            json!({"email": OTHER, "password": "another long password"}),
        )
        .await;
    assert_eq!(b.status, StatusCode::OK, "{:?}", b.json);
    let (_, secret_b) = b.cookie();
    let session_b = b.json["session_id"].as_str().unwrap().to_owned();
    assert_ne!(b.json["actor_id"], a.json["actor_id"]);
    assert_eq!(
        library(&w, &[("cookie", &cookie_a)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        w.session_revoked(&session_a).await.as_deref(),
        Some("logout")
    );

    // A late A request that reaches the server with B's cookie declares A's session: refused with
    // B's id, never dispatched as B.
    let cookie_b = cookie_header(&name, &secret_b);
    let late = library(
        &w,
        &[
            ("cookie", &cookie_b),
            ("x-cantos-theatre-session", &session_a),
        ],
    )
    .await;
    assert_eq!(late.status, StatusCode::CONFLICT);
    assert_eq!(late.headers["x-cantos-theatre-session"], session_b.as_str());
    // A late A logout cannot end B's session either.
    let late_logout = w
        .app
        .send(
            Method::DELETE,
            &format!("{THEATRE}/identity/session"),
            &[
                ("cookie", &cookie_b),
                ("origin", ORIGIN),
                ("x-cantos-theatre-session", &session_a),
            ],
            None,
        )
        .await;
    assert_eq!(late_logout.status, StatusCode::CONFLICT);
    assert!(w.session_revoked(&session_b).await.is_none());
    // The honest B request is admitted.
    assert_eq!(
        library(
            &w,
            &[
                ("cookie", &cookie_b),
                ("x-cantos-theatre-session", &session_b)
            ]
        )
        .await
        .status,
        StatusCode::OK
    );
}

// ---- Studio roster --------------------------------------------------------------------------------

#[tokio::test]
async fn studio_login_needs_the_server_roster_and_a_theatre_session_never_enters_studio() {
    let mut w = World::start(STUDIO_ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    w.signup(OTHER, "another long password").await;
    // With no roster configured Studio login does not exist at all.
    let none = w.login_web("studio", OWNER, PASSWORD).await;
    assert_eq!(none.status, StatusCode::NOT_FOUND);
    // The roster is learned from a real account: the owner's Identity subject.
    let theatre = w.login_web("theatre", OWNER, PASSWORD).await;
    let owner_subject = w
        .subject_of(theatre.json["session_id"].as_str().unwrap())
        .await;
    w.restart_server(&[owner_subject]);

    // Password verified but not on the roster: forbidden, and no session of any kind is cut.
    let denied = w.login_web("studio", OTHER, "another long password").await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert!(denied.set_cookies().is_empty());
    let studio_sessions: i64 =
        w.db.admin
            .query_one(
                "SELECT count(*) FROM identity_sessions WHERE channel='studio-web'",
                &[],
            )
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
    assert_eq!(studio_sessions, 0);
    // A wrong password for a roster member is the ordinary generic denial.
    assert_eq!(
        w.login_web("studio", OWNER, "wrong password").await.status,
        StatusCode::UNAUTHORIZED
    );

    let allowed = w.login_web("studio", OWNER, PASSWORD).await;
    assert_eq!(allowed.status, StatusCode::OK, "{:?}", allowed.json);
    let (name, secret) = allowed.cookie();
    assert_eq!(name, STUDIO_COOKIE);
    assert!(secret.starts_with("ci1_s_"));
    assert!(allowed.set_cookies()[0].contains("Path=/api/v1/studio"));
    // Same Identity subject, same actor as the Theatre session: one account, one ActorId.
    assert_eq!(allowed.json["actor_id"], theatre.json["actor_id"]);
    let cookie = cookie_header(&name, &secret);
    // The Studio data plane admits it (the script does not exist, which is a different answer
    // from "unauthenticated").
    let head = format!("{STUDIO}/scripts/00000000-0000-4000-8000-000000000001/head");
    let reply = w.app.get(&head, &[("cookie", &cookie)]).await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND, "{:?}", reply.json);
    assert_eq!(w.app.get(&head, &[]).await.status, StatusCode::UNAUTHORIZED);
    // The Studio session fence is the session id and is stamped on a Studio reply.
    assert_eq!(
        reply.headers["x-cantos-studio-session"],
        allowed.json["session_id"].as_str().unwrap()
    );
    // Login granted no Studio resource: an arbitrary script still answers not-found/forbidden
    // rather than exposing anything, and Theatre routes refuse the Studio cookie.
    assert_eq!(
        w.app
            .get(
                &format!("{THEATRE}/identity/session"),
                &[("cookie", &cookie)]
            )
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    // A removed roster entry closes Studio for an already issued session at the next request.
    w.restart_server(&[Subject::from_bytes([7; 16]).unwrap()]);
    assert_eq!(
        w.app.get(&head, &[("cookie", &cookie)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    // Wrong purpose at the wire: a Theatre credential can never be exchanged for a Studio one.
    let theatre_secret = theatre.cookie().1;
    assert_eq!(
        w.app
            .get(
                &head,
                &[("cookie", &cookie_header(STUDIO_COOKIE, &theatre_secret))]
            )
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

// ---- expiry, renewal, outage and security changes --------------------------------------------------

#[tokio::test]
async fn web_renewal_extends_idle_but_never_the_absolute_deadline_and_expired_secrets_die() {
    let w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let login = w.login_web("theatre", OWNER, PASSWORD).await;
    let (name, secret) = login.cookie();
    let cookie = cookie_header(&name, &secret);
    let absolute = login.json["absolute_expires_at"].as_u64().unwrap();
    let first_idle = login.json["expires_at"].as_u64().unwrap();
    assert!(first_idle <= absolute);
    // Age the idle deadline by hand: the renewal moves it forward again, bounded by absolute.
    let guards = w.db.admin.rows(
        "SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND name IN ('identity_sessions_binding_immutable','identity_session_tokens_immutable') ORDER BY name", &[]
    ).await.unwrap();
    for row in &guards {
        let name: String = row.try_get(0).unwrap();
        w.db.admin
            .exec(&format!("DROP TRIGGER {name}"), &[])
            .await
            .unwrap();
    }
    w.db.admin
        .exec(
            "UPDATE identity_session_tokens SET expires_at=expires_at-300",
            &[],
        )
        .await
        .unwrap();
    w.db.admin
        .exec(
            "UPDATE identity_sessions SET idle_expires=idle_expires-300",
            &[],
        )
        .await
        .unwrap();
    for row in &guards {
        let ddl: String = row.try_get(1).unwrap();
        w.db.admin.exec(&ddl, &[]).await.unwrap();
    }
    let renewed = w
        .app
        .post(
            &format!("{THEATRE}/identity/session/renew"),
            &[("cookie", &cookie), ("origin", ORIGIN)],
            json!(null),
        )
        .await;
    assert_eq!(renewed.status, StatusCode::OK, "{:?}", renewed.json);
    assert!(renewed.json["expires_at"].as_u64().unwrap() > first_idle - 300);
    assert!(renewed.json["expires_at"].as_u64().unwrap() <= absolute);
    assert_eq!(
        renewed.json["absolute_expires_at"].as_u64().unwrap(),
        absolute
    );
    // Renewal keeps the same cookie value (no rotation to race between tabs), refreshed Max-Age.
    assert_eq!(renewed.cookie(), (name.clone(), secret.clone()));
    // An expired secret is dead even though nothing revoked it.
    let token_guard: String = guards
        .iter()
        .find(|row| row.try_get::<_, String>(0).unwrap() == "identity_session_tokens_immutable")
        .unwrap()
        .try_get(1)
        .unwrap();
    w.db.admin
        .exec("DROP TRIGGER identity_session_tokens_immutable", &[])
        .await
        .unwrap();
    w.db.admin
        .exec("UPDATE identity_session_tokens SET expires_at=1", &[])
        .await
        .unwrap();
    w.db.admin.exec(&token_guard, &[]).await.unwrap();
    assert_eq!(
        library(&w, &[("cookie", &cookie)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    let late_renewal = w
        .app
        .post(
            &format!("{THEATRE}/identity/session/renew"),
            &[("cookie", &cookie), ("origin", ORIGIN)],
            json!(null),
        )
        .await;
    assert_eq!(late_renewal.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn identity_outage_fails_protected_requests_closed_but_logout_stays_local_and_recovery_resumes(
) {
    let mut w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let web = w.login_web("theatre", OWNER, PASSWORD).await;
    let (name, secret) = web.cookie();
    let cookie = cookie_header(&name, &secret);
    let second = w.login_native(OWNER, PASSWORD).await;
    let auth = bearer(second.json["access_token"].as_str().unwrap());
    assert_eq!(
        library(&w, &[("cookie", &cookie)]).await.status,
        StatusCode::OK
    );

    w.identity.take().unwrap().terminate().await;
    // Nothing is admitted on stale knowledge, and nothing is revoked by an outage alone.
    for headers in [
        vec![("cookie", cookie.as_str())],
        vec![("authorization", auth.as_str())],
    ] {
        assert_eq!(
            library(&w, &headers).await.status,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    let login = w.login_web("theatre", OWNER, PASSWORD).await;
    assert_eq!(login.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(login.set_cookies().is_empty());
    // Public reads need no Identity; logout is a local write.
    let out = w
        .app
        .send(
            Method::DELETE,
            &format!("{THEATRE}/native/identity/session"),
            &[("authorization", &auth)],
            None,
        )
        .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);

    // Identity returns on the same database: same epoch, so the surviving session resumes.
    w.identity = Some(w.env.serve().await);
    assert_eq!(
        library(&w, &[("cookie", &cookie)]).await.status,
        StatusCode::OK
    );
    assert_eq!(
        library(&w, &[("authorization", &auth)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert!(w
        .session_revoked(second.json["session_id"].as_str().unwrap())
        .await
        .is_some());
    assert!(w
        .session_revoked(web.json["session_id"].as_str().unwrap())
        .await
        .is_none());
}

#[tokio::test]
async fn a_subject_disabled_in_identity_loses_every_session_and_is_never_resurrected() {
    let mut w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let web = w.login_web("theatre", OWNER, PASSWORD).await;
    let (name, secret) = web.cookie();
    let cookie = cookie_header(&name, &secret);
    let native = w.login_native(OWNER, PASSWORD).await;
    let auth = bearer(native.json["access_token"].as_str().unwrap());
    let web_session = web.json["session_id"].as_str().unwrap().to_owned();
    let subject = w.subject_of(&web_session).await;
    assert_eq!(
        library(&w, &[("cookie", &cookie)]).await.status,
        StatusCode::OK
    );

    // Disable through Identity's owning library while its process is stopped (one owner per file).
    w.identity.take().unwrap().terminate().await;
    let store = IdentityStore::open(&w.env.database, Intent::Serve, Arc::new(SystemClock))
        .await
        .unwrap();
    let id = SubjectId::from_bytes(subject.bytes()).unwrap();
    let current = store.subject(id).await.unwrap().unwrap();
    store
        .disable_subject(
            OperationId::from_bytes([9; 16]).unwrap(),
            id,
            current.version,
            store.epoch(),
        )
        .await
        .unwrap();
    drop(store);
    w.identity = Some(w.env.serve().await);

    // The very next admission reads the new state: both families die, durably.
    assert_eq!(
        library(&w, &[("cookie", &cookie)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        library(&w, &[("authorization", &auth)]).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        w.session_revoked(&web_session).await.as_deref(),
        Some("disabled")
    );
    // A login for the disabled account is the same generic denial and cuts no session.
    let relogin = w.login_web("theatre", OWNER, PASSWORD).await;
    assert_eq!(relogin.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_http_development_origin_uses_plain_cookie_names_without_secure() {
    let w = World::start("http://127.0.0.1:8080").await;
    // Signup budgets are per run; use the dev origin for the whole journey.
    let before = w.sink.messages().len();
    let started = w
        .app
        .post(
            &format!("{THEATRE}/identity/signup"),
            &[("origin", "http://127.0.0.1:8080")],
            json!({"email": OWNER}),
        )
        .await;
    assert_eq!(started.status, StatusCode::ACCEPTED);
    let (_, link) = w.sink.wait_for(before + 1).await.remove(before).link();
    let done = w
        .app
        .post(
            &format!("{THEATRE}/identity/signup/complete"),
            &[("origin", "http://127.0.0.1:8080")],
            json!({"operation_id": link.operation, "proof_id": link.proof,
                   "proof_secret": link.secret, "password": PASSWORD, "explicit_confirmation": true}),
        )
        .await;
    assert_eq!(done.status, StatusCode::OK);
    let login = w
        .app
        .post(
            &format!("{THEATRE}/identity/login"),
            &[("origin", "http://127.0.0.1:8080")],
            json!({"email": OWNER, "password": PASSWORD}),
        )
        .await;
    assert_eq!(login.status, StatusCode::OK);
    let raw = login.set_cookies().remove(0);
    assert!(
        raw.starts_with("cantos_id_theatre=") && !raw.contains("Secure"),
        "{raw}"
    );
    assert!(
        raw.contains("HttpOnly") && raw.contains("SameSite=Strict"),
        "{raw}"
    );
    let _ = &w.run;
}
