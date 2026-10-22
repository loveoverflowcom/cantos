//! FIFO async mutex for one document. Cookie-changing exchanges (login, renew, logout) are
//! dispatched one at a time, so the browser applies their `Set-Cookie` in dispatch order. A
//! cancelled waiter leaves the queue and never strands the next one.
use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

#[derive(Default)]
struct State {
    locked: bool,
    next: u64,
    queue: VecDeque<(u64, Option<Waker>)>,
}
impl State {
    fn wake_front(&mut self) {
        if let Some(waker) = self.queue.front_mut().and_then(|(_, waker)| waker.take()) {
            waker.wake();
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Serial(Rc<RefCell<State>>);

/// Held while one cookie-changing exchange is in flight.
pub(crate) struct Hold(Rc<RefCell<State>>);
impl Drop for Hold {
    fn drop(&mut self) {
        let mut state = self.0.borrow_mut();
        state.locked = false;
        state.wake_front();
    }
}

pub(crate) struct Acquire {
    state: Rc<RefCell<State>>,
    id: Option<u64>,
}
impl Serial {
    pub fn acquire(&self) -> Acquire {
        Acquire {
            state: self.0.clone(),
            id: None,
        }
    }
}
impl Future for Acquire {
    type Output = Hold;
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Hold> {
        let state = self.state.clone();
        let mut guard = state.borrow_mut();
        let id = match self.id {
            Some(id) => id,
            None => {
                let id = guard.next;
                guard.next += 1;
                guard.queue.push_back((id, None));
                self.id = Some(id);
                id
            }
        };
        if !guard.locked && guard.queue.front().is_some_and(|(front, _)| *front == id) {
            guard.queue.pop_front();
            guard.locked = true;
            drop(guard);
            self.id = None;
            return Poll::Ready(Hold(state));
        }
        if let Some((_, waker)) = guard.queue.iter_mut().find(|(queued, _)| *queued == id) {
            *waker = Some(context.waker().clone());
        }
        Poll::Pending
    }
}
impl Drop for Acquire {
    fn drop(&mut self) {
        let Some(id) = self.id else {
            return;
        };
        let mut state = self.state.borrow_mut();
        state.queue.retain(|(queued, _)| *queued != id);
        if !state.locked {
            state.wake_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::Waker;

    fn poll<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
        Pin::new(future).poll(&mut Context::from_waker(Waker::noop()))
    }

    #[test]
    fn waiters_enter_in_arrival_order_one_at_a_time() {
        let serial = Serial::default();
        let mut first = serial.acquire();
        let Poll::Ready(hold) = poll(&mut first) else {
            panic!("an idle mutex must admit at once");
        };
        let mut second = serial.acquire();
        let mut third = serial.acquire();
        assert!(poll(&mut second).is_pending());
        assert!(poll(&mut third).is_pending());
        drop(hold);
        assert!(
            poll(&mut third).is_pending(),
            "the third waiter cannot jump the queue"
        );
        let Poll::Ready(second_hold) = poll(&mut second) else {
            panic!("the second waiter is next");
        };
        assert!(poll(&mut third).is_pending());
        drop(second_hold);
        assert!(poll(&mut third).is_ready());
    }

    #[test]
    fn a_cancelled_waiter_never_strands_the_queue() {
        let serial = Serial::default();
        let Poll::Ready(hold) = poll(&mut serial.acquire()) else {
            panic!();
        };
        let mut abandoned = serial.acquire();
        let mut waiting = serial.acquire();
        assert!(poll(&mut abandoned).is_pending());
        assert!(poll(&mut waiting).is_pending());
        drop(abandoned);
        drop(hold);
        assert!(poll(&mut waiting).is_ready());
    }
}
