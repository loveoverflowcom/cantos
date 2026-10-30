//! One authoritative statement joins current security state to the proof-created mailbox.
//! Existing v3 rows already bind verified email immutably to a subject; no new schema is needed.
use super::{IdentityStore, IdentityStoreError};
use crate::domain::{
    email::EmailAddress,
    ids::{InstanceEpoch, SubjectId},
    security_version::SecurityVersion,
};
use cantos_store_turso::Value;

impl IdentityStore {
    pub async fn verified_mailbox(
        &self,
        subject: SubjectId,
        version: SecurityVersion,
        epoch: InstanceEpoch,
    ) -> Result<Option<EmailAddress>, IdentityStoreError> {
        if epoch != self.epoch() {
            return Ok(None);
        }
        let reader = self.database.reader().await?;
        let mut rows = reader.query("SELECT e.email FROM identity_subjects s JOIN identity_emails e ON e.subject_id=s.subject_id WHERE s.subject_id=?1 AND s.state=1 AND s.security_version=?2 AND e.verified_at>=0",
            cantos_store_turso::params_from_iter([Value::Blob(subject.to_bytes().to_vec()),Value::Integer(version.get() as i64)])).await?;
        let Some(row) = rows.next().await? else {
            return Ok(None);
        };
        let Value::Text(text) = row
            .get_value(0)
            .map_err(|_| IdentityStoreError::CorruptRecord)?
        else {
            return Err(IdentityStoreError::CorruptRecord);
        };
        let email = EmailAddress::parse(&text).map_err(|_| IdentityStoreError::CorruptRecord)?;
        if email.as_str() != text {
            return Err(IdentityStoreError::CorruptRecord);
        }
        Ok(Some(email))
    }
}
