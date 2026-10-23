//! Checked row hydration; neither the engine nor raw integers construct authentication facts.

use cantos_store_turso::{Row, Value};

use super::{ChangeKind, IdentityStoreError, OperationReceipt, SecurityEvent, StoredSubject};
use crate::domain::{
    admission::AccountState,
    ids::{InstanceEpoch, OperationId, SubjectId},
    security_version::SecurityVersion,
};

pub(super) fn integer(row: &Row, index: usize) -> Result<i64, IdentityStoreError> {
    match row
        .get_value(index)
        .map_err(|_| IdentityStoreError::CorruptRecord)?
    {
        Value::Integer(value) => Ok(value),
        _ => Err(IdentityStoreError::CorruptRecord),
    }
}

pub(super) fn positive(value: i64) -> Result<u64, IdentityStoreError> {
    u64::try_from(value)
        .ok()
        .filter(|v| *v > 0)
        .ok_or(IdentityStoreError::CorruptRecord)
}

fn bytes(row: &Row, index: usize) -> Result<[u8; 16], IdentityStoreError> {
    match row
        .get_value(index)
        .map_err(|_| IdentityStoreError::CorruptRecord)?
    {
        Value::Blob(value) => value
            .try_into()
            .map_err(|_| IdentityStoreError::CorruptRecord),
        _ => Err(IdentityStoreError::CorruptRecord),
    }
}

fn version(row: &Row, index: usize) -> Result<SecurityVersion, IdentityStoreError> {
    SecurityVersion::new(positive(integer(row, index)?)?)
        .map_err(|_| IdentityStoreError::CorruptRecord)
}

fn subject(row: &Row, index: usize) -> Result<SubjectId, IdentityStoreError> {
    SubjectId::from_bytes(bytes(row, index)?).map_err(|_| IdentityStoreError::CorruptRecord)
}

fn state(row: &Row, index: usize) -> Result<AccountState, IdentityStoreError> {
    match integer(row, index)? {
        1 => Ok(AccountState::Active),
        2 => Ok(AccountState::Disabled),
        _ => Err(IdentityStoreError::CorruptRecord),
    }
}

fn kind(row: &Row, index: usize) -> Result<ChangeKind, IdentityStoreError> {
    match integer(row, index)? {
        1 => Ok(ChangeKind::Created),
        2 => Ok(ChangeKind::Disabled),
        _ => Err(IdentityStoreError::CorruptRecord),
    }
}

pub(super) fn decode_epoch(row: Row) -> Result<InstanceEpoch, IdentityStoreError> {
    InstanceEpoch::from_bytes(bytes(&row, 0)?).map_err(|_| IdentityStoreError::CorruptRecord)
}

pub(super) fn decode_subject(
    row: Row,
    epoch: InstanceEpoch,
) -> Result<StoredSubject, IdentityStoreError> {
    Ok(StoredSubject {
        subject: subject(&row, 0)?,
        epoch,
        version: version(&row, 1)?,
        state: state(&row, 2)?,
    })
}

pub(super) fn decode_receipt(
    row: Row,
    epoch: InstanceEpoch,
) -> Result<OperationReceipt, IdentityStoreError> {
    let kind = kind(&row, 2)?;
    let expected = integer(&row, 3)?;
    let committed_version = version(&row, 4)?;
    let expected_version = match kind {
        ChangeKind::Created if expected == 0 && committed_version.get() == 1 => None,
        ChangeKind::Disabled => {
            let value = SecurityVersion::new(positive(expected)?)
                .map_err(|_| IdentityStoreError::CorruptRecord)?;
            if value.successor().ok() != Some(committed_version) {
                return Err(IdentityStoreError::CorruptRecord);
            }
            Some(value)
        }
        _ => return Err(IdentityStoreError::CorruptRecord),
    };
    Ok(OperationReceipt {
        operation: OperationId::from_bytes(bytes(&row, 0)?)
            .map_err(|_| IdentityStoreError::CorruptRecord)?,
        subject: subject(&row, 1)?,
        epoch,
        kind,
        expected_version,
        committed_version,
        event_seq: positive(integer(&row, 5)?)?,
    })
}

pub(super) fn decode_event(row: Row) -> Result<SecurityEvent, IdentityStoreError> {
    let event = SecurityEvent {
        seq: positive(integer(&row, 0)?)?,
        subject: subject(&row, 1)?,
        version: version(&row, 2)?,
        state: state(&row, 3)?,
        kind: kind(&row, 4)?,
    };
    if !matches!(
        (event.kind, event.state, event.version.get()),
        (ChangeKind::Created, AccountState::Active, 1)
            | (ChangeKind::Disabled, AccountState::Disabled, 2..)
    ) {
        return Err(IdentityStoreError::CorruptRecord);
    }
    Ok(event)
}
