//! Admission-only characterization, not new security requirements. The actual RPC decoder,
//! `IdentityAuth::authenticate`/`confirm`, decision and Turso adapter run together. Only the
//! Identity RPC peer is scripted; a forwarding SessionStore probe records actual revoke results.
//! Fixed facts/clock and named SQL aborts make schedules explicit. No Quint expectations enter.
use super::*;
use crate::platform::identity_auth::session::{Issued, Rotated, SessionRecord};
use std::sync::Mutex;

const FAIL_REVOKE: &str = "CREATE TRIGGER pilot_fail_revoke AFTER UPDATE ON identity_sessions \
    WHEN NEW.revoked = 1 BEGIN SELECT RAISE(ABORT, 'cantos:test.pilot.revoke'); END";

type Change = fn(&mut Script);

// A different subject is deliberately absent: the real RPC parser refuses it first.
const SECURITY_CHANGES: [(&str, Change, RevokeReason, &str); 5] = [
    (
        "disabled",
        |s| s.active = false,
        RevokeReason::Disabled,
        "disabled",
    ),
    (
        "advanced",
        |s| s.version = 4,
        RevokeReason::VersionAdvanced,
        "version-advanced",
    ),
    (
        "regressed",
        |s| s.version = 2,
        RevokeReason::VersionRegressed,
        "version-regressed",
    ),
    (
        "epoch",
        |s| s.epoch = [0x33; 16],
        RevokeReason::EpochChanged,
        "epoch-changed",
    ),
    (
        "missing",
        |s| s.state_not_found = true,
        RevokeReason::SubjectMissing,
        "subject-missing",
    ),
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct RevokeAttempt {
    session_id: String,
    reason: RevokeReason,
    result: Result<(), SessionError>,
}

/// Records the effect boundary; never implements session state or a reference reducer.
struct RevokeProbe<'a> {
    inner: &'a TursoSessions,
    attempts: Mutex<Vec<RevokeAttempt>>,
}

impl<'a> RevokeProbe<'a> {
    fn new(inner: &'a TursoSessions) -> Self {
        Self {
            inner,
            attempts: Mutex::new(Vec::new()),
        }
    }

    fn observed(&self) -> Vec<RevokeAttempt> {
        self.attempts.lock().unwrap().clone()
    }
}

impl SessionStore for RevokeProbe<'_> {
    fn bind(
        &self,
        subject: Subject,
    ) -> impl std::future::Future<Output = Result<String, SessionError>> + Send {
        self.inner.bind(subject)
    }

    fn issue(
        &self,
        channel: Channel,
        observed: &ObservedSecurity,
        authenticated_at: u64,
        secrets: &Secrets,
    ) -> impl std::future::Future<Output = Result<Issued, SessionError>> + Send {
        self.inner
            .issue(channel, observed, authenticated_at, secrets)
    }

    fn access(
        &self,
        token: &str,
        channel: Channel,
    ) -> impl std::future::Future<Output = Result<SessionRecord, SessionError>> + Send {
        self.inner.access(token, channel)
    }

    fn session_id_of(
        &self,
        token: &str,
        channel: Channel,
    ) -> impl std::future::Future<Output = Result<Option<String>, SessionError>> + Send {
        self.inner.session_id_of(token, channel)
    }

    async fn revoke(&self, session_id: &str, reason: RevokeReason) -> Result<(), SessionError> {
        let result = self.inner.revoke(session_id, reason).await;
        self.attempts.lock().unwrap().push(RevokeAttempt {
            session_id: session_id.to_owned(),
            reason,
            result,
        });
        result
    }

    fn logout(
        &self,
        token: &str,
        channel: Channel,
    ) -> impl std::future::Future<Output = Result<(), SessionError>> + Send {
        self.inner.logout(token, channel)
    }

    fn revoke_refresh(
        &self,
        refresh: &str,
        session_id: &str,
    ) -> impl std::future::Future<Output = Result<(), SessionError>> + Send {
        self.inner.revoke_refresh(refresh, session_id)
    }

    fn peek_refresh(
        &self,
        refresh: &str,
        session_id: &str,
    ) -> impl std::future::Future<Output = Result<SessionRecord, SessionError>> + Send {
        self.inner.peek_refresh(refresh, session_id)
    }

    fn rotate(
        &self,
        refresh: &str,
        session_id: &str,
        secrets: &Secrets,
    ) -> impl std::future::Future<Output = Result<Rotated, SessionError>> + Send {
        self.inner.rotate(refresh, session_id, secrets)
    }

    fn renew_web(
        &self,
        token: &str,
        channel: Channel,
    ) -> impl std::future::Future<Output = Result<(SessionRecord, Lifetime), SessionError>> + Send
    {
        self.inner.renew_web(token, channel)
    }
}

