//! Authorize one committed send boundary before any provider invocation.
//! A prior send is checked before authority/fence errors so a duplicate cannot attest NotSent.
use super::super::postgres::{attempts, authority, check_lease, mode, run};
use crate::{
    contexts::synthesis::{
        jobs::input_fingerprint, receipt::decode_receipt, AttemptWork, DurableError,
    },
    platform::postgres::Store,
};

impl Store {
    /// Commit one send boundary; duplicate callers must not send.
    pub async fn authorize_dispatch(&self, work: &AttemptWork) -> Result<(), DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let already_sent = attempts::has_dispatch(&tx, &work.intent.attempt_id).await?;
        if already_sent {
            return Err(DurableError::Conflict);
        }
        let r = run(&tx, &work.lease.run_id).await?;
        let doc = authority(&tx, &r).await?;
        let step = check_lease(&tx, &work.lease).await?;
        let intent_bytes = attempts::intent_for_dispatch(
            &tx,
            &work.intent.attempt_id,
            &work.lease.step_id,
            work.lease.token,
        )
        .await?;
        let intent = decode_receipt(&intent_bytes).map_err(|_| DurableError::Corrupt)?;
        let line = doc
            .resolved
            .iter()
            .find(|line| line.dialogue_id == intent.dialogue_id)
            .ok_or(DurableError::Corrupt)?;
        if intent != work.intent
            || mode(&r)? != work.mode
            || input_fingerprint(line).map_err(|_| DurableError::Corrupt)?
                != step.get::<_, String>("input_fingerprint")
            || line != &work.line
        {
            return Err(DurableError::Conflict);
        }
        let known = attempts::has_observation_or_resolution(&tx, &intent.attempt_id).await?;
        if known {
            return Err(DurableError::Conflict);
        }
        let n = attempts::record_dispatch(&tx, &intent.attempt_id, work.lease.token).await?;
        if n != 1 {
            return Err(DurableError::Conflict);
        }
        authority(&tx, &r).await?;
        check_lease(&tx, &work.lease).await?;
        tx.commit().await?;
        Ok(())
    }
}
