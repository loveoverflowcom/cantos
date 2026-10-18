//! One in-flight refresh result shared by all callers, including unknown outcomes.
//! The production browser owner and host concurrency tests use this exact coordinator.
use crate::Failure;
use futures_util::future::{LocalBoxFuture, Shared};
use futures_util::FutureExt;
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
#[derive(Clone)]
pub(crate) struct Flight {
    future: Shared<LocalBoxFuture<'static, Result<(), Failure>>>,
    identity: Rc<()>,
}
impl Flight {
    fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.identity, &other.identity)
    }
}
impl Future for Flight {
    type Output = Result<(), Failure>;
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.future).poll(context)
    }
}
#[derive(Default)]
pub(crate) struct RefreshFlight(RefCell<Option<Flight>>);
impl RefreshFlight {
    pub fn detach(&self) {
        self.0.borrow_mut().take();
    }
    pub fn is_pending(&self) -> bool {
        self.0.borrow().is_some()
    }
    pub fn acquire(
        &self,
        start: impl FnOnce() -> LocalBoxFuture<'static, Result<(), Failure>>,
    ) -> Flight {
        let mut slot = self.0.borrow_mut();
        if let Some(existing) = slot.as_ref() {
            return existing.clone();
        }
        let future = Flight {
            future: start().shared(),
            identity: Rc::new(()),
        };
        *slot = Some(future.clone());
        future
    }
    pub fn finish(&self, future: &Flight) {
        if self
            .0
            .borrow()
            .as_ref()
            .is_some_and(|current| current.ptr_eq(future))
        {
            self.0.borrow_mut().take();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        future::Future,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    };
    fn poll(future: &mut Flight) -> Poll<Result<(), Failure>> {
        Pin::new(future).poll(&mut Context::from_waker(Waker::noop()))
    }
    #[test]
    fn concurrent_callers_send_one_refresh_and_receive_the_same_unknown_outcome() {
        let coordinator = RefreshFlight::default();
        let calls = Rc::new(Cell::new(0));
        let ready = Rc::new(Cell::new(false));
        let count = calls.clone();
        let release = ready.clone();
        let mut a = coordinator.acquire(|| {
            async move {
                count.set(count.get() + 1);
                std::future::poll_fn(move |_| {
                    if release.get() {
                        Poll::Ready(Err(Failure::Unavailable))
                    } else {
                        Poll::Pending
                    }
                })
                .await
            }
            .boxed_local()
        });
        let mut b = coordinator.acquire(|| panic!("a second request must never start"));
        assert_eq!(poll(&mut a), Poll::Pending);
        assert_eq!(poll(&mut b), Poll::Pending);
        assert_eq!(calls.get(), 1);
        ready.set(true);
        assert_eq!(poll(&mut b), Poll::Ready(Err(Failure::Unavailable)));
        assert_eq!(poll(&mut a), Poll::Ready(Err(Failure::Unavailable)));
        assert_eq!(calls.get(), 1);
        coordinator.finish(&a);
        assert!(!coordinator.is_pending());
    }
    #[test]
    fn a_late_waiter_cannot_clear_a_newer_flight() {
        let coordinator = RefreshFlight::default();
        let mut old = coordinator.acquire(|| async { Ok(()) }.boxed_local());
        assert_eq!(poll(&mut old), Poll::Ready(Ok(())));
        coordinator.detach();
        let current = coordinator.acquire(|| std::future::pending().boxed_local());
        coordinator.finish(&old);
        assert!(coordinator.is_pending());
        let joined = coordinator.acquire(|| panic!("newer flight was erased"));
        assert!(current.ptr_eq(&joined));
    }
    #[test]
    fn new_account_detaches_old_refresh_without_reusing_or_cancelling_the_new_flight() {
        let coordinator = RefreshFlight::default();
        let old = coordinator.acquire(|| std::future::pending().boxed_local());
        coordinator.detach();
        let current = coordinator.acquire(|| std::future::pending().boxed_local());
        assert!(!old.ptr_eq(&current));
        coordinator.finish(&old);
        assert!(coordinator.is_pending());
        assert!(current.ptr_eq(&coordinator.acquire(|| panic!("B would replay its refresh"))));
    }
    #[test]
    fn dropped_waiters_do_not_start_a_second_spent_refresh() {
        let coordinator = RefreshFlight::default();
        let first = coordinator.acquire(|| std::future::pending().boxed_local());
        let identity = first.clone();
        drop(first);
        let next = coordinator.acquire(|| panic!("interrupted refresh was replayed"));
        assert!(identity.ptr_eq(&next));
    }
}