async fn seeded() -> (Scripted, Secrets, Issued) {
    let h = Scripted::with_clock(FixedClock::new(T0)).await;
    let secrets = Secrets {
        access: format!("ci1_n_{}", "a".repeat(64)),
        refresh: Some(format!("ci1_r_{}", "b".repeat(64))),
    };
    // Issue via the real adapter. Login/fact freshness is outside this admission-only seed.
    let observed = ObservedSecurity {
        subject: Subject::from_bytes([0x11; 16]).unwrap(),
        epoch: Epoch::from_bytes([0x22; 16]).unwrap(),
        version: 3,
    };
    let family = h
        .sessions
        .issue(NATIVE, &observed, S0, &secrets)
        .await
        .unwrap();
    (h, secrets, family)
}

async fn admit(
    h: &Scripted,
    probe: &RevokeProbe<'_>,
    secrets: &Secrets,
) -> Result<(), AuthFailure> {
    h.auth
        .authenticate(probe, &secrets.access, NATIVE)
        .await
        .map(|_| ())
}

// Observe every stored column independently of the production row decoder and the probe.
async fn snapshot(h: &Scripted) -> [Vec<Vec<Value>>; 4] {
    [
        rows(&h.database, "SELECT * FROM actors ORDER BY id").await,
        rows(
            &h.database,
            "SELECT * FROM actor_external_identities ORDER BY issuer,subject",
        )
        .await,
        rows(&h.database, "SELECT * FROM identity_sessions ORDER BY id").await,
        rows(
            &h.database,
            "SELECT * FROM identity_session_tokens ORDER BY session_id,access_hash",
        )
        .await,
    ]
}

fn attempt(
    family: &Issued,
    reason: RevokeReason,
    result: Result<(), SessionError>,
) -> RevokeAttempt {
    RevokeAttempt {
        session_id: family.session_id.clone(),
        reason,
        result,
    }
}

#[tokio::test]
async fn committed_revoke_survives_matching_identity_and_stops_before_rpc() {
    for (label, change, reason, stored_reason) in SECURITY_CHANGES {
        let (h, secrets, family) = seeded().await;
        let probe = RevokeProbe::new(&h.sessions);
        assert_eq!(admit(&h, &probe, &secrets).await, Ok(()), "{label}");
        assert_eq!(h.fake.state_calls(), 1);
        h.fake.set(change);
        assert_eq!(
            admit(&h, &probe, &secrets).await,
            Err(AuthFailure::Unauthenticated),
            "{label}"
        );
        assert_eq!(
            probe.observed(),
            [attempt(&family, reason, Ok(()))],
            "{label}"
        );
        assert_eq!(
            revoked_with(&session_row(&h.database, &family.session_id).await),
            (1, Some(stored_reason.into())),
            "{label}"
        );
        h.fake.set(|s| {
            *s = Script {
                state_calls: s.state_calls,
                ..Script::healthy()
            }
        });
        let committed = snapshot(&h).await;
        assert_eq!(
            admit(&h, &probe, &secrets).await,
            Err(AuthFailure::Unauthenticated),
            "{label}"
        );
        assert_eq!(
            h.fake.state_calls(),
            2,
            "committed revocation refuses before Identity: {label}"
        );
        assert_eq!(snapshot(&h).await, committed, "{label}");
        assert_eq!(probe.observed().len(), 1, "{label}");
    }
}

