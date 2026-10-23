//! Host tests of the production owner through a scripted [`Port`]. The port only records what
//! the owner dispatches and lets a test complete each answer in any order; it decides nothing.
//! This is `example-tested` / `mock-integration-tested`: no browser, cookie jar or Server.
use super::*;
use std::{
    pin::Pin,
    task::{Context, Poll, Waker},
};

const A1: &str = "00000000-0000-4000-8000-000000000001";
const B2: &str = "00000000-0000-4000-8000-000000000002";

type Slot = Rc<RefCell<Option<Result<Reply, Failure>>>>;

#[derive(Default)]
struct Script {
    wall: Cell<u64>,
    elapsed: Cell<u64>,
    sent: RefCell<Vec<(Request, Slot)>>,
    lost: Cell<u32>,
    announced: Cell<u32>,
    /// Another tab holds the cross-tab lock while set.
    tabs_busy: Cell<bool>,
}
struct TestPort(Rc<Script>);
impl Port for TestPort {
    type TabLock = ();
    fn clock(&self) -> (u64, u64) {
        (self.0.wall.get(), self.0.elapsed.get())
    }
    fn send(
        &self,
        _surface: Surface,
        request: Request,
    ) -> impl Future<Output = Result<Reply, Failure>> {
        let script = self.0.clone();
        async move {
            let slot = Slot::default();
            script.sent.borrow_mut().push((request, slot.clone()));
            std::future::poll_fn(move |_| match slot.borrow_mut().take() {
                Some(answer) => Poll::Ready(answer),
                None => Poll::Pending,
            })
            .await
        }
    }
    fn tab_lock(
        &self,
        _surface: Surface,
        _origin: Option<&str>,
    ) -> impl Future<Output = Result<(), Failure>> {
        let script = self.0.clone();
        std::future::poll_fn(move |_| {
            if script.tabs_busy.get() {
                Poll::Pending
            } else {
                Poll::Ready(Ok(()))
            }
        })
    }
    fn pause(&self, _millis: u32) -> impl Future<Output = Result<(), Failure>> {
        std::future::ready(Ok(()))
    }
    fn lost(&self, _surface: Surface) {
        self.0.lost.set(self.0.lost.get() + 1);
    }
    fn announce(&self, _surface: Surface) {
        self.0.announced.set(self.0.announced.get() + 1);
    }
}

type Task<T> = Pin<Box<dyn Future<Output = T>>>;
fn start<T>(future: impl Future<Output = T> + 'static) -> Task<T> {
    Box::pin(future)
}
fn step<T>(task: &mut Task<T>) -> Poll<T> {
    task.as_mut().poll(&mut Context::from_waker(Waker::noop()))
}
fn done<T>(task: &mut Task<T>) -> T {
    match step(task) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("the task is still waiting"),
    }
}

struct World {
    script: Rc<Script>,
    client: Client<TestPort>,
}
impl World {
    fn new() -> Self {
        let script = Rc::new(Script::default());
        script.wall.set(100);
        let client = Client::new(TestPort(script.clone()), Surface::Theatre, None).unwrap();
        Self { script, client }
    }
    fn count(&self) -> usize {
        self.script.sent.borrow().len()
    }
    fn request(&self, index: usize) -> Request {
        self.script.sent.borrow()[index].0.clone()
    }
    fn posts(&self) -> usize {
        self.script
            .sent
            .borrow()
            .iter()
            .filter(|(request, _)| {
                request.method == Method::Post && request.url.ends_with("/login")
            })
            .count()
    }
    fn complete(&self, index: usize, answer: Result<Reply, Failure>) {
        *self.script.sent.borrow()[index].1.borrow_mut() = Some(answer);
    }
    fn advance(&self, seconds: u64) {
        self.script.wall.set(self.script.wall.get() + seconds);
        self.script.elapsed.set(self.script.elapsed.get() + seconds);
    }
    /// Adopt a session through the real authoritative read.
    fn adopt(&self, session: &str, idle: u64) {
        let client = self.client.clone();
        let mut read = start(async move { client.read_session().await });
        assert!(step(&mut read).is_pending());
        self.complete(self.count() - 1, Ok(session_reply(session, idle)));
        assert!(done(&mut read).unwrap().is_some());
    }
}

