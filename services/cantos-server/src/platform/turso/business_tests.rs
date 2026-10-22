//! Versions 12-15 (the CMP business API closure of #333) on the real pinned engine and run-owned
//! files: the upgrade from the exact registry of eleven that was deployed before them, and the
//! database-level barriers of every table they create, reached through raw SQL and never through
//! an adapter. A refusal is asserted by its exact guard code or constraint kind; no constraint is
//! weakened to build a fixture.
use super::*;
use crate::{
    contexts::listening::{auth::AuthenticatedListener, LibraryService},
    platform::store::Error,
    test_support::{
        library::{audio, bookmark, bookmark_delete, history_start, progress, saved},
        SeededWork, ServerFixture, WorkSeed,
    },
};
use cantos_contracts::listener_library as dto;

/// What a refused statement must say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Expect {
    Guard(&'static str),
    Check,
    ForeignKey,
}

fn matches(error: &Error, expected: Expect) -> bool {
    match expected {
        Expect::Guard(code) => error.guard() == Some(code.strip_prefix("cantos:").unwrap_or(code)),
        Expect::Check => error.constraint() == Some(ConstraintKind::Check),
        Expect::ForeignKey => error.constraint() == Some(ConstraintKind::ForeignKey),
    }
}

async fn expect_refused(fixture: &ServerFixture, label: &str, sql: &str, expected: Expect) {
    match fixture.try_exec(sql, &[]).await {
        Ok(changed) => panic!("{label}: accepted ({changed} rows): {sql}"),
        Err(error) => assert!(
            matches(&error, expected),
            "{label}: expected {expected:?}, got {error:?}\n{sql}"
        ),
    }
}

/// Every statement in `cases` must be refused as described and leave the file unchanged.
async fn refuse_all(fixture: &ServerFixture, tables: &[&str], cases: &[(&str, String, Expect)]) {
    let mut before = Vec::new();
    for table in tables {
        before.push(
            fixture
                .count(&format!("SELECT count(*) FROM {table}"), &[])
                .await,
        );
    }
    for (label, sql, expected) in cases {
        expect_refused(fixture, label, sql, *expected).await;
    }
    for (table, count) in tables.iter().zip(before) {
        assert_eq!(
            fixture
                .count(&format!("SELECT count(*) FROM {table}"), &[])
                .await,
            count,
            "{table} changed"
        );
    }
}

fn immutable(
    label: &str,
    table: &str,
    key: &str,
    code: &'static str,
) -> Vec<(String, String, Expect)> {
    vec![
        (
            format!("{label}: update"),
            format!("UPDATE {table} SET digest=digest WHERE {key}"),
            Expect::Guard(code),
        ),
        (
            format!("{label}: delete"),
            format!("DELETE FROM {table} WHERE {key}"),
            Expect::Guard(code),
        ),
        (
            format!("{label}: replace"),
            format!("INSERT OR REPLACE INTO {table} SELECT * FROM {table} WHERE {key}"),
            Expect::Guard(code),
        ),
        (
            format!("{label}: replace into"),
            format!("REPLACE INTO {table} SELECT * FROM {table} WHERE {key}"),
            Expect::Guard(code),
        ),
    ]
}

fn borrowed(cases: &[(String, String, Expect)]) -> Vec<(&str, String, Expect)> {
    cases
        .iter()
        .map(|(label, sql, expected)| (label.as_str(), sql.clone(), *expected))
        .collect()
}

async fn world() -> (ServerFixture, SeededWork) {
    let fixture = ServerFixture::new().await;
    let work = fixture
        .seed_work(
            None,
            &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Một lời hẹn", "Tập hai"]),
        )
        .await;
    (fixture, work)
}

fn principal(actor: &str) -> AuthenticatedListener {
    AuthenticatedListener::from_verified_actor(ActorId::new(actor).unwrap())
}

async fn history_of(owner: &OwnedDatabase) -> Vec<Vec<Value>> {
    let reader = owner.reader().await.unwrap();
    let mut rows = reader
        .query(
            "SELECT version,name,checksum FROM cantos_schema_history ORDER BY version",
            (),
        )
        .await
        .unwrap();
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        out.push((0..3).map(|index| row.get_value(index).unwrap()).collect());
    }
    out
}