#[tokio::test]
async fn failed_revoke_is_redone_when_the_next_identity_read_still_differs() {
    for (label, change, reason, stored_reason) in SECURITY_CHANGES {
        let (h, secrets, family) = seeded().await;
        let probe = RevokeProbe::new(&h.sessions);
        let before = snapshot(&h).await;
        execute(&h.database, FAIL_REVOKE).await;
        h.fake.set(change);
        assert_eq!(
            admit(&h, &probe, &secrets).await,
            Err(AuthFailure::Unauthenticated),
            "{label}"
        );
        assert_eq!(
            probe.observed(),
            [attempt(&family, reason, Err(SessionError::Unavailable))],
            "{label}"
        );
        assert_eq!(
            snapshot(&h).await,
            before,
            "the real failed transaction rolls back: {label}"
        );
        execute(&h.database, "DROP TRIGGER pilot_fail_revoke").await;
        assert_eq!(
            admit(&h, &probe, &secrets).await,
            Err(AuthFailure::Unauthenticated),
            "{label}"
        );
        assert_eq!(
            probe.observed(),
            [
                attempt(&family, reason, Err(SessionError::Unavailable)),
                attempt(&family, reason, Ok(()))
            ],
            "{label}"
        );
        assert_eq!(
            revoked_with(&session_row(&h.database, &family.session_id).await),
            (1, Some(stored_reason.into())),
            "{label}"
        );
        assert_eq!(
            h.fake.state_calls(),
            2,
            "one fresh read on each live admission: {label}"
        );
    }
}

#[tokio::test]
async fn matching_identity_after_failed_revoke_reproduces_the_pending_contract_gap() {
    for (label, change, reason, _) in SECURITY_CHANGES {
        let (h, secrets, family) = seeded().await;
        let probe = RevokeProbe::new(&h.sessions);
        let before = snapshot(&h).await;
        execute(&h.database, FAIL_REVOKE).await;
        h.fake.set(change);
        assert_eq!(
            admit(&h, &probe, &secrets).await,
            Err(AuthFailure::Unauthenticated),
            "{label}"
        );
        assert_eq!(snapshot(&h).await, before, "{label}");
        execute(&h.database, "DROP TRIGGER pilot_fail_revoke").await;
        h.fake.set(|s| {
            *s = Script {
                state_calls: s.state_calls,
                ..Script::healthy()
            }
        });
        // Characterizes HEAD; it does not approve re-admission or narrow the owning contract.
        assert_eq!(admit(&h, &probe, &secrets).await, Ok(()), "{label}");
        assert_eq!(
            probe.observed(),
            [attempt(&family, reason, Err(SessionError::Unavailable))],
            "no durable retry obligation: {label}"
        );
        assert_eq!(snapshot(&h).await, before, "no revoke committed: {label}");
        assert_eq!(h.fake.state_calls(), 2, "{label}");
    }
}

#[tokio::test]
async fn malformed_state_and_outage_never_reach_the_revoke_effect() {
    let cases: [(&str, Change); 4] = [
        ("other subject (pending contract decision)", |s| {
            s.state_subject = Some(vec![0x44; 16])
        }),
        ("unspecified account state", |s| s.state_value = Some(0)),
        ("zero version", |s| s.version = 0),
        ("Identity unavailable", |s| s.state_unavailable = true),
    ];
    for (label, change) in cases {
        let (h, secrets, _) = seeded().await;
        let probe = RevokeProbe::new(&h.sessions);
        let before = snapshot(&h).await;
        h.fake.set(change);
        assert_eq!(
            admit(&h, &probe, &secrets).await,
            Err(AuthFailure::Unavailable),
            "{label}"
        );
        assert_eq!(
            probe.observed(),
            [],
            "the RPC boundary provides no revoking fact: {label}"
        );
        assert_eq!(snapshot(&h).await, before, "{label}");
        h.fake.set(|s| {
            *s = Script {
                state_calls: s.state_calls,
                ..Script::healthy()
            }
        });
        assert_eq!(admit(&h, &probe, &secrets).await, Ok(()), "{label}");
        assert_eq!(h.fake.state_calls(), 2, "fresh read on recovery: {label}");
        assert_eq!(probe.observed(), [], "{label}");
        assert_eq!(snapshot(&h).await, before, "{label}");
    }
}
