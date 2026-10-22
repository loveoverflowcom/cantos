//! Durable Firebase enrollment fences; provider calls never run inside a transaction.
use sha2::{Digest, Sha256};
use crate::platform::store::Row;
use uuid::Uuid;

use crate::platform::store::Store;

use super::{
    application::{EnrollmentFuture, PrepareOutcome, Repository},
    domain::{Enrollment, FailureStage, Phase},
};
use crate::platform::theatre_registration::domain::{
    CreatedPerson, Email, NetworkKey, PendingRegistration, RegistrationError, RegistrationPolicy,
};
use crate::platform::theatre_registration::firebase_action::FirebaseProjectId;

#[derive(Clone)]
pub struct PostgresFirebaseEnrollmentRepository {
    store: Store,
    namespace: String,
    project: String,
}

impl PostgresFirebaseEnrollmentRepository {
    pub fn new(store: Store, namespace: &str, project: &str) -> Result<Self, RegistrationError> {
        FirebaseProjectId::parse(project).map_err(|_| RegistrationError::Configuration)?;
        if namespace.is_empty() || namespace.len() > 2048 || namespace.chars().any(char::is_control)
        {
            return Err(RegistrationError::Configuration);
        }
        Ok(Self {
            store,
            namespace: namespace.to_owned(),
            project: project.to_owned(),
        })
    }

    async fn prepare_inner(
        &self,
        candidate: Enrollment,
        network: NetworkKey,
        policy: RegistrationPolicy,
    ) -> Result<PrepareOutcome, RegistrationError> {
        if candidate.project() != self.project {
            return Err(RegistrationError::InvalidState);
        }
        let mut client = self.store.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        // The namespace-wide quota lock also serializes absent-email inserts. Exact
        // database keys, rather than the advisory hash, remain the binding authority.
        tx.query_one(
            "SELECT pg_advisory_xact_lock($1)",
            &[&namespace_lock(&self.namespace)],
        )
        .await
        .map_err(unavailable)?;
        let key = candidate.registration().email().canonical_key();
        let row = tx.query_opt(
            "SELECT id::text,email,project,uid,phase FROM theatre_firebase_enrollments WHERE namespace=$1 AND project=$2 AND email_key=$3 AND phase NOT IN ('cancelled','expired') FOR UPDATE",
            &[&self.namespace, &self.project, &key],
        ).await.map_err(unavailable)?;
        let existing = if let Some(row) = row {
            let enrollment = enrollment(&row)?;
            let phase: String = row.try_get("phase").map_err(corrupt)?;
            let timing = tx.query_one(
                "SELECT expires_at>clock_timestamp(),last_sent_at<=clock_timestamp()-($3::bigint*interval '1 second') FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
                &[&self.namespace, &enrollment.registration().id().to_string(), &i64::from(policy.resend_seconds())],
            ).await.map_err(unavailable)?;
            let fresh: bool = timing.get(0);
            let cooldown_elapsed: bool = timing.get(1);
            if phase == "awaiting_action" && !fresh {
                tx.execute(
                    "UPDATE theatre_firebase_enrollments SET phase='expired' WHERE namespace=$1 AND id=$2::text::uuid AND phase='awaiting_action'",
                    &[&self.namespace, &enrollment.registration().id().to_string()],
                ).await.map_err(database)?;
                None
            } else if phase == "awaiting_action" && fresh && cooldown_elapsed {
                Some(enrollment)
            } else {
                tx.commit().await.map_err(unavailable)?;
                return Ok(PrepareOutcome::Suppressed);
            }
        } else {
            None
        };

        let network_key = network.as_bytes().as_slice();
        let counts = tx.query_one(
            "WITH cutoff AS MATERIALIZED(SELECT clock_timestamp()-($2::bigint*interval '1 second') AS starts_at) SELECT (SELECT count(*) FROM theatre_firebase_send_starts WHERE namespace=$1 AND accepted_at>cutoff.starts_at),(SELECT count(*) FROM theatre_firebase_send_starts WHERE namespace=$1 AND network_key=$3 AND accepted_at>cutoff.starts_at) FROM cutoff",
            &[&self.namespace, &i64::from(policy.window_seconds()), &network_key],
        ).await.map_err(unavailable)?;
        if counts.get::<_, i64>(0) >= i64::from(policy.max_requests())
            || counts.get::<_, i64>(1) >= i64::from(policy.max_network_requests())
        {
            tx.commit().await.map_err(unavailable)?;
            return Ok(PrepareOutcome::Suppressed);
        }

        let outcome = if let Some(enrollment) = existing {
            let changed = tx.execute(
                "UPDATE theatre_firebase_enrollments SET phase='send_started',last_sent_at=clock_timestamp() WHERE namespace=$1 AND id=$2::text::uuid AND phase='awaiting_action' AND expires_at>clock_timestamp()",
                &[&self.namespace, &enrollment.registration().id().to_string()],
            ).await.map_err(database)?;
            if changed != 1 {
                tx.commit().await.map_err(unavailable)?;
                return Ok(PrepareOutcome::Suppressed);
            }
            PrepareOutcome::Resend(enrollment)
        } else {
            tx.execute(
                "WITH clock AS MATERIALIZED(SELECT clock_timestamp() AS now) INSERT INTO theatre_firebase_enrollments(namespace,id,project,uid,email,email_key,accepted_at,expires_at,last_sent_at,phase) SELECT $1,$2::text::uuid,$3,$4,$5,$6,clock.now,clock.now+($7::bigint*interval '1 second'),clock.now,'bootstrap_started' FROM clock",
                &[&self.namespace, &candidate.registration().id().to_string(), &self.project, &candidate.uid(), &candidate.registration().email().as_str(), &key, &i64::from(policy.verification_seconds())],
            ).await.map_err(database)?;
            PrepareOutcome::Bootstrap(candidate)
        };
        let id = match &outcome {
            PrepareOutcome::Bootstrap(enrollment) | PrepareOutcome::Resend(enrollment) => {
                enrollment.registration().id()
            }
            PrepareOutcome::Suppressed => return Err(RegistrationError::InvalidState),
        };
        tx.execute(
            "INSERT INTO theatre_firebase_send_starts(namespace,enrollment_id,network_key,accepted_at) SELECT namespace,id,$3,last_sent_at FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
            &[&self.namespace, &id.to_string(), &network_key],
        ).await.map_err(database)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(outcome)
    }