#[tokio::test]
async fn the_registry_of_eleven_upgrades_to_the_business_closure_without_rewriting_a_checksum() {
    let run = TestRun::new("server-schema-business-upgrade");
    let path = run.path("server.db");
    let v11 = Registry::new(DatabaseKind::Server, SERVER_EPOCH, &MIGRATIONS[..11]).unwrap();
    let old = OwnedDatabase::open(OpenRequest::new(&path, v11, Intent::Migrate, clock()))
        .await
        .unwrap();
    let tx = old.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO actors(id,active) VALUES('alice',1),('bob',0)",
        (),
    )
    .await
    .unwrap();
    tx.execute(
        &format!("INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES('cantos-identity','{SUBJECT}','alice',7)"),
        (),
    )
    .await
    .unwrap();
    tx.execute(
        SESSION_INSERT,
        params_from_iter(session(S1, "theatre-native")),
    )
    .await
    .unwrap();
    // A revoked session beside a live one: the revocation must survive the upgrade.
    tx.execute(
        SESSION_INSERT,
        params_from_iter(with(
            with(session(S2, "theatre-web"), 11, Value::Integer(1)),
            12,
            text("logout"),
        )),
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO development_sessions(token_hash,actor_id,expires_at,revoked,session_id) VALUES(zeroblob(32),'alice',9,1,?1)",
        params_from_iter([text(S3)]),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let registered = history_of(&old).await;
    assert_eq!(registered.len(), 11);
    drop(old);
    let before = bytes(&path);

    // Serving never upgrades: the file of eleven is refused unchanged until an explicit migration.
    assert_eq!(
        refused(&path, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::Refused(Refusal::PendingMigrations {
            applied: 11,
            latest: 15,
        }))
    );
    assert_eq!(bytes(&path), before);

    drop(open(&path, Intent::Migrate).await.unwrap());
    let database = open(&path, Intent::Serve).await.unwrap();
    let after = values(
        &database,
        "SELECT version,name,checksum FROM cantos_schema_history ORDER BY version",
    )
    .await;
    // Versions 1-11 keep the exact name and checksum they were registered with.
    assert_eq!(after[..11], registered[..]);
    assert_eq!(
        after[11..]
            .iter()
            .map(|row| row[1].clone())
            .collect::<Vec<_>>(),
        [
            "server_catalog_publication_read",
            "server_production_briefs",
            "server_audio_release_chain",
            "server_listener_library"
        ]
        .map(text)
    );
    assert_eq!(
        values(&database, "SELECT id,active FROM actors ORDER BY id").await,
        [
            [text("alice"), Value::Integer(1)],
            [text("bob"), Value::Integer(0)]
        ]
    );
    assert_eq!(
        values(
            &database,
            "SELECT id,channel,revoked,revoked_reason FROM identity_sessions ORDER BY id"
        )
        .await,
        [
            vec![
                text(S2),
                text("theatre-web"),
                Value::Integer(1),
                text("logout")
            ],
            vec![
                text(S1),
                text("theatre-native"),
                Value::Integer(0),
                Value::Null
            ],
        ]
    );
    assert_eq!(
        values(&database, "SELECT revoked FROM development_sessions").await,
        [[Value::Integer(1)]]
    );
    for table in [
        "catalog_works",
        "catalog_units",
        "production_briefs",
        "audio_releases",
        "audio_publication_pointers",
        "listener_state",
        "listener_progress",
    ] {
        assert_eq!(
            values(&database, &format!("SELECT count(*) FROM {table}")).await,
            [[Value::Integer(0)]],
            "{table} starts empty"
        );
    }
}

