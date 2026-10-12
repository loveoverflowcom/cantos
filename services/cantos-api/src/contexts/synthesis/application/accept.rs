//! Admit recorded output under the current authority and lease, then select it once together
//! with its verified clip. No transaction spans synthesis or file I/O.
use super::super::postgres::{
    acceptance::{self, SelectedOutput},
    authority, check_lease, run, stop_job,
};
use crate::contexts::synthesis::{
    jobs::input_fingerprint,
    receipt::{audit_receipt, ReceiptOutcome},
    DurableError, JobLease,
};
use crate::platform::postgres::Store;

impl Store {
    pub async fn quarantine_recorded_output(
        &self,
        lease: &JobLease,
        invalid: bool,
    ) -> Result<(), DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        check_lease(&tx, lease).await?;
        if !acceptance::has_recorded_output(&tx, &lease.step_id).await? {
            return Err(DurableError::Conflict);
        }
        let reason = if invalid {
            "recorded_output_invalid"
        } else {
            "recorded_output_unavailable"
        };
        stop_job(&tx, &lease.job_id, "paused", reason).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn accept_step(
        &self,
        lease: &JobLease,
        attempt: &str,
        audio: &[u8],
    ) -> Result<(), DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let r = run(&tx, &lease.run_id).await?;
        let doc = authority(&tx, &r).await?;
        let step = check_lease(&tx, lease).await?;
        if acceptance::resolved(&tx, attempt).await? {
            return Err(DurableError::ReconciliationRequired);
        }
        let observation = acceptance::observed_attempt(&tx, attempt, &lease.step_id).await?;
        let receipt = &observation.receipt;
        if !matches!(receipt.outcome, ReceiptOutcome::Output { .. })
            || receipt.attempt_id != attempt
            || receipt.operation_id != observation.intent.operation_id
            || receipt.dialogue_id != step.get::<_, String>("dialogue_id")
        {
            return Err(DurableError::Conflict);
        }
        let line = doc
            .resolved
            .iter()
            .find(|line| line.dialogue_id == receipt.dialogue_id)
            .ok_or(DurableError::Corrupt)?;
        if input_fingerprint(line).map_err(|_| DurableError::Corrupt)?
            != step.get::<_, String>("input_fingerprint")
        {
            return Err(DurableError::Corrupt);
        }
        audit_receipt(&doc, receipt, Some(audio)).map_err(|_| DurableError::Corrupt)?;
        acceptance::select_output(
            &tx,
            SelectedOutput {
                lease,
                attempt,
                run: &r,
                speech_fingerprint: step.get("speech_fingerprint"),
                owner: &doc.owner_id,
                line,
                receipt,
                audio,
                output_path: observation.output_path,
            },
        )
        .await?;
        if !acceptance::pending_steps(&tx, &lease.job_id).await? {
            acceptance::complete_job(&tx, &lease.job_id).await?;
        }
        authority(&tx, &r).await?;
        tx.commit().await?;
        Ok(())
    }
}