fn session_reply(session: &str, idle: u64) -> Reply {
    let wire = Session {
        actor_id: "actor".into(),
        session_id: session.into(),
        expires_at: idle,
        absolute_expires_at: 900,
        authenticated_at: Some(10),
    };
    Reply {
        status: 200,
        content_type: Some("application/json; charset=utf-8".into()),
        retry_after: None,
        body: serde_json::to_vec(&wire).unwrap(),
    }
}
fn status(status: u16) -> Reply {
    Reply {
        status,
        content_type: None,
        retry_after: None,
        body: vec![],
    }
}
fn limited(seconds: &str) -> Reply {
    Reply {
        retry_after: Some(seconds.into()),
        ..status(429)
    }
}

#[test]
fn a_cancel_while_failed_logout_cleanup_is_retried_sends_no_login() {
    let world = World::new();
    world.adopt(A1, 600);
    // Logout whose DELETE is not acknowledged: the session stays queued and blocks admission.
    world.client.begin_logout(Some(A1)).unwrap();
    let client = world.client.clone();
    let mut failed = start(async move { client.cleanup().await });
    assert!(step(&mut failed).is_pending());
    world.complete(1, Ok(status(503)));
    assert_eq!(done(&mut failed), Err(Failure::CleanupPending));
    // A login waits for that cleanup (it retries the DELETE) ...
    let client = world.client.clone();
    let mut login = start(async move { client.login("a@b.c".into(), "pw".into(), None).await });
    assert!(step(&mut login).is_pending());
    assert_eq!(world.request(2).method, Method::Delete);
    // ... the user cancels while it waits, then the cleanup completes.
    world.client.cancel_authorization().unwrap();
    world.complete(2, Ok(status(204)));
    assert_eq!(done(&mut login), Err(Failure::Changed));
    assert_eq!(
        world.posts(),
        0,
        "a cancelled login must never be dispatched"
    );
    assert!(!world.client.has_session());
    // The owner is usable again: the cancelled attempt left nothing behind.
    assert_eq!(world.client.admission(), Ok(()));
}

#[test]
fn a_cancel_while_another_tab_holds_the_cookie_lock_sends_no_login() {
    let world = World::new();
    world.script.tabs_busy.set(true);
    let client = world.client.clone();
    let mut login = start(async move { client.login("a@b.c".into(), "pw".into(), None).await });
    assert!(step(&mut login).is_pending());
    world.client.cancel_authorization().unwrap();
    world.script.tabs_busy.set(false);
    assert_eq!(done(&mut login), Err(Failure::Changed));
    assert_eq!(world.count(), 0);
}

#[test]
fn a_cancel_after_the_post_left_revokes_the_late_session_and_never_adopts_it() {
    let world = World::new();
    let client = world.client.clone();
    let mut login = start(async move { client.login("a@b.c".into(), "pw".into(), None).await });
    assert!(step(&mut login).is_pending());
    assert_eq!(world.posts(), 1);
    world.client.cancel_authorization().unwrap();
    world.complete(0, Ok(session_reply(B2, 600)));
    // The unknown effect is closed by revoking exactly the session the API reported.
    assert!(step(&mut login).is_pending());
    let delete = world.request(1);
    assert_eq!(delete.method, Method::Delete);
    assert_eq!(delete.fence.as_deref(), Some(B2));
    world.complete(1, Ok(status(204)));
    assert_eq!(done(&mut login), Err(Failure::Changed));
    assert!(!world.client.has_session());
    assert!(world.client.0.cleanup.borrow().is_empty());
}

