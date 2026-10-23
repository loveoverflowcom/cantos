//! Bounded KDF execution. A job holds its semaphore permit until the blocking closure itself
//! returns, so a cancelled or timed-out request cannot free capacity for work that is still
//! running: `spawn_blocking` tasks are not aborted by dropping their handle.

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::{sync::Semaphore, time::timeout};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolFault {
    /// The waiting line is full: the request is refused immediately.
    QueueFull,
    /// No worker became free before the queue deadline.
    QueueDeadline,
    /// The job panicked or its worker thread was lost.
    JobFailed,
}

#[derive(Clone)]
pub struct KdfPool {
    permits: Arc<Semaphore>,
    waiting: Arc<AtomicUsize>,
    running_limit: usize,
    waiting_limit: usize,
    deadline: Duration,
}

struct WaitGuard(Arc<AtomicUsize>);

impl Drop for WaitGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl KdfPool {
    pub fn new(running_limit: usize, waiting_limit: usize, deadline: Duration) -> Self {
        let running_limit = running_limit.max(1);
        Self {
            permits: Arc::new(Semaphore::new(running_limit)),
            waiting: Arc::new(AtomicUsize::new(0)),
            running_limit,
            waiting_limit,
            deadline,
        }
    }

    /// Jobs holding a permit right now, including ones whose caller already gave up.
    pub fn running(&self) -> usize {
        self.running_limit - self.permits.available_permits()
    }

    pub fn waiting(&self) -> usize {
        self.waiting.load(Ordering::SeqCst)
    }

    pub async fn run<R: Send + 'static>(
        &self,
        job: impl FnOnce() -> R + Send + 'static,
    ) -> Result<R, PoolFault> {
        let queued = self.waiting.fetch_add(1, Ordering::SeqCst);
        let guard = WaitGuard(Arc::clone(&self.waiting));
        if queued >= self.waiting_limit && self.permits.available_permits() == 0 {
            return Err(PoolFault::QueueFull);
        }
        let permit = match timeout(self.deadline, Arc::clone(&self.permits).acquire_owned()).await {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => return Err(PoolFault::JobFailed),
            Err(_) => return Err(PoolFault::QueueDeadline),
        };
        drop(guard);
        // The permit travels into the closure: it is released when the work ends, not when
        // this future is dropped.
        tokio::task::spawn_blocking(move || {
            let _held = permit;
            job()
        })
        .await
        .map_err(|_| PoolFault::JobFailed)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{mpsc, Barrier};

    use super::*;

    /// A condition that never arrives fails the test instead of hanging the suite.
    async fn until(mut condition: impl FnMut() -> bool) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !condition() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("condition was not reached");
    }

    fn gate() -> (mpsc::Sender<()>, Arc<std::sync::Mutex<mpsc::Receiver<()>>>) {
        let (tx, rx) = mpsc::channel();
        (tx, Arc::new(std::sync::Mutex::new(rx)))
    }

    #[tokio::test]
    async fn work_runs_off_the_executor_and_returns_its_value() {
        let pool = KdfPool::new(2, 2, Duration::from_secs(1));
        let caller = std::thread::current().id();
        let (value, ran_on) = pool
            .run(|| (41 + 1, std::thread::current().id()))
            .await
            .unwrap();
        assert_eq!(value, 42);
        assert_ne!(ran_on, caller);
        assert_eq!((pool.running(), pool.waiting()), (0, 0));
    }

    #[tokio::test]
    async fn running_never_exceeds_the_limit_and_the_queue_refuses_beyond_its_bound() {
        let pool = KdfPool::new(1, 1, Duration::from_secs(5));
        let (release, rx) = gate();
        let first_rx = Arc::clone(&rx);
        let first = tokio::spawn({
            let pool = pool.clone();
            async move {
                pool.run(move || first_rx.lock().unwrap().recv().unwrap())
                    .await
            }
        });
        until(|| pool.running() != 0).await;
        let second_rx = Arc::clone(&rx);
        let second = tokio::spawn({
            let pool = pool.clone();
            async move {
                pool.run(move || second_rx.lock().unwrap().recv().unwrap())
                    .await
            }
        });
        until(|| pool.waiting() != 0).await;
        assert_eq!((pool.running(), pool.waiting()), (1, 1));
        // A third request finds one running and a full line: refused without waiting.
        assert_eq!(pool.run(|| ()).await, Err(PoolFault::QueueFull));
        release.send(()).unwrap();
        release.send(()).unwrap();
        first.await.unwrap().unwrap();
        second.await.unwrap().unwrap();
        assert_eq!((pool.running(), pool.waiting()), (0, 0));
    }

    #[tokio::test]
    async fn a_waiter_that_times_out_leaves_the_running_job_and_the_counters_intact() {
        let pool = KdfPool::new(1, 4, Duration::from_millis(50));
        let (release, rx) = gate();
        let job_rx = Arc::clone(&rx);
        let running = tokio::spawn({
            let pool = pool.clone();
            async move {
                pool.run(move || job_rx.lock().unwrap().recv().unwrap())
                    .await
            }
        });
        until(|| pool.running() != 0).await;
        assert_eq!(pool.run(|| ()).await, Err(PoolFault::QueueDeadline));
        assert_eq!((pool.running(), pool.waiting()), (1, 0));
        release.send(()).unwrap();
        running.await.unwrap().unwrap();
        assert_eq!(pool.running(), 0);
    }

    #[tokio::test]
    async fn dropping_the_caller_does_not_free_a_permit_until_the_work_ends() {
        let pool = KdfPool::new(1, 1, Duration::from_secs(5));
        let (release, rx) = gate();
        let started = Arc::new(Barrier::new(2));
        let job_started = Arc::clone(&started);
        let caller = tokio::spawn({
            let pool = pool.clone();
            async move {
                pool.run(move || {
                    job_started.wait();
                    rx.lock().unwrap().recv().unwrap();
                })
                .await
            }
        });
        tokio::task::spawn_blocking(move || started.wait())
            .await
            .unwrap();
        // The request is cancelled while its hash is still executing.
        caller.abort();
        let _ = caller.await;
        assert_eq!(pool.running(), 1, "the worker still holds its permit");
        release.send(()).unwrap();
        until(|| pool.running() == 0).await;
        assert_eq!(pool.run(|| 7).await, Ok(7));
    }

    #[tokio::test]
    async fn a_panicking_job_is_contained_and_releases_its_permit() {
        let pool = KdfPool::new(1, 1, Duration::from_secs(1));
        assert_eq!(
            pool.run(|| -> () { panic!("simulated KDF failure") }).await,
            Err(PoolFault::JobFailed)
        );
        assert_eq!(pool.run(|| 1).await, Ok(1));
    }
}
