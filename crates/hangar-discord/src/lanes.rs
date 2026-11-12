//! Work kept in order where order matters, and side by side elsewhere: jobs
//! given the same key run one after the other, as given; jobs of different
//! keys run at once.
//!
//! What the connector does for an agent (its turn's card posted, edited,
//! folded, its answer) has to reach Discord in order, and each step waits on
//! Discord; sixteen agents must not wait on each other for it, and the loop
//! that hears their reports must wait on none of it. So each agent, each
//! channel the person writes in and each workspace is a lane.
//!
//! A lane's worker lives while it has work, and goes when its queue is
//! empty. The lanes own their tasks: closed (the connector stopped, or was
//! started again with another bot), what they were doing is cut and what
//! waited is dropped, rather than carried on beside the connector that
//! took their place.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use futures_util::FutureExt;
use tokio::sync::mpsc;

type Job = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Default)]
struct Inner {
    lanes: HashMap<String, mpsc::UnboundedSender<Job>>,
    /// The jobs of `run_once` given and not finished, by key and tag.
    once: HashSet<(String, &'static str)>,
    /// Every task of the lanes, cut with them.
    tasks: tokio::task::JoinSet<()>,
    closed: bool,
}

impl Inner {
    /// `task` started among the lanes' own; those that ended are forgotten.
    fn start(&mut self, task: impl Future<Output = ()> + Send + 'static) {
        while self.tasks.try_join_next().is_some() {}
        self.tasks.spawn(task);
    }
}

#[derive(Clone, Default)]
pub struct Lanes(Arc<Mutex<Inner>>);

impl Lanes {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs `job` after those already given under `key`. Called from inside
    /// a tokio runtime.
    pub fn run(&self, key: &str, job: impl Future<Output = ()> + Send + 'static) {
        let mut job: Job = Box::pin(job);
        let mut inner = self.inner();
        if inner.closed {
            return;
        }
        if let Some(lane) = inner.lanes.get(key) {
            match lane.send(job) {
                Ok(()) => return,
                // Its worker is gone: a new one takes over.
                Err(mpsc::error::SendError(back)) => job = back,
            }
        }
        let (tx, rx) = mpsc::unbounded_channel();
        let _ = tx.send(job);
        inner.lanes.insert(key.to_string(), tx);
        let worker = work(self.clone(), key.to_string(), rx);
        inner.start(worker);
    }

    /// Runs `job` at once, beside the lanes: for what has no order to keep
    /// and cannot wait behind anything (a click, which Discord gives three
    /// seconds to acknowledge). Cut with the lanes all the same.
    pub fn beside(&self, job: impl Future<Output = ()> + Send + 'static) {
        let mut inner = self.inner();
        if !inner.closed {
            inner.start(job);
        }
    }

    /// Cuts what the lanes do and drops what waits in them; nothing given
    /// afterwards runs.
    pub fn close(&self) {
        let mut inner = self.inner();
        inner.closed = true;
        inner.tasks.abort_all();
        inner.lanes.clear();
    }

    pub fn closed(&self) -> bool {
        self.inner().closed
    }

    /// The same, unless a job given under `key` with `tag` has not finished
    /// yet: for what is asked again at every tick, which must not pile up
    /// behind a slow lane. Whether it was given.
    pub fn run_once(
        &self,
        key: &str,
        tag: &'static str,
        job: impl Future<Output = ()> + Send + 'static,
    ) -> bool {
        if !self.inner().once.insert((key.to_string(), tag)) {
            return false;
        }
        let (lanes, owner) = (self.clone(), key.to_string());
        self.run(key, async move {
            // Forgotten when the job ends, however it ends.
            let _done = Done(lanes, owner, tag);
            job.await;
        });
        true
    }