#[test]
fn a_definite_refusal_keeps_the_previous_session_and_an_unknown_effect_drops_it() {
    let world = World::new();
    world.adopt(A1, 600);
    for refusal in [status(401), status(400), status(403), limited("7")] {
        let client = world.client.clone();
        let mut login = start(async move { client.login("a@b.c".into(), "pw".into(), None).await });
        assert!(step(&mut login).is_pending());
        world.complete(world.count() - 1, Ok(refusal));
        assert!(done(&mut login).is_err());
        assert_eq!(world.client.session().unwrap().session, A1);
        world.advance(10);
    }
    let client = world.client.clone();
    let mut login = start(async move { client.login("a@b.c".into(), "pw".into(), None).await });
    assert!(step(&mut login).is_pending());
    world.complete(world.count() - 1, Err(Failure::Unavailable));
    assert_eq!(done(&mut login), Err(Failure::Unavailable));
    assert!(
        !world.client.has_session(),
        "the cookie may have been replaced"
    );
}

#[test]
fn an_old_read_renew_or_denial_arriving_after_a_new_intent_changes_nothing() {
    for old in [
        Ok(session_reply(A1, 700)),
        Ok(status(401)),
        Err(Failure::Unavailable),
    ] {
        let world = World::new();
        world.adopt(A1, 600);
        // An old read is in flight ...
        let client = world.client.clone();
        let mut read = start(async move { client.read_session().await });
        assert!(step(&mut read).is_pending());
        let stale_read = world.count() - 1;
        // ... the account is switched to B through logout and login.
        world.client.begin_logout(Some(A1)).unwrap();
        let client = world.client.clone();
        let mut login = start(async move { client.login("b@b.c".into(), "pw".into(), None).await });
        assert!(step(&mut login).is_pending());
        world.complete(world.count() - 1, Ok(status(204)));
        assert!(step(&mut login).is_pending());
        world.complete(world.count() - 1, Ok(session_reply(B2, 600)));
        assert_eq!(done(&mut login).unwrap().session, B2);
        let lost = world.script.lost.get();
        // The stale answer, success or denial, is fenced.
        world.complete(stale_read, old.clone());
        assert!(
            done(&mut read).is_err(),
            "{:?}",
            old.as_ref().map(|r| r.status)
        );
        assert_eq!(world.client.session().unwrap().session, B2);
        assert_eq!(world.script.lost.get(), lost);
    }
}

#[test]
fn a_renewal_reply_older_than_a_confirmed_read_keeps_the_larger_idle() {
    let world = World::new();
    world.adopt(A1, 600);
    world.advance(430); // 80 s before idle: a renewal is due
    let client = world.client.clone();
    let mut access = start(async move {
        client
            .access(Some(A1))
            .await
            .map(|lease| lease.session().to_owned())
    });
    assert!(step(&mut access).is_pending());
    let renew = world.count() - 1;
    assert!(world
        .request(renew)
        .url
        .ends_with("/identity/session/renew"));
    assert_eq!(world.request(renew).fence.as_deref(), Some(A1));
    // An authoritative read confirms a later idle deadline first.
    let client = world.client.clone();
    let mut read = start(async move { client.read_session().await });
    assert!(step(&mut read).is_pending());
    world.complete(world.count() - 1, Ok(session_reply(A1, 800)));
    assert_eq!(done(&mut read).unwrap().unwrap().idle, 800);
    // The renewal answers 700: valid for the snapshot it was sent for, never a regression.
    world.complete(renew, Ok(session_reply(A1, 700)));
    assert_eq!(done(&mut access).unwrap(), A1);
    assert_eq!(world.client.session().unwrap().idle, 800);
    assert_eq!(world.script.lost.get(), 0);
}

#[test]
fn a_renewal_below_its_dispatch_snapshot_is_refused_and_a_foreign_family_never_published() {
    for (reply, family_kept) in [
        (session_reply(A1, 500), false),
        (session_reply(B2, 700), false),
    ] {
        let world = World::new();
        world.adopt(A1, 600);
        world.advance(430);
        let client = world.client.clone();
        let mut access = start(async move { client.access(Some(A1)).await.map(|_| ()) });
        assert!(step(&mut access).is_pending());
        world.complete(world.count() - 1, Ok(reply));
        assert_eq!(done(&mut access), Err(Failure::Protocol));
        assert_eq!(world.client.has_session(), family_kept);
        assert_eq!(world.script.lost.get(), 1, "the views must stop showing it");
    }
}

