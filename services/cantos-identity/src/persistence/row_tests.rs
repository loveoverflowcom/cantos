//! Production hydration receives malformed SELECT projections from the real engine.
//! No table constraint, trigger or foreign-key setting is weakened to construct a row.

use cantos_store_turso::registry::Intent;
use cantos_store_turso::testing::{FixedClock, TestRun};
use cantos_store_turso::{params_from_iter, Row, Value};

use super::rows::{decode_epoch, decode_event, decode_receipt, decode_subject};
use super::{ChangeKind, IdentityStore, IdentityStoreError};
use crate::domain::admission::AccountState;

fn id(value: u128) -> Value {
    Value::Blob(value.to_be_bytes().to_vec())
}

async fn store(run: &TestRun) -> IdentityStore {
    IdentityStore::open(run.path("rows.db"), Intent::Migrate, FixedClock::new(1))
        .await
        .unwrap()
}

async fn row(store: &IdentityStore, values: Vec<Value>) -> Row {
    let columns = (1..=values.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let reader = store.database.reader().await.unwrap();
    let mut rows = reader
        .query(&format!("SELECT {columns}"), params_from_iter(values))
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert!(rows.next().await.unwrap().is_none());
    row
}

#[tokio::test]
async fn instance_decoder_refuses_wrong_types_lengths_and_zero_without_changing_metadata() {
    let run = TestRun::new("identity-decode-epoch");
    let store = store(&run).await;
    let epoch = store.epoch();
    assert_eq!(
        decode_epoch(row(&store, vec![Value::Blob(epoch.to_bytes().to_vec())]).await),
        Ok(epoch)
    );
    for value in [
        Value::Null,
        Value::Integer(1),
        Value::Text("synthetic".into()),
        Value::Blob(vec![1; 15]),
        Value::Blob(vec![1; 17]),
        id(0),
    ] {
        assert_eq!(
            decode_epoch(row(&store, vec![value]).await),
            Err(IdentityStoreError::CorruptRecord)
        );
    }
    let reader = store.database.reader().await.unwrap();
    let mut rows = reader
        .query("SELECT instance_epoch FROM identity_instance", ())
        .await
        .unwrap();
    assert_eq!(decode_epoch(rows.next().await.unwrap().unwrap()), Ok(epoch));
    assert!(rows.next().await.unwrap().is_none());
}

#[tokio::test]
async fn subject_decoder_refuses_malformed_ids_versions_states_and_missing_columns() {
    let run = TestRun::new("identity-decode-subject");
    let store = store(&run).await;
    let valid = vec![id(1), Value::Integer(1), Value::Integer(1)];
    let subject = decode_subject(row(&store, valid.clone()).await, store.epoch()).unwrap();
    assert_eq!(subject.state, AccountState::Active);
    assert_eq!(subject.version.get(), 1);
    for (column, value) in [
        (0, Value::Null),
        (0, Value::Text("synthetic".into())),
        (0, Value::Blob(vec![1; 15])),
        (0, id(0)),
        (1, Value::Null),
        (1, Value::Text("1".into())),
        (1, Value::Real(1.0)),
        (1, Value::Integer(0)),
        (1, Value::Integer(-1)),
        (2, Value::Null),
        (2, Value::Text("1".into())),
        (2, Value::Real(1.0)),
        (2, Value::Integer(0)),
        (2, Value::Integer(3)),
    ] {
        let mut values = valid.clone();
        values[column] = value;
        assert_eq!(
            decode_subject(row(&store, values).await, store.epoch()),
            Err(IdentityStoreError::CorruptRecord),
            "malformed column {column}"
        );
    }
    assert_eq!(
        decode_subject(row(&store, vec![id(1)]).await, store.epoch()),
        Err(IdentityStoreError::CorruptRecord)
    );
}

#[tokio::test]
async fn receipt_decoder_refuses_invalid_bindings_and_version_relationships() {
    let run = TestRun::new("identity-decode-receipt");
    let store = store(&run).await;
    let created = vec![
        id(11),
        id(1),
        Value::Integer(1),
        Value::Integer(0),
        Value::Integer(1),
        Value::Integer(1),
    ];
    let receipt = decode_receipt(row(&store, created.clone()).await, store.epoch()).unwrap();
    assert_eq!(receipt.kind, ChangeKind::Created);
    assert_eq!(receipt.expected_version, None);
    for (column, value) in [
        (0, id(0)),
        (0, Value::Blob(vec![1; 17])),
        (1, id(0)),
        (1, Value::Null),
        (2, Value::Integer(3)),
        (2, Value::Text("1".into())),
        (2, Value::Real(1.0)),
        (3, Value::Text("0".into())),
        (3, Value::Real(0.0)),
        (3, Value::Integer(1)),
        (4, Value::Integer(2)),
        (4, Value::Integer(0)),
        (4, Value::Null),
        (4, Value::Real(1.0)),
        (5, Value::Integer(0)),
        (5, Value::Integer(-1)),
        (5, Value::Text("1".into())),
    ] {
        let mut values = created.clone();
        values[column] = value;
        assert_eq!(
            decode_receipt(row(&store, values).await, store.epoch()),
            Err(IdentityStoreError::CorruptRecord),
            "malformed column {column}"
        );
    }
    for (expected, committed) in [(0, 1), (-1, 1), (1, 1), (1, 3), (i64::MAX, i64::MAX)] {
        let mut values = created.clone();
        values[2] = Value::Integer(2);
        values[3] = Value::Integer(expected);
        values[4] = Value::Integer(committed);
        assert_eq!(
            decode_receipt(row(&store, values).await, store.epoch()),
            Err(IdentityStoreError::CorruptRecord),
            "invalid disable relationship {expected}/{committed}"
        );
    }
    let mut disabled = created;
    disabled[2] = Value::Integer(2);
    disabled[3] = Value::Integer(1);
    disabled[4] = Value::Integer(2);
    let receipt = decode_receipt(row(&store, disabled).await, store.epoch()).unwrap();
    assert_eq!(receipt.kind, ChangeKind::Disabled);
    assert_eq!(receipt.expected_version.unwrap().get(), 1);
    assert_eq!(receipt.committed_version.get(), 2);
    assert_eq!(
        decode_receipt(row(&store, vec![id(11)]).await, store.epoch()),
        Err(IdentityStoreError::CorruptRecord)
    );
}

#[tokio::test]
async fn event_decoder_refuses_invalid_kind_state_version_combinations() {
    let run = TestRun::new("identity-decode-event");
    let store = store(&run).await;
    let created = vec![
        Value::Integer(1),
        id(1),
        Value::Integer(1),
        Value::Integer(1),
        Value::Integer(1),
    ];
    let event = decode_event(row(&store, created.clone()).await).unwrap();
    assert_eq!(event.kind, ChangeKind::Created);
    assert_eq!(event.state, AccountState::Active);
    for (column, value) in [
        (0, Value::Integer(0)),
        (0, Value::Integer(-1)),
        (0, Value::Null),
        (0, Value::Real(1.0)),
        (1, id(0)),
        (1, Value::Blob(vec![1; 15])),
        (2, Value::Integer(0)),
        (2, Value::Integer(2)),
        (3, Value::Integer(2)),
        (3, Value::Integer(3)),
        (3, Value::Text("1".into())),
        (4, Value::Integer(2)),
        (4, Value::Integer(3)),
        (4, Value::Null),
    ] {
        let mut values = created.clone();
        values[column] = value;
        assert_eq!(
            decode_event(row(&store, values).await),
            Err(IdentityStoreError::CorruptRecord),
            "malformed column {column}"
        );
    }
    for (version, state) in [(1, 2), (2, 1)] {
        let mut values = created.clone();
        values[2] = Value::Integer(version);
        values[3] = Value::Integer(state);
        values[4] = Value::Integer(2);
        assert_eq!(
            decode_event(row(&store, values).await),
            Err(IdentityStoreError::CorruptRecord)
        );
    }
    let mut disabled = created;
    disabled[2] = Value::Integer(2);
    disabled[3] = Value::Integer(2);
    disabled[4] = Value::Integer(2);
    let event = decode_event(row(&store, disabled).await).unwrap();
    assert_eq!(event.kind, ChangeKind::Disabled);
    assert_eq!(event.state, AccountState::Disabled);
    assert_eq!(
        decode_event(row(&store, vec![Value::Integer(1)]).await),
        Err(IdentityStoreError::CorruptRecord)
    );
}