    async fn advance(
        &self,
        id: Uuid,
        previous: &str,
        next: &str,
        require_fresh: bool,
    ) -> Result<bool, RegistrationError> {
        let mut client = self.store.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        // A predicate evaluated before an UPDATE's lock wait could admit an expired
        // operation. Acquire the row first and read the database clock afterward.
        tx.query_opt(
            "SELECT id FROM theatre_firebase_enrollments WHERE namespace=$1 AND project=$2 AND id=$3::text::uuid FOR UPDATE",
            &[&self.namespace, &self.project, &id.to_string()],
        ).await.map_err(database)?;
        let changed = tx.execute(
            "UPDATE theatre_firebase_enrollments SET phase=$5 WHERE namespace=$1 AND project=$2 AND id=$3::text::uuid AND phase=$4 AND (NOT $6 OR expires_at>clock_timestamp())",
            &[&self.namespace, &self.project, &id.to_string(), &previous, &next, &require_fresh],
        ).await.map_err(database)?;
        // Receipt triggers and index/constraint work can wait after the UPDATE's
        // predicate. Sample time again after that work before granting the next
        // external effect or releasing setup, and reject the entire transition.
        if changed == 1 && require_fresh {
            let fresh: bool = tx.query_one(
                "SELECT expires_at>clock_timestamp() FROM theatre_firebase_enrollments WHERE namespace=$1 AND project=$2 AND id=$3::text::uuid",
                &[&self.namespace, &self.project, &id.to_string()],
            ).await.map_err(database)?.get(0);
            if !fresh {
                tx.rollback().await.map_err(unavailable)?;
                return Ok(false);
            }
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(changed == 1)
    }

    async fn claim_inner(&self, id: Uuid) -> Result<Option<Enrollment>, RegistrationError> {
        let mut client = self.store.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        let row = tx.query_opt(
            "SELECT id::text,email,project,uid FROM theatre_firebase_enrollments WHERE namespace=$1 AND project=$2 AND id=$3::text::uuid FOR UPDATE",
            &[&self.namespace, &self.project, &id.to_string()],
        ).await.map_err(unavailable)?;
        let Some(row) = row else {
            tx.commit().await.map_err(unavailable)?;
            return Ok(None);
        };
        let enrollment = enrollment(&row)?;
        let changed = tx.execute(
            "UPDATE theatre_firebase_enrollments SET phase='inspecting' WHERE namespace=$1 AND id=$2::text::uuid AND phase='awaiting_action' AND expires_at>clock_timestamp()",
            &[&self.namespace, &id.to_string()],
        ).await.map_err(database)?;
        tx.commit().await.map_err(unavailable)?;
        Ok((changed == 1).then_some(enrollment))
    }
}

fn unavailable(_: impl std::fmt::Debug) -> RegistrationError {
    RegistrationError::Unavailable
}

fn database(error: crate::platform::store::Error) -> RegistrationError {
    match error.code() {
        Some(code) if *code == tokio_postgres::error::SqlState::UNIQUE_VIOLATION => {
            RegistrationError::Conflict
        }
        Some(code)
            if *code == tokio_postgres::error::SqlState::CHECK_VIOLATION
                || *code == tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION =>
        {
            RegistrationError::InvalidState
        }
        _ => RegistrationError::Unavailable,
    }
}

fn corrupt(_: impl std::fmt::Debug) -> RegistrationError {
    RegistrationError::InvalidState
}

fn enrollment(row: &Row) -> Result<Enrollment, RegistrationError> {
    let id: String = row.try_get("id").map_err(corrupt)?;
    let email: String = row.try_get("email").map_err(corrupt)?;
    let project: String = row.try_get("project").map_err(corrupt)?;
    let uid: String = row.try_get("uid").map_err(corrupt)?;
    let id = Uuid::parse_str(&id).map_err(corrupt)?;
    let parsed = Email::parse(&email).map_err(corrupt)?;
    if parsed.as_str() != email {
        return Err(RegistrationError::InvalidState);
    }
    let enrollment =
        Enrollment::new(PendingRegistration::new(id, parsed)?, &project).map_err(corrupt)?;
    if enrollment.uid() != uid {
        return Err(RegistrationError::InvalidState);
    }
    Ok(enrollment)
}

fn namespace_lock(namespace: &str) -> i64 {
    let mut digest = Sha256::new();
    digest.update(b"cantos-theatre-firebase-enrollment-budget-v1");
    digest.update((namespace.len() as u64).to_be_bytes());
    digest.update(namespace.as_bytes());
    let digest = digest.finalize();
    let mut lock = [0; 8];
    lock.copy_from_slice(&digest[..8]);
    i64::from_be_bytes(lock)
}

impl Repository for PostgresFirebaseEnrollmentRepository {
    fn prepare(
        &self,
        candidate: Enrollment,
        network: NetworkKey,
        policy: RegistrationPolicy,
    ) -> EnrollmentFuture<'_, PrepareOutcome> {
        Box::pin(self.prepare_inner(candidate, network, policy))
    }