#[test]
fn a_renewal_of_a_switched_intent_is_stale_and_publishes_nothing() {
    let world = World::new();
    world.adopt(A1, 600);
    world.advance(430);
    let client = world.client.clone();
    let mut access = start(async move { client.access(Some(A1)).await.map(|_| ()) });
    assert!(step(&mut access).is_pending());
    let renew = world.count() - 1;
    // Another tab switched the session; an authoritative read adopts it and moves the intent.
    let client = world.client.clone();
    let mut read = start(async move { client.read_session().await });
    assert!(step(&mut read).is_pending());
    world.complete(world.count() - 1, Ok(session_reply(B2, 600)));
    assert_eq!(done(&mut read).unwrap().unwrap().session, B2);
    world.complete(renew, Ok(session_reply(A1, 700)));
    assert_eq!(done(&mut access), Err(Failure::Changed));
    let kept = world.client.session().unwrap();
    assert_eq!((kept.session.as_str(), kept.idle), (B2, 600));
    // A stale 401 of the old renewal cannot sign out the new account either.
    let world = World::new();
    world.adopt(A1, 600);
    world.advance(430);
    let client = world.client.clone();
    let mut access = start(async move { client.access(Some(A1)).await.map(|_| ()) });
    assert!(step(&mut access).is_pending());
    let renew = world.count() - 1;
    let client = world.client.clone();
    let mut read = start(async move { client.read_session().await });
    assert!(step(&mut read).is_pending());
    world.complete(world.count() - 1, Ok(session_reply(B2, 600)));
    done(&mut read).unwrap();
    world.complete(renew, Ok(status(401)));
    assert_eq!(done(&mut access), Err(Failure::Changed));
    assert_eq!(world.client.session().unwrap().session, B2);
    assert_eq!(world.script.lost.get(), 0);
}

#[test]
fn a_429_holds_every_protected_request_even_for_a_session_that_is_still_valid() {
    let world = World::new();
    world.adopt(A1, 600);
    let client = world.client.clone();
    let mut access = start(async move { client.access(Some(A1)).await });
    let lease = done(&mut access).unwrap();
    let sent = world.count();
    assert_eq!(lease.response(200, None, None), Ok(()));
    assert_eq!(
        lease.response(429, Some("12"), None),
        Err(Failure::RateLimited(12))
    );
    // Within the hold nothing is admitted, so nothing can be dispatched.
    world.advance(5);
    let client = world.client.clone();
    let mut held = start(async move { client.access(Some(A1)).await.map(|_| ()) });
    assert_eq!(done(&mut held), Err(Failure::RateLimited(7)));
    assert_eq!(world.count(), sent);
    let client = world.client.clone();
    let mut read = start(async move { client.read_session().await.map(|_| ()) });
    assert_eq!(done(&mut read), Err(Failure::RateLimited(7)));
    world.advance(7);
    let client = world.client.clone();
    let mut again = start(async move { client.access(Some(A1)).await.map(|_| ()) });
    assert_eq!(done(&mut again), Ok(()));
}

#[test]
fn an_unreadable_retry_after_still_holds_and_a_stale_lease_still_arms_the_gate() {
    let world = World::new();
    world.adopt(A1, 600);
    let client = world.client.clone();
    let mut access = start(async move { client.access(Some(A1)).await });
    let lease = done(&mut access).unwrap();
    // Another tab switches the account: the lease is stale, but the API's 429 is still a fact.
    world.client.invalidate().unwrap();
    assert_eq!(lease.response(429, None, None), Err(Failure::Changed));
    assert_eq!(world.client.gate(), Err(Failure::RateLimited(30)));
    assert_eq!(lease.response(401, None, None), Err(Failure::Changed));
    assert_eq!(world.script.lost.get(), 0, "a stale 401 signs nobody out");
}

