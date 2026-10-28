#[tokio::test]
async fn candidate_sample_principal_refuses_login_and_out_of_scope_ownership() {
    let run = TestRun::new("sample-seed-candidate");
    let path = run.path("candidate.db");
    let candidate = Migration {
        version: 26,
        name: "candidate_sample_seed_principal",
        sql: include_str!("../platform/turso/catalog_sample_seed_principal.sql"),
    };
    let migrations: &'static [Migration] = Box::leak(
        MIGRATIONS[..25]
            .iter()
            .copied()
            .chain([candidate])
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let registry = Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, migrations).unwrap();
    foreign(&path, registry).await;
    let owner = OwnedDatabase::open(OpenRequest::new(&path, registry, Intent::Serve, clock()))
        .await
        .unwrap();
    let tx = owner.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO actors(id) VALUES ('cantos_sample_seed_v1')",
        (),
    )
    .await
    .unwrap();
    tx.execute("INSERT INTO sample_seed_principals VALUES ('cantos_sample_seed_v1','d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212')", ()).await.unwrap();
    tx.commit().await.unwrap();

    for (sql, expected) in [
        ("INSERT INTO actor_external_identities VALUES ('issuer','subject','cantos_sample_seed_v1',1)", "binding.sample_seed_no_login"),
        ("INSERT INTO identity_sessions(id,actor_id,channel,issuer,subject,assurance,authenticated_at,observed_security_version,observed_instance_epoch,absolute_expires,idle_expires) VALUES ('00000000-0000-4000-8000-000000000002','cantos_sample_seed_v1','studio-web','cantos-identity','11111111111111111111111111111111','password',1,1,x'01010101010101010101010101010101',5,5)", "binding.sample_seed_no_login"),
        ("INSERT INTO development_sessions(token_hash,actor_id,expires_at,session_id) VALUES (zeroblob(32),'cantos_sample_seed_v1',9,'00000000-0000-4000-8000-000000000001')", "binding.sample_seed_no_login"),
        ("INSERT INTO catalog_members(work_id,actor_id,role) VALUES ('00000000-0000-4000-8000-000000000001','cantos_sample_seed_v1','editor')", "binding.sample_seed_no_grant"),
        ("INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES ('00000000-0000-4000-8000-000000000001','cantos_sample_seed_v1','story','draft',1)", "binding.sample_seed_scope"),
        ("UPDATE sample_seed_principals SET source_sha256=source_sha256", "immutable.sample_seed_principal"),
        ("DELETE FROM sample_seed_principals", "immutable.sample_seed_principal"),
    ] {
        let tx = owner.begin_write().await.unwrap();
        assert_eq!(tx.execute(sql, ()).await.unwrap_err(), guard(expected));
        tx.rollback().await.unwrap();
    }
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES ('b4630614-a59d-4a5f-8d34-ae9433f01d2a','cantos_sample_seed_v1','story','draft',1)", ()).await.unwrap();
    tx.execute("INSERT INTO catalog_work_revisions VALUES ('b4630614-a59d-4a5f-8d34-ae9433f01d2a',1,x'01','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','cantos_sample_seed_v1')", ()).await.unwrap();
    tx.commit().await.unwrap();
    let tx = owner.begin_write().await.unwrap();
    assert_eq!(tx.execute("INSERT INTO catalog_units(id,work_id,work_kind,kind,head_revision,position,lifecycle) VALUES ('00000000-0000-4000-8000-000000000001','b4630614-a59d-4a5f-8d34-ae9433f01d2a','story','chapter',1,0,'draft')", ()).await.unwrap_err(), guard("binding.sample_seed_scope"));
    tx.rollback().await.unwrap();
    // The database fence admits every ID in the independently defined domain allowlist.
    for (index, (work, units)) in
        crate::contexts::catalog::domain::metadata_disclosure::approved_sample_ids()
            .iter()
            .enumerate()
    {
        let tx = owner.begin_write().await.unwrap();
        if index != 0 {
            tx.execute("INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES (?1,'cantos_sample_seed_v1','story','draft',1)", params_from_iter([text(work)])).await.unwrap();
            tx.execute("INSERT INTO catalog_work_revisions VALUES (?1,1,x'01','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','cantos_sample_seed_v1')", params_from_iter([text(work)])).await.unwrap();
        }
        for (position, unit) in units.iter().enumerate() {
            tx.execute("INSERT INTO catalog_units(id,work_id,work_kind,kind,head_revision,position,lifecycle) VALUES (?1,?2,'story','chapter',1,?3,'draft')", params_from_iter([text(unit), text(work), Value::Integer(position as i64)])).await.unwrap();
            tx.execute("INSERT INTO catalog_unit_revisions VALUES (?1,1,x'01','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','cantos_sample_seed_v1')", params_from_iter([text(unit)])).await.unwrap();
        }
        tx.commit().await.unwrap();
    }
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO catalog_sample_seed_actions VALUES ('00000000-0000-4000-8000-000000000001','cantos_sample_seed_v1','publish','d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212','operator reference','evidence reference','rights statement','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',1)", ()).await.unwrap();
    tx.commit().await.unwrap();
    for sql in [
        "UPDATE catalog_sample_seed_actions SET statement='changed'",
        "DELETE FROM catalog_sample_seed_actions",
        "INSERT OR REPLACE INTO catalog_sample_seed_actions SELECT * FROM catalog_sample_seed_actions",
    ] {
        let tx = owner.begin_write().await.unwrap();
        assert!(tx.execute(sql, ()).await.is_err(), "candidate accepted: {sql}");
        tx.rollback().await.unwrap();
    }
}