    fn dispatchable(&self, id: Uuid, phase: Phase) -> EnrollmentFuture<'_, bool> {
        Box::pin(async move {
            let expected = match phase {
                Phase::BootstrapStarted => "bootstrap_started",
                Phase::SendStarted => "send_started",
                _ => return Err(RegistrationError::InvalidState),
            };
            let mut client = self.store.connection().await.map_err(unavailable)?;
            let tx = client.transaction().await.map_err(unavailable)?;
            tx.query_opt(
                "SELECT id FROM theatre_firebase_enrollments WHERE namespace=$1 AND project=$2 AND id=$3::text::uuid FOR UPDATE",
                &[&self.namespace, &self.project, &id.to_string()],
            ).await.map_err(unavailable)?;
            let row = tx.query_opt(
                "SELECT phase=$4 AND expires_at>clock_timestamp() FROM theatre_firebase_enrollments WHERE namespace=$1 AND project=$2 AND id=$3::text::uuid",
                &[&self.namespace, &self.project, &id.to_string(), &expected],
            ).await.map_err(unavailable)?;
            let allowed = row.map(|row| row.get(0)).unwrap_or(false);
            tx.commit().await.map_err(unavailable)?;
            Ok(allowed)
        })
    }

    fn ready(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(self.advance(id, "bootstrap_started", "ready", false))
    }

    fn begin_send(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(self.advance(id, "ready", "send_started", true))
    }

    fn begin_delivery(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(self.advance(id, "send_started", "mail_started", true))
    }

    fn sent(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(self.advance(id, "mail_started", "awaiting_action", true))
    }

    fn claim_action(&self, id: Uuid) -> EnrollmentFuture<'_, Option<Enrollment>> {
        Box::pin(self.claim_inner(id))
    }

    fn release_inspection(&self, id: Uuid) -> EnrollmentFuture<'_, ()> {
        Box::pin(async move {
            let mut client = self.store.connection().await.map_err(unavailable)?;
            let tx = client.transaction().await.map_err(unavailable)?;
            tx.query_opt(
                "SELECT id FROM theatre_firebase_enrollments WHERE namespace=$1 AND project=$2 AND id=$3::text::uuid FOR UPDATE",
                &[&self.namespace, &self.project, &id.to_string()],
            ).await.map_err(unavailable)?;
            tx.execute(
                "UPDATE theatre_firebase_enrollments SET phase=CASE WHEN expires_at>clock_timestamp() THEN 'awaiting_action' ELSE 'expired' END WHERE namespace=$1 AND project=$2 AND id=$3::text::uuid AND phase='inspecting'",
                &[&self.namespace, &self.project, &id.to_string()],
            ).await.map_err(database)?;
            tx.commit().await.map_err(unavailable)?;
            Ok(())
        })
    }

    fn begin_consume(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(self.advance(id, "inspecting", "consuming", true))
    }

    fn begin_create(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(self.advance(id, "consuming", "creating", true))
    }

    fn cancel_consumed(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(self.advance(id, "consuming", "cancelled", true))
    }

    fn created(&self, id: Uuid, person: CreatedPerson) -> EnrollmentFuture<'_, bool> {
        Box::pin(async move {
            if person.registration_id() != id || person.subject() != id {
                return Ok(false);
            }
            // Preserve a positively acknowledged create even after expiry. The next
            // setup reservation separately checks freshness and cannot replay create.
            let changed = self.store.connection().await.map_err(unavailable)?.execute(
                "UPDATE theatre_firebase_enrollments SET phase='created',person_id=$4::text::uuid WHERE namespace=$1 AND project=$2 AND id=$3::text::uuid AND phase='creating'",
                &[&self.namespace, &self.project, &id.to_string(), &person.subject().to_string()],
            ).await.map_err(database)?;
            Ok(changed == 1)
        })
    }

    fn begin_setup(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(self.advance(id, "created", "setup_started", true))
    }

    fn begin_handoff(&self, id: Uuid) -> EnrollmentFuture<'_, bool> {
        Box::pin(self.advance(id, "setup_started", "handed_off", true))
    }

    fn block(&self, id: Uuid, stage: FailureStage) -> EnrollmentFuture<'_, ()> {
        Box::pin(async move {
            self.store.connection().await.map_err(unavailable)?.execute(
                "UPDATE theatre_firebase_enrollments SET phase='blocked',block_reason=$4 WHERE namespace=$1 AND project=$2 AND id=$3::text::uuid AND phase IN ('bootstrap_started','ready','send_started','mail_started','inspecting','consuming','creating','created','setup_started')",
                &[&self.namespace, &self.project, &id.to_string(), &stage.as_code()],
            ).await.map_err(database)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