#[test]
fn only_a_current_401_signs_out_and_403_is_left_to_the_caller() {
    let world = World::new();
    world.adopt(A1, 600);
    let client = world.client.clone();
    let mut access = start(async move { client.access(Some(A1)).await });
    let lease = done(&mut access).unwrap();
    assert_eq!(lease.response(403, None, None), Ok(()));
    assert_eq!(lease.response(409, None, None), Ok(()));
    assert!(world.client.has_session());
    assert_eq!(lease.response(401, None, None), Err(Failure::SignedOut));
    assert!(!world.client.has_session());
    assert_eq!(world.script.lost.get(), 1);
}

#[test]
fn an_echo_of_another_session_drops_the_local_session_for_any_status() {
    for answer in [200, 204, 400, 409, 429, 503] {
        let world = World::new();
        world.adopt(A1, 600);
        let client = world.client.clone();
        let mut access = start(async move { client.access(Some(A1)).await });
        let lease = done(&mut access).unwrap();
        // The lease's own session, or no readable echo, changes nothing.
        assert_eq!(lease.response(200, None, Some(A1)), Ok(()));
        assert!(world.client.has_session());
        assert_eq!(
            lease.response(answer, Some("12"), Some(B2)),
            Err(Failure::Changed),
            "{answer}"
        );
        assert!(!world.client.has_session(), "{answer}");
        assert_eq!(lease.check(), Err(Failure::Changed));
        // A fence mismatch is not a sign-out: the views re-read instead of being told it is gone.
        assert_eq!(world.script.lost.get(), 0, "{answer}");
        // The 429 is still a fact about the API, even though the account changed.
        let held = world.client.gate();
        assert_eq!(held.is_err(), answer == 429, "{answer}");
        world.advance(12);
        // A stale lease cannot drop or sign out whatever session the owner adopts next.
        let sent = world.count();
        world.adopt(B2, 600);
        assert_eq!(world.count(), sent + 1);
        assert_eq!(lease.response(401, None, Some(A1)), Err(Failure::Changed));
        assert_eq!(world.client.session().unwrap().session, B2);
    }
}

#[test]
fn cookie_changing_exchanges_leave_one_at_a_time_in_dispatch_order() {
    let world = World::new();
    world.adopt(A1, 600);
    world.advance(430);
    // Renewal of A is in flight ...
    let client = world.client.clone();
    let mut access = start(async move { client.access(Some(A1)).await.map(|_| ()) });
    assert!(step(&mut access).is_pending());
    let renew = world.count() - 1;
    // ... the account is switched in the same document: logout A, then login B.
    world.client.begin_logout(Some(A1)).unwrap();
    let client = world.client.clone();
    let mut login = start(async move { client.login("b@b.c".into(), "pw".into(), None).await });
    assert!(step(&mut login).is_pending());
    assert_eq!(
        world.count(),
        renew + 1,
        "the DELETE waits for the renewal's answer"
    );
    // The renewal answers late; only then can the next cookie-changing request leave.
    world.complete(renew, Ok(session_reply(A1, 700)));
    assert_eq!(done(&mut access), Err(Failure::Changed));
    assert!(step(&mut login).is_pending());
    assert_eq!(world.request(renew + 1).method, Method::Delete);
    assert_eq!(world.count(), renew + 2, "login still waits for the DELETE");
    world.complete(renew + 1, Ok(status(204)));
    assert!(step(&mut login).is_pending());
    assert_eq!(world.posts(), 1);
    world.complete(renew + 2, Ok(session_reply(B2, 600)));
    assert_eq!(done(&mut login).unwrap().session, B2);
}

#[test]
fn another_tab_holding_the_cookie_lock_delays_renewal_and_dispatches_nothing() {
    let world = World::new();
    world.adopt(A1, 600);
    world.advance(430);
    world.script.tabs_busy.set(true);
    let client = world.client.clone();
    let mut access = start(async move { client.access(Some(A1)).await.map(|_| ()) });
    assert!(step(&mut access).is_pending());
    assert_eq!(world.count(), 1, "only the earlier read was sent");
    world.script.tabs_busy.set(false);
    assert!(step(&mut access).is_pending());
    assert_eq!(world.count(), 2);
    world.complete(1, Ok(session_reply(A1, 700)));
    assert_eq!(done(&mut access), Ok(()));
}

