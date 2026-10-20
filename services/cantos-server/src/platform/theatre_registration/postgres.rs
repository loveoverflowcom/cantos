//! Durable registration fences. No transaction contains provider or mail I/O.
use sha2::{Digest, Sha256};
use tokio_postgres::Row;
use uuid::Uuid;

use crate::platform::postgres::Store;

use super::{
    application::{PrepareOutcome, RegistrationFuture, Repository},
    domain::{
        BlockReason, Candidate, CreatedPerson, Email, PendingRegistration, RegistrationError,
        RegistrationPolicy, TokenHash,
    },
};

/// Every operation is pinned to the exact configured issuer namespace. Reconfiguring the
/// issuer cannot consume a challenge or advance an earlier issuer's provisioning attempt.
#[derive(Clone)]
pub struct PostgresRegistrationRepository {
    store: Store,
    namespace: String,
}

impl PostgresRegistrationRepository {
    pub fn new(store: Store, namespace: &str) -> Result<Self, RegistrationError> {
        if namespace.is_empty() || namespace.len() > 2048 || namespace.chars().any(char::is_control)
        {
            return Err(RegistrationError::Configuration);
        }
        Ok(Self {
            store,
            namespace: namespace.to_owned(),
        })
    }

    async fn prepare_inner(
        &self,
        candidate: Candidate,
        policy: RegistrationPolicy,
    ) -> Result<PrepareOutcome, RegistrationError> {
        let mut client = self.store.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        // A single namespace-wide lock makes quota count+receipt insertion atomic across
        // all email addresses, processes and connections. Network/mail work stays outside.
        tx.query_one(
            "SELECT pg_advisory_xact_lock($1)",
            &[&namespace_lock(&self.namespace)],
        )
        .await
        .map_err(unavailable)?;
        let key = candidate.registration.email().canonical_key();
        let existing = tx.query_opt(
            "SELECT id::text,email,phase FROM theatre_registrations WHERE namespace=$1 AND email_key=$2 FOR UPDATE",
            &[&self.namespace, &key],
        ).await.map_err(unavailable)?;
        let registration = if let Some(row) = existing {
            let registration = pending(&row)?;
            let phase: String = row.try_get("phase").map_err(unavailable)?;
            let cooldown = i64::from(policy.resend_seconds());
            // Re-read the clock after any registration-row lock wait. Once Creating has
            // committed, even an expired challenge can never be replaced by public retry.
            let replace: bool = tx.query_one(
                "SELECT last_sent_at<=clock_timestamp()-($3::bigint*interval '1 second') FROM theatre_registrations WHERE namespace=$1 AND id=$2::text::uuid",
                &[&self.namespace, &registration.id().to_string(), &cooldown],
            ).await.map_err(unavailable)?.get(0);
            if !matches!(phase.as_str(), "awaiting_verification" | "cancelled") || !replace {
                tx.commit().await.map_err(unavailable)?;
                return Ok(PrepareOutcome::Suppressed);
            }
            registration
        } else {
            candidate.registration
        };

        let network = candidate.network_key.as_bytes().as_slice();
        let window = i64::from(policy.window_seconds());
        let counts = tx.query_one(
            "WITH cutoff AS MATERIALIZED(SELECT clock_timestamp()-($2::bigint*interval '1 second') AS starts_at) SELECT (SELECT count(*) FROM theatre_registration_mail_starts WHERE namespace=$1 AND accepted_at>cutoff.starts_at),(SELECT count(*) FROM theatre_registration_mail_starts WHERE namespace=$1 AND network_key=$3 AND accepted_at>cutoff.starts_at) FROM cutoff",
            &[&self.namespace, &window, &network],
        ).await.map_err(unavailable)?;
        let global_count: i64 = counts.get(0);
        let network_count: i64 = counts.get(1);
        if global_count >= i64::from(policy.max_requests())
            || network_count >= i64::from(policy.max_network_requests())
        {
            tx.commit().await.map_err(unavailable)?;
            return Ok(PrepareOutcome::Suppressed);
        }

        let id = registration.id().to_string();
        let token = candidate.token_hash.as_bytes().as_slice();
        let ttl = i64::from(policy.verification_seconds());
        tx.execute(
            "WITH clock AS MATERIALIZED(SELECT clock_timestamp() AS now) INSERT INTO theatre_registrations(namespace,id,email,email_key,token_hash,expires_at,last_sent_at,phase) SELECT $1,$2::text::uuid,$3,$4,$5,clock.now+($6::bigint*interval '1 second'),clock.now,'awaiting_verification' FROM clock ON CONFLICT(namespace,email_key) DO UPDATE SET token_hash=EXCLUDED.token_hash,expires_at=EXCLUDED.expires_at,last_sent_at=EXCLUDED.last_sent_at,phase='awaiting_verification'",
            &[&self.namespace, &id, &registration.email().as_str(), &key, &token, &ttl],
        ).await.map_err(unavailable)?;
        tx.execute(
            "INSERT INTO theatre_registration_mail_starts(namespace,token_hash,registration_id,network_key,accepted_at) SELECT namespace,token_hash,id,$3,last_sent_at FROM theatre_registrations WHERE namespace=$1 AND id=$2::text::uuid",
            &[&self.namespace, &id, &network],
        ).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(PrepareOutcome::Deliver(registration))
    }

    async fn advance(
        &self,
        id: Uuid,
        previous: &str,
        next: &str,
    ) -> Result<bool, RegistrationError> {
        let client = self.store.connection().await.map_err(unavailable)?;
        let changed = client
            .execute(
                "UPDATE theatre_registrations SET phase=$4 WHERE namespace=$1 AND id=$2::text::uuid AND phase=$3",
                &[&self.namespace, &id.to_string(), &previous, &next],
            )
            .await
            .map_err(unavailable)?;
        Ok(changed == 1)
    }

