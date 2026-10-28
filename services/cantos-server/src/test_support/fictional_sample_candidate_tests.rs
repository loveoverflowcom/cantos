#[tokio::test]
async fn fictional_sample_principal_refuses_login_and_out_of_scope_ownership() {
    let run = TestRun::new("fictional-seed-candidate");
    let path = run.path("candidate.db");
    let candidate = Migration {
        version: 27,
        name: "candidate_fictional_seed_principal",
        sql: include_str!("../platform/turso/catalog_fictional_sample.sql"),
    };
    let migrations: &'static [Migration] = Box::leak(
        MIGRATIONS[..26]
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
        "INSERT INTO actors(id) VALUES ('cantos_fictional_seed_v2')",
        (),
    )
    .await
    .unwrap();
    tx.execute("INSERT INTO fictional_seed_principals VALUES ('cantos_fictional_seed_v2','0ad007cb18d46b58df25dbc2e8ea98bf58bce752c9e6d640fc4a78a90ef779d6')", ()).await.unwrap();
    tx.commit().await.unwrap();

    for (sql, expected) in [
        ("INSERT INTO actor_external_identities VALUES ('issuer','subject','cantos_fictional_seed_v2',1)", "binding.fictional_seed_no_login"),
        ("INSERT INTO identity_sessions(id,actor_id,channel,issuer,subject,assurance,authenticated_at,observed_security_version,observed_instance_epoch,absolute_expires,idle_expires) VALUES ('00000000-0000-4000-8000-000000000002','cantos_fictional_seed_v2','studio-web','cantos-identity','11111111111111111111111111111111','password',1,1,x'01010101010101010101010101010101',5,5)", "binding.fictional_seed_no_login"),
        ("INSERT INTO development_sessions(token_hash,actor_id,expires_at,session_id) VALUES (zeroblob(32),'cantos_fictional_seed_v2',9,'00000000-0000-4000-8000-000000000001')", "binding.fictional_seed_no_login"),
        ("INSERT INTO catalog_members(work_id,actor_id,role) VALUES ('00000000-0000-4000-8000-000000000001','cantos_fictional_seed_v2','editor')", "binding.fictional_seed_no_grant"),
        ("INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES ('00000000-0000-4000-8000-000000000001','cantos_fictional_seed_v2','story','draft',1)", "binding.fictional_seed_scope"),
        ("UPDATE fictional_seed_principals SET source_sha256=source_sha256", "immutable.fictional_seed_principal"),
        ("DELETE FROM fictional_seed_principals", "immutable.fictional_seed_principal"),
    ] {
        let tx = owner.begin_write().await.unwrap();
        assert_eq!(tx.execute(sql, ()).await.unwrap_err(), guard(expected));
        tx.rollback().await.unwrap();
    }
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES ('dbe1f751-0e5f-47d2-8910-62e2eafb2dcf','cantos_fictional_seed_v2','story','draft',1)", ()).await.unwrap();
    tx.execute("INSERT INTO catalog_work_revisions VALUES ('dbe1f751-0e5f-47d2-8910-62e2eafb2dcf',1,x'01','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','cantos_fictional_seed_v2')", ()).await.unwrap();
    tx.commit().await.unwrap();
    let tx = owner.begin_write().await.unwrap();
    assert_eq!(tx.execute("INSERT INTO catalog_units(id,work_id,work_kind,kind,head_revision,position,lifecycle) VALUES ('00000000-0000-4000-8000-000000000001','dbe1f751-0e5f-47d2-8910-62e2eafb2dcf','story','chapter',1,0,'draft')", ()).await.unwrap_err(), guard("binding.fictional_seed_scope"));
    tx.rollback().await.unwrap();
    // The database fence admits every ID in the independently defined domain allowlist.
    for (index, (work, units)) in
        crate::contexts::catalog::domain::metadata_disclosure::fictional_sample_ids()
            .iter()
            .enumerate()
    {
        let tx = owner.begin_write().await.unwrap();
        if index != 0 {
            tx.execute("INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES (?1,'cantos_fictional_seed_v2','story','draft',1)", params_from_iter([text(work)])).await.unwrap();
            tx.execute("INSERT INTO catalog_work_revisions VALUES (?1,1,x'01','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','cantos_fictional_seed_v2')", params_from_iter([text(work)])).await.unwrap();
        }
        for (position, unit) in units.iter().enumerate() {
            tx.execute("INSERT INTO catalog_units(id,work_id,work_kind,kind,head_revision,position,lifecycle) VALUES (?1,?2,'story','chapter',1,?3,'draft')", params_from_iter([text(unit), text(work), Value::Integer(position as i64)])).await.unwrap();
            tx.execute("INSERT INTO catalog_unit_revisions VALUES (?1,1,x'01','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','cantos_fictional_seed_v2')", params_from_iter([text(unit)])).await.unwrap();
        }
        tx.commit().await.unwrap();
    }
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO catalog_fictional_seed_actions VALUES ('00000000-0000-4000-8000-000000000001','cantos_fictional_seed_v2','publish','0ad007cb18d46b58df25dbc2e8ea98bf58bce752c9e6d640fc4a78a90ef779d6','operator reference','evidence reference','rights statement','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',1)", ()).await.unwrap();
    tx.commit().await.unwrap();
    for sql in [
        "UPDATE catalog_fictional_seed_actions SET statement='changed'",
        "DELETE FROM catalog_fictional_seed_actions",
        "INSERT OR REPLACE INTO catalog_fictional_seed_actions SELECT * FROM catalog_fictional_seed_actions",
    ] {
        let tx = owner.begin_write().await.unwrap();
        assert!(tx.execute(sql, ()).await.is_err(), "candidate accepted: {sql}");
        tx.rollback().await.unwrap();
    }
}
