//! Compiled only for the disposable recovery fixture, never service-only OCI builds.
use super::{rows, IdentityStore, IdentityStoreError};
use crate::domain::{
    admission::AccountState,
    ids::{OperationId, SubjectId},
};

impl IdentityStore {
    /// Finite test actor: only the pre-existing synthetic account. No credential/hash read,
    /// new account, general administration API or caller-selected mailbox is supplied.
    pub async fn disable_disposable_recovery_account(&self) -> Result<(), IdentityStoreError> {
        let reader = self.database.reader().await?;
        let mut rows = reader.query("SELECT subject_id FROM identity_emails WHERE email='recovery-disabled@example.test'", ()).await?;
        let row = rows
            .next()
            .await?
            .ok_or(IdentityStoreError::SubjectMissing)?;
        let subject = SubjectId::from_bytes(rows::bytes(&row, 0)?)
            .map_err(|_| IdentityStoreError::CorruptRecord)?;
        drop(rows);
        drop(reader);
        let current = self
            .subject(subject)
            .await?
            .ok_or(IdentityStoreError::SubjectMissing)?;
        if current.state == AccountState::Disabled {
            return Ok(());
        }
        self.disable_subject(
            OperationId::from_bytes([0xd1; 16]).map_err(|_| IdentityStoreError::CorruptRecord)?,
            subject,
            current.version,
            self.epoch(),
        )
        .await?;
        Ok(())
    }
}
