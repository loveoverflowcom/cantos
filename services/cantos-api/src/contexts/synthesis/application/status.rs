//! Read a job only after authenticating and authorizing its current script scope.
use super::super::postgres::jobs::{run, summary};
use crate::{
    contexts::{
        production::production_actor as durable_actor,
        synthesis::{DurableError, JobSummary},
    },
    platform::postgres::{normalized_uuid, Store},
};

impl Store {
    pub async fn job_status(&self, token: &str, id: &str) -> Result<JobSummary, DurableError> {
        let id = normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = durable_actor(&tx, token).await?;
        let r = run(&tx, &id).await?;
        // Status is readable after approval expires; ownership still checked on every read.
        crate::contexts::editorial::authorize(
            &tx,
            &actor,
            &r.get::<_, String>("script_id"),
            crate::contexts::editorial::Action::Review,
            crate::platform::postgres::LockMode::Share,
        )
        .await?;
        // An attached view answers only after the checks above, for this authorized run.
        #[cfg(feature = "dbsp-experimental")]
        if let Some(view) = &self.job_summaries {
            let result = view.summary(&tx, &id).await?;
            tx.commit().await?;
            return Ok(result);
        }
        let result = summary(&tx, &id).await?;
        tx.commit().await?;
        Ok(result)
    }
}
