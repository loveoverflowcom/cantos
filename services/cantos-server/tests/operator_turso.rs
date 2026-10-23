//! Actual operator executable, one stopped Server file, immutable source bytes and exact
//! registry receipts. No provider credentials, development login or HTTP authority is invented.
use cantos_server::{
    platform::store::StoreError,
    test_support::{
        editorial::{fixture, request, ALICE, SCRIPT},
        ServerFixture,
    },
};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    process::{Command, Output},
};

fn cli(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cantos-operator"))
        .env_clear()
        .env("CANTOS_SERVER_DATABASE", path)
        .args(args)
        .output()
        .unwrap()
}

#[tokio::test]
async fn actual_operator_registry_replays_bind_one_actor_and_never_issue_a_session() {
    let mut fixture = ServerFixture::new().await;
    let path = fixture.database_path();
    fixture.close();
    let first = cli(&path, &["register-identity", "issuer", "Subject"]);
    assert!(first.status.success());
    let actor = String::from_utf8(first.stdout).unwrap();
    assert!(uuid::Uuid::parse_str(actor.trim()).is_ok());
    let replay = cli(&path, &["register-identity", "issuer", "Subject"]);
    assert!(replay.status.success());
    assert_eq!(String::from_utf8(replay.stdout).unwrap(), actor);
    for expected in ["Linked", "Existing"] {
        let linked = cli(
            &path,
            &[
                "link-identity",
                actor.trim(),
                "other-issuer",
                "second-subject",
            ],
        );
        assert!(linked.status.success());
        assert_eq!(String::from_utf8(linked.stdout).unwrap().trim(), expected);
    }
    fixture.restart().await;
    let rows = fixture
        .rows(
            "SELECT issuer,subject,actor_id FROM actor_external_identities ORDER BY issuer",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    for (row, issuer, subject) in [
        (&rows[0], "issuer", "Subject"),
        (&rows[1], "other-issuer", "second-subject"),
    ] {
        assert_eq!(row.try_get::<_, String>(0).unwrap(), issuer);
        assert_eq!(row.try_get::<_, String>(1).unwrap(), subject);
        assert_eq!(row.try_get::<_, String>(2).unwrap(), actor.trim());
    }
    for (table, expected) in [
        ("actors", 1),
        ("identity_sessions", 0),
        ("identity_session_tokens", 0),
        ("development_sessions", 0),
    ] {
        assert_eq!(
            fixture
                .count(&format!("SELECT count(*) FROM {table}"), &[])
                .await,
            expected
        );
    }
}

#[tokio::test]
async fn actual_source_and_token_commands_preserve_originals_and_revoke_access_after_reopen() {
    let mut fixture = fixture().await;
    let path = fixture.database_path();
    let original = "Người dẫn chuyện\r\n  Bản thảo gốc.\n";
    let input = fixture.dir().join("original.txt");
    std::fs::write(&input, original).unwrap();
    let evidence_before: String = fixture.rows("SELECT description FROM script_evidence WHERE owner_id='alice' AND kind='source' AND id='source-demo'", &[]).await.unwrap()[0].try_get(0).unwrap();
    fixture.close();
    let args = [
        "record-source",
        "alice",
        "source-demo",
        "synthetic source",
        input.to_str().unwrap(),
    ];
    assert!(cli(&path, &args).status.success());
    assert!(
        !cli(&path, &args).status.success(),
        "a duplicate cannot rewrite the original"
    );
    fixture.restart().await;
    assert_eq!(fixture.rows("SELECT description FROM script_evidence WHERE owner_id='alice' AND kind='source' AND id='source-demo'", &[]).await.unwrap()[0].try_get::<_, String>(0).unwrap(), evidence_before);
    fixture
        .store()
        .save(ALICE, SCRIPT, request(0))
        .await
        .unwrap();
    let source = fixture
        .store()
        .source(ALICE, SCRIPT, "source-demo")
        .await
        .unwrap();
    assert_eq!(source.original_text, original);
    assert_eq!(
        source.sha256,
        format!("{:x}", Sha256::digest(original.as_bytes()))
    );
    assert!(matches!(
        fixture.store().source(ALICE, SCRIPT, "unlinked").await,
        Err(StoreError::NotFound)
    ));
    fixture.close();
    let created = cli(&path, &["create-creator"]);
    assert!(created.status.success());
    let actor = String::from_utf8(created.stdout).unwrap();
    assert!(!cli(&path, &["issue-token", actor.trim(), "0"])
        .status
        .success());
    let issued = cli(&path, &["issue-token", actor.trim(), "1"]);
    assert!(issued.status.success());
    let token = String::from_utf8(issued.stdout).unwrap();
    let token = token.trim();
    assert!(token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()));
    fixture.restart().await;
    assert_eq!(
        fixture.store().authenticate(token).await.unwrap(),
        actor.trim()
    );
    fixture.close();
    assert!(cli(&path, &["revoke-tokens", actor.trim()])
        .status
        .success());
    fixture.restart().await;
    assert!(matches!(
        fixture.store().authenticate(token).await,
        Err(StoreError::Unauthenticated)
    ));
    assert_eq!(
        fixture
            .store()
            .source(ALICE, SCRIPT, "source-demo")
            .await
            .unwrap(),
        source
    );
}