    /// How many lanes have work.
    #[cfg(test)]
    pub fn busy(&self) -> usize {
        self.inner().lanes.len()
    }
}

/// Forgets a `run_once` job when it is dropped.
struct Done(Lanes, String, &'static str);

impl Drop for Done {
    fn drop(&mut self) {
        self.0
            .inner()
            .once
            .remove(&(std::mem::take(&mut self.1), self.2));
    }
}

async fn work(lanes: Lanes, key: String, mut rx: mpsc::UnboundedReceiver<Job>) {
    loop {
        let job = match rx.try_recv() {
            Ok(job) => job,
            Err(_) => {
                // Nothing waits: the lane goes, unless a job was given
                // meanwhile (the same lock is held to give one).
                let mut inner = lanes.inner();
                match rx.try_recv() {
                    Ok(job) => job,
                    Err(_) => {
                        inner.lanes.remove(&key);
                        return;
                    }
                }
            }
        };
        // A job that breaks is its own loss: the lane goes on with the next.
        if std::panic::AssertUnwindSafe(job)
            .catch_unwind()
            .await
            .is_err()
        {
            eprintln!("discord: a job of lane {key} broke");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    async fn idle(lanes: &Lanes) {
        for _ in 0..500 {
            if lanes.busy() == 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("the lanes never emptied");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_lane_keeps_its_order_and_lanes_run_side_by_side() {
        let lanes = Lanes::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let began = std::time::Instant::now();
        for lane in ["a", "b", "c", "d"] {
            for step in 0..5 {
                let seen = seen.clone();
                lanes.run(lane, async move {
                    // Each step waits, as a call to Discord does.
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    seen.lock().unwrap().push((lane, step));
                });
            }
        }
        idle(&lanes).await;
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 20);
        for lane in ["a", "b", "c", "d"] {
            let steps: Vec<i32> = seen
                .iter()
                .filter(|(l, _)| *l == lane)
                .map(|(_, s)| *s)
                .collect();
            assert_eq!(steps, [0, 1, 2, 3, 4], "lane {lane} in order");
        }
        // Four lanes of five 20 ms steps: 100 ms side by side, 400 in a row.
        assert!(
            began.elapsed() < Duration::from_millis(300),
            "{:?}",
            began.elapsed()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn what_a_tick_asks_again_does_not_pile_up() {
        let lanes = Lanes::default();
        let runs = Arc::new(Mutex::new(0));
        let mut given = 0;
        for _ in 0..10 {
            let runs = runs.clone();
            if lanes.run_once("ws", "dashboard", async move {
                tokio::time::sleep(Duration::from_millis(30)).await;
                *runs.lock().unwrap() += 1;
            }) {
                given += 1;
            }
        }
        assert_eq!(given, 1, "one waits, the others are not given");
        idle(&lanes).await;
        assert_eq!(*runs.lock().unwrap(), 1);
        // Once it has run, it can be asked again; another tag never waited.
        assert!(lanes.run_once("ws", "dashboard", async {}));
        assert!(lanes.run_once("ws", "journal", async {}));
        idle(&lanes).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn closed_lanes_cut_their_work_and_take_no_more() {
        let lanes = Lanes::default();
        let done = Arc::new(Mutex::new(Vec::new()));
        for (lane, wait) in [("a", 300), ("a", 0), ("b", 300)] {
            let done = done.clone();
            lanes.run(lane, async move {
                tokio::time::sleep(Duration::from_millis(wait)).await;
                done.lock().unwrap().push(lane);
            });
        }
        let beside = done.clone();
        lanes.beside(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            beside.lock().unwrap().push("beside");
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        lanes.close();
        assert!(lanes.closed());
        // Given after: not run either.
        let late = done.clone();
        lanes.run("c", async move { late.lock().unwrap().push("late") });
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            done.lock().unwrap().is_empty(),
            "{:?}",
            done.lock().unwrap()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_job_that_panics_does_not_block_its_lane() {
        let lanes = Lanes::default();
        lanes.run("a", async { panic!("a job gone wrong") });
        // Given at once, while the first still breaks: not lost with it.
        let ran = Arc::new(Mutex::new(false));
        let flag = ran.clone();
        lanes.run("a", async move { *flag.lock().unwrap() = true });
        for _ in 0..200 {
            if *ran.lock().unwrap() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("the lane stayed blocked");
    }
}