#[tokio::test]
async fn a_binary_of_eleven_refuses_the_file_of_fifteen_and_a_diverged_slice_is_never_adopted() {
    let run = TestRun::new("server-schema-business-refusals");
    let path = run.path("server.db");
    drop(open(&path, Intent::Migrate).await.unwrap());
    let before = bytes(&path);
    // What a binary that predates this slice would be: it knows eleven versions only.
    let old = Registry::new(DatabaseKind::Server, SERVER_EPOCH, &MIGRATIONS[..11]).unwrap();
    for intent in [Intent::Serve, Intent::Migrate] {
        match OwnedDatabase::open(OpenRequest::new(&path, old, intent, clock())).await {
            Err(OpenError::Refused(Refusal::NewerThanBinary {
                applied: 15,
                known: 11,
            })) => {}
            Err(other) => panic!("{other:?}"),
            Ok(_) => panic!("an older binary must not open a newer file"),
        }
    }
    assert_eq!(bytes(&path), before);
    // A file whose version 12 differs from the registered bytes is diverged, not adopted.
    let altered: &'static [Migration] = Box::leak(
        MIGRATIONS
            .iter()
            .copied()
            .map(|migration| {
                if migration.version == 12 {
                    Migration {
                        sql: "CREATE TABLE catalog_works(id TEXT PRIMARY KEY) STRICT;",
                        ..migration
                    }
                } else {
                    migration
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let diverged = run.path("diverged.db");
    foreign(
        &diverged,
        Registry::new(DatabaseKind::Server, SERVER_EPOCH, altered).unwrap(),
    )
    .await;
    let before = bytes(&diverged);
    assert!(matches!(
        refused(&diverged, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::Refused(Refusal::HistoryDiverged { version: 12 }))
    ));
    assert_eq!(bytes(&diverged), before);
}

#[tokio::test]
async fn catalog_rows_are_well_formed_keep_their_identity_and_their_history_is_append_only() {
    let (fixture, work) = world().await;
    let unit = &work.units[0];
    let mut cases = vec![
        (
            "a work id outside the canonical shape",
            "INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES('NOT-A-UUID','alice','podcast','draft',1)".to_owned(),
            Expect::Check,
        ),
        (
            "an unknown work kind",
            "INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES('aaaaaaaa-0000-4000-8000-000000000001','alice','radio','draft',1)".to_owned(),
            Expect::Check,
        ),
        (
            "a head revision of zero",
            "INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES('aaaaaaaa-0000-4000-8000-000000000001','alice','podcast','draft',0)".to_owned(),
            Expect::Check,
        ),
        (
            "an owner that is no actor",
            "INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES('aaaaaaaa-0000-4000-8000-000000000001','nobody','podcast','draft',1)".to_owned(),
            Expect::ForeignKey,
        ),
        (
            "a podcast chapter",
            format!("INSERT INTO catalog_units(id,work_id,work_kind,kind,head_revision,\"position\",lifecycle) VALUES('aaaaaaaa-0000-4000-8000-000000000002','{}','podcast','chapter',1,9,'draft')", work.work_id),
            Expect::Check,
        ),
        (
            "a unit under a work of another kind",
            format!("INSERT INTO catalog_units(id,work_id,work_kind,kind,head_revision,\"position\",lifecycle) VALUES('aaaaaaaa-0000-4000-8000-000000000002','{}','novel','chapter',1,9,'draft')", work.work_id),
            Expect::ForeignKey,
        ),
        (
            "a unit taking an existing position",
            format!("INSERT INTO catalog_units(id,work_id,work_kind,kind,head_revision,\"position\",lifecycle) VALUES('aaaaaaaa-0000-4000-8000-000000000002','{}','podcast','episode',1,1,'draft')", work.work_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
        (
            "an empty work snapshot",
            format!("INSERT INTO catalog_work_revisions VALUES('{}',9,x'','{}','alice')", work.work_id, "a".repeat(64)),
            Expect::Check,
        ),
        (
            "an uppercase snapshot digest",
            format!("INSERT INTO catalog_work_revisions VALUES('{}',9,x'7b7d','{}','alice')", work.work_id, "A".repeat(64)),
            Expect::Check,
        ),
        (
            "a revision of an unknown work",
            format!("INSERT INTO catalog_work_revisions VALUES('aaaaaaaa-0000-4000-8000-0000000000ff',9,x'7b7d','{}','alice')", "a".repeat(64)),
            Expect::ForeignKey,
        ),
        (
            "a content digest outside its scheme",
            format!("INSERT INTO catalog_content_revisions VALUES('aaaaaaaa-0000-4000-8000-000000000003','{}',9,x'7b7d','cc-r1:sha256:zz','alice')", unit.unit_id),
            Expect::Check,
        ),
        (
            "a content head naming a missing revision",
            format!("UPDATE catalog_content_heads SET revision_id='aaaaaaaa-0000-4000-8000-0000000000fe',revision=7 WHERE unit_id='{}'", unit.unit_id),
            Expect::ForeignKey,
        ),
        (
            "a work changing owner",
            format!("UPDATE catalog_works SET owner_id='bob' WHERE id='{}'", work.work_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
        (
            "a work changing kind",
            format!("UPDATE catalog_works SET kind='novel' WHERE id='{}'", work.work_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
        (
            "a work being deleted",
            format!("DELETE FROM catalog_works WHERE id='{}'", work.work_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
        (
            "a work being replaced",
            format!("INSERT OR REPLACE INTO catalog_works SELECT * FROM catalog_works WHERE id='{}'", work.work_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
        (
            "a unit changing parent",
            format!("UPDATE catalog_units SET work_id='aaaaaaaa-0000-4000-8000-0000000000ff' WHERE id='{}'", unit.unit_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
        (
            "a unit being deleted",
            format!("DELETE FROM catalog_units WHERE id='{}'", unit.unit_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
        (
            "a unit being replaced",
            format!("REPLACE INTO catalog_units SELECT * FROM catalog_units WHERE id='{}'", unit.unit_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
        (
            "a content head changing unit",
            format!("UPDATE catalog_content_heads SET unit_id='{}' WHERE unit_id='{}'", work.units[1].unit_id, unit.unit_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
        (
            "a content head being deleted",
            format!("DELETE FROM catalog_content_heads WHERE unit_id='{}'", unit.unit_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
        (
            "a content head being replaced",
            format!("INSERT OR REPLACE INTO catalog_content_heads SELECT * FROM catalog_content_heads WHERE unit_id='{}'", unit.unit_id),
            Expect::Guard("cantos:immutable.catalog_identity"),
        ),
    ];
    for (label, table, key) in [
        (
            "work revision",
            "catalog_work_revisions",
            format!("work_id='{}'", work.work_id),
        ),
        (
            "unit revision",
            "catalog_unit_revisions",
            format!("unit_id='{}'", unit.unit_id),
        ),
        (
            "content revision",
            "catalog_content_revisions",
            format!("unit_id='{}'", unit.unit_id),
        ),
    ] {
        let set = match table {
            "catalog_content_revisions" => "revision_digest=revision_digest",
            _ => "snapshot_digest=snapshot_digest",
        };
        for (what, sql) in [
            ("update", format!("UPDATE {table} SET {set} WHERE {key}")),
            ("delete", format!("DELETE FROM {table} WHERE {key}")),
            (
                "replace",
                format!("INSERT OR REPLACE INTO {table} SELECT * FROM {table} WHERE {key}"),
            ),
            (
                "upsert",
                format!(
                    "INSERT INTO {table} SELECT * FROM {table} WHERE {key} ON CONFLICT DO NOTHING"
                ),
            ),
        ] {
            cases.push((
                Box::leak(format!("{label}: {what}").into_boxed_str()) as &str,
                sql,
                Expect::Guard("cantos:immutable.catalog_history"),
            ));
        }
    }
    let cases: Vec<(&str, String, Expect)> = cases.into_iter().collect();
    refuse_all(
        &fixture,
        &[
            "catalog_works",
            "catalog_units",
            "catalog_work_revisions",
            "catalog_unit_revisions",
            "catalog_content_revisions",
            "catalog_content_heads",
        ],
        &cases,
    )
    .await;

    // A head revision with no row behind it is refused when the unit of work commits.
    let error = fixture
        .try_exec(
            "INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES('aaaaaaaa-0000-4000-8000-000000000001','alice','podcast','draft',1)",
            &[],
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.constraint(),
        Some(ConstraintKind::ForeignKey),
        "{error:?}"
    );
    // What a head may do: advance in place to a revision that exists.
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM catalog_works WHERE head_revision=1",
                &[]
            )
            .await,
        1
    );
}

#[tokio::test]
async fn the_audio_release_chain_is_digest_checked_bound_and_immutable_and_the_pointer_only_moves_forward(
) {
    let (fixture, work) = world().await;
    let unit = &work.units[0];
    let release = unit.chain.release_id.clone().unwrap();
    let mut cases: Vec<(String, String, Expect)> = Vec::new();
    for (label, table, key, code) in [
        (
            "submission",
            "audio_submissions",
            format!("id='{}'", unit.chain.submission_id),
            "cantos:immutable.audio_submissions",
        ),
        (
            "rendition",
            "audio_renditions",
            format!("id='{}'", unit.chain.rendition_id),
            "cantos:immutable.audio_renditions",
        ),
        (
            "candidate",
            "audio_release_candidates",
            format!("id='{}'", unit.chain.candidate_id),
            "cantos:immutable.audio_release_candidates",
        ),
        (
            "clearance",
            "audio_distribution_clearances",
            format!("candidate_id='{}'", unit.chain.candidate_id),
            "cantos:immutable.audio_distribution_clearances",
        ),
        (
            "approval",
            "audio_publication_approvals",
            format!("candidate_id='{}'", unit.chain.candidate_id),
            "cantos:immutable.audio_publication_approvals",
        ),
        (
            "release",
            "audio_releases",
            format!("id='{release}'"),
            "cantos:immutable.audio_releases",
        ),
        (
            "brief",
            "production_briefs",
            format!("unit_id='{}'", unit.unit_id),
            "cantos:immutable.production_briefs",
        ),
    ] {
        cases.extend(immutable(label, table, &key, code));
    }
    for (label, table, key) in [
        (
            "listener work anchor",
            "audio_listener_works",
            format!("work_id='{}'", work.work_id),
        ),
        (
            "listener unit anchor",
            "audio_listener_units",
            format!("unit_id='{}'", unit.unit_id),
        ),
        (
            "listener release row",
            "audio_listener_releases",
            format!("release_id='{release}'"),
        ),
    ] {
        cases.extend([
            (
                format!("{label}: update"),
                format!(
                    "UPDATE {table} SET {col} WHERE {key}",
                    col = match table {
                        "audio_listener_works" => "first_published_at=first_published_at",
                        "audio_listener_units" => "first_position=first_position",
                        _ => "published_at=published_at",
                    }
                ),
                Expect::Guard("cantos:immutable.audio_listener_index"),
            ),
            (
                format!("{label}: delete"),
                format!("DELETE FROM {table} WHERE {key}"),
                Expect::Guard("cantos:immutable.audio_listener_index"),
            ),
            (
                format!("{label}: replace"),
                format!("INSERT OR REPLACE INTO {table} SELECT * FROM {table} WHERE {key}"),
                Expect::Guard("cantos:immutable.audio_listener_index"),
            ),
        ]);
    }
    let pointer = format!("unit_id='{}'", unit.unit_id);
    cases.extend([
        (
            "pointer skipping a version".to_owned(),
            format!("UPDATE audio_publication_pointers SET version=version+2 WHERE {pointer}"),
            Expect::Guard("cantos:audio_publication_pointers.forward"),
        ),
        (
            "pointer standing still".to_owned(),
            format!("UPDATE audio_publication_pointers SET active_release_id=NULL WHERE {pointer}"),
            Expect::Guard("cantos:audio_publication_pointers.forward"),
        ),
        (
            "pointer changing owner".to_owned(),
            format!("UPDATE audio_publication_pointers SET owner_id='bob',version=version+1 WHERE {pointer}"),
            Expect::Guard("cantos:audio_publication_pointers.forward"),
        ),
        (
            "pointer being deleted".to_owned(),
            format!("DELETE FROM audio_publication_pointers WHERE {pointer}"),
            Expect::Guard("cantos:immutable.audio_publication_pointers"),
        ),
        (
            "pointer being replaced".to_owned(),
            format!("INSERT OR REPLACE INTO audio_publication_pointers SELECT * FROM audio_publication_pointers WHERE {pointer}"),
            Expect::Guard("cantos:immutable.audio_publication_pointers"),
        ),
        (
            "pointer naming another unit's release".to_owned(),
            format!(
                "UPDATE audio_publication_pointers SET active_release_id='{}',version=version+1 WHERE {pointer}",
                work.units[1].chain.release_id.clone().unwrap()
            ),
            Expect::ForeignKey,
        ),
        (
            "a release digest outside its scheme".to_owned(),
            "INSERT INTO audio_releases VALUES('aaaaaaaa-0000-4000-8000-0000000000a1','x','alice','x','x',x'7b7d','rm-r1:sha256:aa','rl-r1:sha256:bb')".to_owned(),
            Expect::Check,
        ),
        (
            "a release of an unknown candidate".to_owned(),
            format!(
                "INSERT INTO audio_releases VALUES('aaaaaaaa-0000-4000-8000-0000000000a1','{}','alice','aaaaaaaa-0000-4000-8000-0000000000a2','{}',x'7b7d','rm-r1:sha256:{}','rl-r1:sha256:{}')",
                unit.unit_id, unit.chain.rendition_id, "a".repeat(64), "b".repeat(64)
            ),
            Expect::ForeignKey,
        ),
        (
            "a submission token reused".to_owned(),
            format!(
                "INSERT INTO audio_submissions SELECT 'aaaaaaaa-0000-4000-8000-0000000000a3',unit_id,owner_id,brief_id,brief_digest,'aaaaaaaa-0000-4000-8000-0000000000a4',request_digest,token_hash,snapshot,digest FROM audio_submissions WHERE id='{}'",
                unit.chain.submission_id
            ),
            Expect::Guard("cantos:immutable.audio_submissions"),
        ),
        (
            "a submission bound to no brief".to_owned(),
            format!(
                "INSERT INTO audio_submissions SELECT 'aaaaaaaa-0000-4000-8000-0000000000a3',unit_id,owner_id,'aaaaaaaa-0000-4000-8000-0000000000a5',brief_digest,'aaaaaaaa-0000-4000-8000-0000000000a4',request_digest,randomblob(32),snapshot,digest FROM audio_submissions WHERE id='{}'",
                unit.chain.submission_id
            ),
            Expect::ForeignKey,
        ),
        (
            "a rendition of a second submission".to_owned(),
            format!(
                "INSERT INTO audio_renditions SELECT 'aaaaaaaa-0000-4000-8000-0000000000a6',submission_id,object_key,snapshot,digest,storage_target_id FROM audio_renditions WHERE id='{}'",
                unit.chain.rendition_id
            ),
            Expect::Guard("cantos:immutable.audio_renditions"),
        ),
        (
            "a withdrawal of an unknown release".to_owned(),
            "INSERT INTO audio_release_withdrawals VALUES('aaaaaaaa-0000-4000-8000-0000000000a7','alice',1,'x')".to_owned(),
            Expect::ForeignKey,
        ),
        (
            "a withdrawal with no reason".to_owned(),
            format!("INSERT INTO audio_release_withdrawals VALUES('{release}','alice',1,'')"),
            Expect::Check,
        ),
        (
            "a listener row whose rights end before they start".to_owned(),
            format!(
                "INSERT INTO audio_listener_releases SELECT 'aaaaaaaa-0000-4000-8000-0000000000a8',work_id,unit_id,published_at,kind,language,search_text,rights_digest,approval_digest,rights_end,rights_start,work_revision,unit_revision,content_revision_id FROM audio_listener_releases WHERE release_id='{release}'"
            ),
            Expect::Check,
        ),
        (
            "a listener row of an unknown kind".to_owned(),
            format!(
                "INSERT INTO audio_listener_releases SELECT 'aaaaaaaa-0000-4000-8000-0000000000a9',work_id,unit_id,published_at,'radio',language,search_text,rights_digest,approval_digest,rights_start,rights_end,work_revision,unit_revision,content_revision_id FROM audio_listener_releases WHERE release_id='{release}'"
            ),
            Expect::Check,
        ),
        (
            "a listener row pairing a unit with another work".to_owned(),
            format!(
                "INSERT INTO audio_listener_releases SELECT 'aaaaaaaa-0000-4000-8000-0000000000a8','aaaaaaaa-0000-4000-8000-0000000000ff',unit_id,published_at,kind,language,search_text,rights_digest,approval_digest,rights_start,rights_end,work_revision,unit_revision,content_revision_id FROM audio_listener_releases WHERE release_id='{release}'"
            ),
            Expect::ForeignKey,
        ),
    ]);
    let cases = borrowed(&cases);
    refuse_all(
        &fixture,
        &[
            "audio_submissions",
            "audio_renditions",
            "audio_release_candidates",
            "audio_distribution_clearances",
            "audio_publication_approvals",
            "audio_releases",
            "audio_publication_pointers",
            "audio_release_withdrawals",
            "audio_listener_works",
            "audio_listener_units",
            "audio_listener_releases",
            "production_briefs",
        ],
        &cases,
    )
    .await;
    // The one legal movement: forward by exactly one version, withdrawing or replacing.
    assert_eq!(
        fixture
            .exec(
                &format!("UPDATE audio_publication_pointers SET active_release_id=NULL,version=version+1 WHERE {pointer}"),
                &[],
            )
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn library_rows_are_checked_against_their_snapshots_and_only_move_as_their_guards_allow() {
    let (fixture, work) = world().await;
    let unit = &work.units[0];
    let a = audio(&work, unit);
    let alice = principal("alice");
    let service = LibraryService::new(fixture.store().clone());
    service
        .put_progress(
            &alice,
            &a.publication_id,
            progress(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    service
        .set_saved(&alice, &a.work_id, saved(1, 90, 0, true))
        .await
        .unwrap();
    service
        .playback_started(&alice, history_start(&a, 1, 90, 99, 0))
        .await
        .unwrap();
    service
        .create_bookmark(
            &alice,
            &format!("00000000-0000-4000-8000-{:012x}", 0xb01),
            bookmark(1, &a, 2, 90, 6000),
        )
        .await
        .unwrap();
    let bookmark_id = format!("00000000-0000-4000-8000-{:012x}", 0xb01);
    let moved =
        |table: &str, set: &str| format!("UPDATE {table} SET {set} WHERE listener_id='alice'");
    let cases: Vec<(String, String, Expect)> = vec![
        // Snapshot/column agreement: a copied scalar that no longer matches its snapshot.
        (
            "saved flag flipped beside its snapshot".to_owned(),
            moved(
                "listener_saved",
                "saved=0,revision=revision+1,listener_order=listener_order+1,snapshot=CAST(json_set(CAST(snapshot AS TEXT),'$.revision',revision+1,'$.listener_order',listener_order+1) AS BLOB),snapshot_digest=randomblob(32)",
            ),
            Expect::Check,
        ),
        (
            "completion flipped beside its snapshot".to_owned(),
            moved(
                "listener_progress",
                "completion='completed',revision=revision+1,listener_order=listener_order+1,snapshot=CAST(json_set(CAST(snapshot AS TEXT),'$.revision',revision+1,'$.listener_order',listener_order+1) AS BLOB),snapshot_digest=randomblob(32)",
            ),
            Expect::Check,
        ),
        (
            "progress position beyond the duration".to_owned(),
            moved("listener_progress", "position_ms=duration_ms+1,revision=revision+1,listener_order=listener_order+1"),
            Expect::Check,
        ),
        // Forward-only guards.
        (
            "progress revision skipping".to_owned(),
            moved("listener_progress", "revision=revision+2,listener_order=listener_order+1"),
            Expect::Guard("cantos:listener_progress.forward"),
        ),
        (
            "progress order standing still".to_owned(),
            moved("listener_progress", "revision=revision+1"),
            Expect::Guard("cantos:listener_progress.forward"),
        ),
        (
            "saved revision skipping".to_owned(),
            moved("listener_saved", "revision=revision+2,listener_order=listener_order+1"),
            Expect::Guard("cantos:listener_saved.forward"),
        ),
        (
            "account counters moving backwards".to_owned(),
            moved("listener_state", "revision=revision-1"),
            Expect::Guard("cantos:listener_state.forward"),
        ),
        (
            "device sequence moving backwards".to_owned(),
            moved("listener_device_sequences", "sequence=sequence-1"),
            Expect::Guard("cantos:listener_device_sequences.forward"),
        ),
        (
            "device sequence standing still".to_owned(),
            moved("listener_device_sequences", "sequence=sequence"),
            Expect::Guard("cantos:listener_device_sequences.forward"),
        ),
        (
            "bookmark position changing".to_owned(),
            format!("UPDATE listener_bookmarks SET position_ms=position_ms+1 WHERE bookmark_id='{bookmark_id}'"),
            Expect::Guard("cantos:listener_bookmarks.tombstone"),
        ),
        (
            "a tombstone that does not move the account order".to_owned(),
            format!("UPDATE listener_bookmarks SET deleted=1 WHERE bookmark_id='{bookmark_id}'"),
            Expect::Guard("cantos:listener_bookmarks.tombstone"),
        ),
        // Deletion, replacement and settled records.
        ("progress delete".to_owned(), "DELETE FROM listener_progress".to_owned(), Expect::Guard("cantos:immutable.listener_progress")),
        ("saved delete".to_owned(), "DELETE FROM listener_saved".to_owned(), Expect::Guard("cantos:immutable.listener_saved")),
        ("state delete".to_owned(), "DELETE FROM listener_state".to_owned(), Expect::Guard("cantos:immutable.listener_state")),
        ("bookmark delete".to_owned(), "DELETE FROM listener_bookmarks".to_owned(), Expect::Guard("cantos:immutable.listener_bookmarks")),
        ("sequence delete".to_owned(), "DELETE FROM listener_device_sequences".to_owned(), Expect::Guard("cantos:immutable.listener_device_sequences")),
        (
            "progress replace".to_owned(),
            "INSERT OR REPLACE INTO listener_progress SELECT * FROM listener_progress".to_owned(),
            Expect::Guard("cantos:immutable.listener_progress"),
        ),
        (
            "saved replace".to_owned(),
            "INSERT OR REPLACE INTO listener_saved SELECT * FROM listener_saved".to_owned(),
            Expect::Guard("cantos:immutable.listener_saved"),
        ),
        (
            "state replace".to_owned(),
            "REPLACE INTO listener_state SELECT * FROM listener_state".to_owned(),
            Expect::Guard("cantos:immutable.listener_state"),
        ),
        (
            "bookmark replace".to_owned(),
            "INSERT OR REPLACE INTO listener_bookmarks SELECT * FROM listener_bookmarks".to_owned(),
            Expect::Guard("cantos:immutable.listener_bookmarks"),
        ),
        ("operation update".to_owned(), "UPDATE listener_operations SET request_digest=randomblob(32)".to_owned(), Expect::Guard("cantos:immutable.listener_operations")),
        ("operation delete".to_owned(), "DELETE FROM listener_operations".to_owned(), Expect::Guard("cantos:immutable.listener_operations")),
        (
            "operation replace".to_owned(),
            "INSERT OR REPLACE INTO listener_operations SELECT * FROM listener_operations".to_owned(),
            Expect::Guard("cantos:immutable.listener_operations"),
        ),
        ("session update".to_owned(), "UPDATE listener_playback_sessions SET audio_snapshot=x'7b7d'".to_owned(), Expect::Guard("cantos:immutable.listener_playback_sessions")),
        ("session delete".to_owned(), "DELETE FROM listener_playback_sessions".to_owned(), Expect::Guard("cantos:immutable.listener_playback_sessions")),
        (
            "session replace".to_owned(),
            "INSERT OR REPLACE INTO listener_playback_sessions SELECT * FROM listener_playback_sessions".to_owned(),
            Expect::Guard("cantos:immutable.listener_playback_sessions"),
        ),
        // Reference and shape checks.
        (
            "an account that is no actor".to_owned(),
            "INSERT INTO listener_state(listener_id) VALUES('nobody')".to_owned(),
            Expect::ForeignKey,
        ),
        (
            "history epoch beyond history revision".to_owned(),
            "INSERT INTO listener_state(listener_id,history_revision,history_epoch) VALUES('bob',0,1)".to_owned(),
            Expect::Check,
        ),
        (
            "progress of an unknown publication".to_owned(),
            "INSERT INTO listener_progress SELECT listener_id,'aaaaaaaa-0000-4000-8000-0000000000ff',work_id,unit_id,rendition_id,sha256,duration_ms,position_ms,completion,revision,listener_order,accepted_at,CAST(json_set(CAST(snapshot AS TEXT),'$.audio.publication_id','aaaaaaaa-0000-4000-8000-0000000000ff') AS BLOB),snapshot_digest FROM listener_progress".to_owned(),
            Expect::ForeignKey,
        ),
        (
            "a receipt with a malformed operation id".to_owned(),
            "INSERT INTO listener_operations SELECT listener_id,'not-a-uuid',request_digest,response_bytes,response_digest FROM listener_operations".to_owned(),
            Expect::Check,
        ),
        (
            "a receipt with a short digest".to_owned(),
            "INSERT INTO listener_operations SELECT listener_id,'aaaaaaaa-0000-4000-8000-0000000000ff',zeroblob(31),response_bytes,response_digest FROM listener_operations LIMIT 1".to_owned(),
            Expect::Check,
        ),
    ];
    let cases = borrowed(&cases);
    refuse_all(
        &fixture,
        &[
            "listener_state",
            "listener_progress",
            "listener_saved",
            "listener_history",
            "listener_operations",
            "listener_device_sequences",
            "listener_playback_sessions",
            "listener_bookmarks",
        ],
        &cases,
    )
    .await;
    // The allowed single tombstone, once and only once.
    service
        .delete_bookmark(&alice, &bookmark_id, bookmark_delete(3, 90))
        .await
        .unwrap();
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM listener_bookmarks WHERE deleted=1",
                &[]
            )
            .await,
        1
    );
    expect_refused(
        &fixture,
        "a second tombstone",
        &format!("UPDATE listener_bookmarks SET deleted=1,listener_order=listener_order+1 WHERE bookmark_id='{bookmark_id}'"),
        Expect::Guard("cantos:listener_bookmarks.tombstone"),
    )
    .await;
    expect_refused(
        &fixture,
        "resurrecting a tombstone",
        &format!("UPDATE listener_bookmarks SET deleted=0,listener_order=listener_order+1 WHERE bookmark_id='{bookmark_id}'"),
        Expect::Guard("cantos:listener_bookmarks.tombstone"),
    )
    .await;
}
