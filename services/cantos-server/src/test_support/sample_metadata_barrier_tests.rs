//! Database barriers for v25, using synthetic evidence and a real run-owned Turso file.
//! These tests do not authorize or publish metadata; application checks remain separate.
use super::*;

const WORK: &str = "b4630614-a59d-4a5f-8d34-ae9433f01d2a";
const UNIT: &str = "98e5f9c9-9add-4dad-b0b3-03dcf110f120";
const OTHER_WORK: &str = "5de35d0f-ae81-43b5-aed6-f64057f95ef7";
const SOURCE: &str = "d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212";
const OP1: &str = "00000000-0000-4000-8000-000000000001";
const OP2: &str = "00000000-0000-4000-8000-000000000002";

async fn fixture() -> (TestRun, ServerDatabase) {
    let run = TestRun::new("sample-metadata-barriers");
    let path = run.path("server.db");
    let db = open(&path, Intent::Migrate).await.unwrap();
    let wd = "a".repeat(64);
    let ud = "b".repeat(64);
    let statements = vec![
        "INSERT INTO actors(id,active) VALUES('alice',1),('bob',1)".to_owned(),
        format!("INSERT INTO catalog_works VALUES('{WORK}','alice','story','draft',1)"),
        format!("INSERT INTO catalog_work_revisions VALUES('{WORK}',1,x'7b7d','{wd}','alice')"),
        format!("INSERT INTO catalog_works VALUES('{OTHER_WORK}','alice','story','draft',1)"),
        format!("INSERT INTO catalog_work_revisions VALUES('{OTHER_WORK}',1,x'7b7d','{wd}','alice')"),
        format!("INSERT INTO catalog_units VALUES('{UNIT}','{WORK}','story','chapter',1,0,'draft')"),
        format!("INSERT INTO catalog_unit_revisions VALUES('{UNIT}',1,x'7b7d','{ud}','alice')"),
        "INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','source','sample-catalog-input-v1','synthetic source'),('alice','rights','sample-rights','synthetic rights')".to_owned(),
        format!("INSERT INTO source_records(owner_id,id,reference,original_text,sha256,recorded_at) VALUES('alice','sample-catalog-input-v1','synthetic','synthetic','{SOURCE}',1)"),
    ];
    let tx = db.owner().begin_write().await.unwrap();
    for sql in statements {
        tx.execute(&sql, ()).await.unwrap();
    }
    tx.commit().await.unwrap();
    (run, db)
}

fn disclosure(subject_kind: &str, subject: &str, seq: u32, op: &str) -> String {
    let (unit_revision, unit_digest) = if subject_kind == "unit" {
        ("1".to_owned(), format!("'{}'", "b".repeat(64)))
    } else {
        ("NULL".to_owned(), "NULL".to_owned())
    };
    format!("INSERT INTO catalog_sample_metadata_disclosures VALUES('{WORK}','{subject_kind}','{subject}',{seq},'alice','disclosed',1,'{}',{unit_revision},{unit_digest},'sample-catalog-input-v1','{SOURCE}','rights','sample-rights','alice',1,'{op}','{}')", "a".repeat(64), "c".repeat(64))
}

#[tokio::test]
async fn disclosure_pins_and_append_only_sequence_fail_closed_on_real_turso() {
    let (_run, db) = fixture().await;
    let first = disclosure("work", WORK, 1, OP1);
    commit(&db, &[&first]).await;
    for sql in [
        "UPDATE catalog_sample_metadata_disclosures SET decision=decision".to_owned(),
        "DELETE FROM catalog_sample_metadata_disclosures".to_owned(),
        "INSERT OR REPLACE INTO catalog_sample_metadata_disclosures SELECT * FROM catalog_sample_metadata_disclosures".to_owned(),
    ] {
        assert_eq!(attempt(&db, &sql, vec![]).await, Err(guard("immutable.sample_metadata")), "{sql}");
    }
    assert_eq!(
        attempt(&db, &disclosure("work", WORK, 3, OP2), vec![]).await,
        Err(guard("binding.sample_metadata_sequence"))
    );
    let wrong_digest = disclosure("unit", UNIT, 1, OP2).replace(&"b".repeat(64), &"d".repeat(64));
    assert_eq!(
        attempt(&db, &wrong_digest, vec![]).await,
        Err(guard("binding.sample_metadata_pins"))
    );
    // Both revisions exist, but this unit is owned by WORK, not OTHER_WORK.
    let wrong_parent = disclosure("unit", UNIT, 1, OP2).replacen(WORK, OTHER_WORK, 1);
    assert_eq!(
        attempt(&db, &wrong_parent, vec![]).await,
        Err(guard("binding.sample_metadata_unit"))
    );
    let null_pin =
        disclosure("unit", UNIT, 1, OP2).replace(&format!("1,'{}'", "b".repeat(64)), "NULL,NULL");
    assert_eq!(
        attempt(&db, &null_pin, vec![]).await,
        Err(guard("binding.sample_metadata_pins")),
        "unit disclosure without a pin"
    );
    let withdrawn = format!("INSERT INTO catalog_sample_metadata_disclosures VALUES('{WORK}','work','{WORK}',2,'alice','withdrawn',NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,'alice',2,'{OP2}','{}')", "d".repeat(64));
    commit(&db, &[&withdrawn]).await;
    assert_eq!(values(&db, "SELECT decision FROM catalog_sample_metadata_disclosures ORDER BY sequence DESC LIMIT 1").await,
        vec![vec![text("withdrawn")]]);
}