#[test]
fn logout_cleanup_is_pinned_to_its_session_and_other_tabs_are_signalled_once_cleared() {
    let world = World::new();
    world.adopt(A1, 600);
    let client = world.client.clone();
    let mut logout = start(async move { client.logout(Some(A1)).await });
    assert!(step(&mut logout).is_pending());
    let delete = world.request(1);
    assert_eq!(
        (delete.method, delete.fence.as_deref()),
        (Method::Delete, Some(A1))
    );
    assert_eq!(world.script.announced.get(), 0);
    world.complete(1, Ok(status(204)));
    assert_eq!(done(&mut logout), Ok(()));
    assert_eq!(world.script.announced.get(), 1);
    // 401 and 409 are terminal without a signal: nothing was cleared by this document.
    for terminal in [401, 409] {
        let world = World::new();
        world.adopt(A1, 600);
        let client = world.client.clone();
        let mut logout = start(async move { client.logout(Some(A1)).await });
        assert!(step(&mut logout).is_pending());
        world.complete(1, Ok(status(terminal)));
        assert_eq!(done(&mut logout), Ok(()));
        assert_eq!(world.script.announced.get(), 0);
    }
}

#[test]
fn a_signal_from_another_tab_withdraws_authority_without_proving_anything() {
    let world = World::new();
    world.external_change_with_no_session();
    world.adopt(A1, 600);
    let client = world.client.clone();
    let mut access = start(async move { client.access(Some(A1)).await });
    let lease = done(&mut access).unwrap();
    world.client.external_change();
    assert_eq!(world.script.lost.get(), 1);
    assert!(!world.client.has_session());
    assert_eq!(lease.check(), Err(Failure::Changed));
    // Nothing was sent: the views re-read through the authoritative path.
    assert_eq!(world.count(), 1);
}
impl World {
    fn external_change_with_no_session(&self) {
        self.client.external_change();
        assert_eq!(self.script.lost.get(), 0);
    }
}

#[test]
fn login_success_adopts_one_session_and_announces_the_cookie_change() {
    let world = World::new();
    let client = world.client.clone();
    let mut login = start(async move { client.login("a@b.c".into(), "pw".into(), None).await });
    assert!(step(&mut login).is_pending());
    let request = world.request(0);
    assert_eq!(request.method, Method::Post);
    assert_eq!(request.fence, None);
    world.complete(0, Ok(session_reply(A1, 600)));
    assert_eq!(done(&mut login).unwrap().session, A1);
    assert_eq!(world.script.announced.get(), 1);
    // A login answered for another actor than the retained owner is refused and revoked.
    let world = World::new();
    let client = world.client.clone();
    let mut login = start(async move {
        client
            .login("a@b.c".into(), "pw".into(), Some("someone-else".into()))
            .await
    });
    assert!(step(&mut login).is_pending());
    world.complete(0, Ok(session_reply(A1, 600)));
    assert!(step(&mut login).is_pending());
    world.complete(1, Ok(status(204)));
    assert_eq!(done(&mut login), Err(Failure::AccountMismatch));
    assert!(!world.client.has_session());
}

#[test]
fn malformed_or_oversized_session_bodies_are_protocol_faults() {
    let mut oversized = session_reply(A1, 600);
    oversized.body = vec![b' '; MAX_BODY + 1];
    let mut not_json = session_reply(A1, 600);
    not_json.content_type = Some("text/html".into());
    for bad in [oversized, not_json, status(200)] {
        let world = World::new();
        let client = world.client.clone();
        let mut read = start(async move { client.read_session().await });
        assert!(step(&mut read).is_pending());
        world.complete(0, Ok(bad));
        assert_eq!(done(&mut read), Err(Failure::Protocol));
        assert!(!world.client.has_session());
    }
}