    async fn claim_inner(
        &self,
        token_hash: TokenHash,
    ) -> Result<Option<PendingRegistration>, RegistrationError> {
        let mut client = self.store.connection().await.map_err(unavailable)?;
        let tx = client.transaction().await.map_err(unavailable)?;
        let hash = token_hash.as_bytes().as_slice();
        let row = tx
            .query_opt(
                "SELECT id::text,email FROM theatre_registrations WHERE namespace=$1 AND token_hash=$2 FOR UPDATE",
                &[&self.namespace, &hash],
            )
            .await
            .map_err(unavailable)?;
        let Some(row) = row else {
            tx.commit().await.map_err(unavailable)?;
            return Ok(None);
        };
        let registration = pending(&row)?;
        // The clock predicate is evaluated in a statement after the row lock has arrived;
        // waiting behind a resend/cancel cannot extend the proof's lifetime.
        let changed = tx.execute(
            "UPDATE theatre_registrations SET phase='creating' WHERE namespace=$1 AND id=$2::text::uuid AND token_hash=$3 AND phase='awaiting_verification' AND expires_at>clock_timestamp()",
            &[&self.namespace, &registration.id().to_string(), &hash],
        ).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok((changed == 1).then_some(registration))
    }
}

fn unavailable(_: impl std::fmt::Debug) -> RegistrationError {
    RegistrationError::Unavailable
}

fn pending(row: &Row) -> Result<PendingRegistration, RegistrationError> {
    let id: String = row.try_get("id").map_err(unavailable)?;
    let email: String = row.try_get("email").map_err(unavailable)?;
    let id = Uuid::parse_str(&id).map_err(|_| RegistrationError::InvalidState)?;
    let parsed = Email::parse(&email).map_err(|_| RegistrationError::InvalidState)?;
    if parsed.as_str() != email {
        return Err(RegistrationError::InvalidState);
    }
    PendingRegistration::new(id, parsed)
}

/// Hash collisions only serialize unrelated namespaces. Exact C-collated SQL keys are the
/// authority; this lock also serializes budget decisions before any absent-email INSERT.
fn namespace_lock(namespace: &str) -> i64 {
    let mut digest = Sha256::new();
    digest.update(b"cantos-theatre-registration-budget-v1");
    digest.update((namespace.len() as u64).to_be_bytes());
    digest.update(namespace.as_bytes());
    let digest = digest.finalize();
    let mut lock = [0; 8];
    lock.copy_from_slice(&digest[..8]);
    i64::from_be_bytes(lock)
}

fn block_code(reason: BlockReason) -> &'static str {
    match reason {
        BlockReason::CreateRejected => "create_rejected",
        BlockReason::CreateUncertain => "create_uncertain",
        BlockReason::SetupRejected => "setup_rejected",
        BlockReason::SetupUncertain => "setup_uncertain",
        BlockReason::SetupDeliveryRejected => "setup_delivery_rejected",
        BlockReason::SetupDeliveryUncertain => "setup_delivery_uncertain",
        BlockReason::BindingMismatch => "binding_mismatch",
    }
}

impl Repository for PostgresRegistrationRepository {
    fn prepare(
        &self,
        candidate: Candidate,
        policy: RegistrationPolicy,
    ) -> RegistrationFuture<'_, PrepareOutcome> {
        Box::pin(self.prepare_inner(candidate, policy))
    }

    fn claim(&self, token_hash: TokenHash) -> RegistrationFuture<'_, Option<PendingRegistration>> {
        Box::pin(self.claim_inner(token_hash))
    }

    fn cancel(&self, token_hash: TokenHash) -> RegistrationFuture<'_, ()> {
        Box::pin(async move {
            let hash = token_hash.as_bytes().as_slice();
            self.store.connection().await.map_err(unavailable)?.execute(
                "UPDATE theatre_registrations SET phase='cancelled' WHERE namespace=$1 AND token_hash=$2 AND phase='awaiting_verification'",
                &[&self.namespace, &hash],
            ).await.map_err(unavailable)?;
            Ok(())
        })
    }

    fn created(&self, id: Uuid, person: CreatedPerson) -> RegistrationFuture<'_, bool> {
        Box::pin(async move {
            if person.registration_id() != id || person.subject() != id {
                return Ok(false);
            }
            let client = self.store.connection().await.map_err(unavailable)?;
            let changed = client.execute(
                "UPDATE theatre_registrations SET phase='created',person_id=$3::text::uuid WHERE namespace=$1 AND id=$2::text::uuid AND phase='creating'",
                &[&self.namespace, &id.to_string(), &person.subject().to_string()],
            ).await.map_err(unavailable)?;
            Ok(changed == 1)
        })
    }

    fn begin_setup(&self, id: Uuid) -> RegistrationFuture<'_, bool> {
        Box::pin(self.advance(id, "created", "setup_started"))
    }

    fn begin_delivery(&self, id: Uuid) -> RegistrationFuture<'_, bool> {
        Box::pin(self.advance(id, "setup_started", "delivering"))
    }

    fn complete(&self, id: Uuid) -> RegistrationFuture<'_, bool> {
        Box::pin(self.advance(id, "delivering", "complete"))
    }

    fn block(&self, id: Uuid, reason: BlockReason) -> RegistrationFuture<'_, ()> {
        Box::pin(async move {
            self.store.connection().await.map_err(unavailable)?.execute(
                "UPDATE theatre_registrations SET phase='blocked',block_reason=$3 WHERE namespace=$1 AND id=$2::text::uuid AND phase IN ('creating','created','setup_started','delivering')",
                &[&self.namespace, &id.to_string(), &block_code(reason)],
            ).await.map_err(unavailable)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
