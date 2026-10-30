//! Real Identity executable + independent owned files + production Server HTTP. SMTP is a
//! run-owned loopback sink. These tests never send an invitation or alter production data.
use super::*;
use cantos_server::test_support::WorkSeed;
use uuid::Uuid;
fn op() -> String {
    Uuid::new_v4().to_string()
}
#[tokio::test]
async fn viewer_pending_before_signup_claims_exact_actor_and_replay_cannot_resurrect_after_revoke_restart(
) {
    let mut w = World::start(ORIGIN).await;
    w.signup(OWNER, PASSWORD).await;
    let owner = w.login_web("theatre", OWNER, PASSWORD).await;
    let subject = w
        .subject_of(owner.json["session_id"].as_str().unwrap())
        .await;
    w.restart_server(&[subject]);
    let studio = w.login_web("studio", OWNER, PASSWORD).await;
    assert_eq!(studio.status, StatusCode::OK);
    let (name, secret) = studio.cookie();
    let cookie = cookie_header(&name, &secret);
    let mut seed = WorkSeed::podcast(0x6520, "Private shared fixture", &[]);
    seed.owner = studio.json["actor_id"].as_str().unwrap().to_owned();
    let work = w.db.admin.seed_work(None, &seed).await;
    let path = format!("{STUDIO}/works/{}/viewer-access", work.work_id);
    let share = op();
    let before_mail = w.sink.messages().len();
    let pending=w.app.post(&path,&[("origin",ORIGIN),("cookie",&cookie)],json!({"operation_id":op(),"expected_version":0,"change":{"action":"share_mailbox","share_id":share,"email":"A+tag@EXAMPLE.COM"}})).await;
    assert_eq!(pending.status, StatusCode::OK, "{:?}", pending.json);
    assert_eq!(w.sink.messages().len(), before_mail);
    let state = w.app.get(&path, &[("cookie", &cookie)]).await;
    assert_eq!(state.json["pending"][0]["email"], "A+tag@example.com");
    assert_eq!(state.json["pending"].as_array().unwrap().len(), 1);
    let claim_path = format!("{THEATRE}/shared/claim");
    assert_eq!(
        w.app
            .post(&claim_path, &[], json!({"operation_id":op()}))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    w.signup("A+tag@example.com", PASSWORD).await;
    let native = w.login_native("A+tag@example.com", PASSWORD).await;
    assert_eq!(native.status, StatusCode::OK);
    let auth = bearer(native.json["access_token"].as_str().unwrap());
    assert_eq!(
        w.app
            .post(
                &claim_path,
                &[("authorization", &auth)],
                json!({"operation_id":op(),"email":"forged@example.com"})
            )
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    let request = json!({"operation_id":op()});
    let claimed = w
        .app
        .post(&claim_path, &[("authorization", &auth)], request.clone())
        .await;
    assert_eq!(claimed.status, StatusCode::OK, "{:?}", claimed.json);
    assert_eq!(
        claimed.json["grants"],
        json!([{"work_id":work.work_id,"grant_id":share}])
    );
    let actor = native.json["actor_id"].as_str().unwrap();
    let row =
        w.db.admin
            .query_one(
                "SELECT actor_id FROM catalog_viewer_claims WHERE share_id=?1",
                &[&share],
            )
            .await
            .unwrap();
    assert_eq!(row.try_get::<_, String>(0).unwrap(), actor);
    assert_eq!(
        w.app.get(&path, &[("cookie", &cookie)]).await.json["pending"],
        json!([])
    );
    let revoke=w.app.post(&path,&[("origin",ORIGIN),("cookie",&cookie)],json!({"operation_id":op(),"expected_version":2,"change":{"action":"revoke_grant","grant_id":share}})).await;
    assert_eq!(revoke.status, StatusCode::OK, "{:?}", revoke.json);
    w.app.release();
    w.db.admin.restart().await;
    w.restart_server(&[subject]);
    let replay = w
        .app
        .post(&claim_path, &[("authorization", &auth)], request)
        .await;
    assert_eq!(replay.status, StatusCode::OK);
    assert_eq!(replay.json["replayed"], true);
    assert_eq!(replay.json["grants"], claimed.json["grants"]);
    assert_eq!(
        w.db.admin
            .count("SELECT count(*) FROM catalog_viewer_grants", &[])
            .await,
        1
    );
    assert_eq!(
        w.db.admin
            .count("SELECT count(*) FROM catalog_viewer_grant_revocations", &[])
            .await,
        1
    );
    // Case-preserved local parts are distinct. A typed email field cannot broaden a proof.
    let lower=w.app.post(&path,&[("origin",ORIGIN),("cookie",&cookie)],json!({"operation_id":op(),"expected_version":3,"change":{"action":"share_mailbox","share_id":op(),"email":"a+tag@example.com"}})).await;
    assert_eq!(lower.status, StatusCode::OK);
    let mismatch = w
        .app
        .post(
            &claim_path,
            &[("authorization", &auth)],
            json!({"operation_id":op()}),
        )
        .await;
    assert_eq!(mismatch.status, StatusCode::OK);
    assert_eq!(mismatch.json["grants"], json!([]));
    // Cookie claims retain exact Origin and session-fence checks.
    let web = w.login_web("theatre", "A+tag@example.com", PASSWORD).await;
    let (name, secret) = web.cookie();
    let theatre_cookie = cookie_header(&name, &secret);
    assert_eq!(
        w.app
            .post(
                &claim_path,
                &[("cookie", &theatre_cookie)],
                json!({"operation_id":op()})
            )
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        w.app
            .post(
                &claim_path,
                &[
                    ("cookie", &theatre_cookie),
                    ("origin", ORIGIN),
                    ("x-cantos-theatre-session", "different")
                ],
                json!({"operation_id":op()})
            )
            .await
            .status,
        StatusCode::CONFLICT
    );
    w.identity.take().unwrap().terminate().await;
    assert_eq!(
        w.app
            .post(
                &claim_path,
                &[("authorization", &auth)],
                json!({"operation_id":op()})
            )
            .await
            .status,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
