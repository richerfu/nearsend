use super::{task_queue::TaskQueue, worker_state::WorkerState};
use crate::RunnableVariant;
use std::{
    io,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(Default)]
struct Worker {
    running: AtomicBool,
    native: OnceLock<io::Result<WorkerState>>,
}

impl Worker {
    fn available_capacity(&self) -> bool {
        if !self.running.load(Ordering::Acquire) {
            // Starting and idle workers already contribute capacity.
            return true;
        }
        match self.native.get() {
            Some(Ok(native)) => native.is_blocked().is_ok_and(|blocked| !blocked),
            // If kernel introspection is unavailable, preserve progress by using
            // the conservative escape path. Never strand synchronous waiters.
            Some(Err(_)) => false,
            None => true,
        }
    }
}

pub(crate) struct Workers;

impl Workers {
    pub(crate) fn start(queue: Arc<TaskQueue<RunnableVariant>>) {
        let count = thread::available_parallelism().map_or(2, |n| n.get().clamp(2, 8));
        let workers = Arc::new(Mutex::new(Vec::new()));
        for index in 0..count {
            if let Err(error) = Self::spawn(queue.clone(), &workers, index, false) {
                log::error!("Cannot start OHOS worker: {error}");
            }
        }
        thread::Builder::new()
            .name("OhosWorkerMonitor".into())
            .spawn(move || {
                let mut index = count;
                while queue.wait_for_stalled_work(Duration::from_millis(10)) {
                    let capacity = {
                        let mut workers = workers.lock().unwrap_or_else(|e| e.into_inner());
                        workers.retain(|worker| Weak::strong_count(worker) != 0);
                        workers
                            .iter()
                            .filter_map(Weak::upgrade)
                            .filter(|worker| worker.available_capacity())
                            .count()
                    };
                    // Elapsed time only schedules an observation. Actual OS wait
                    // state decides growth; CPU work cannot trigger expansion.
                    if capacity >= count {
                        continue;
                    }
                    if let Err(error) = Self::spawn(queue.clone(), &workers, index, true) {
                        log::error!("Cannot supplement blocked OHOS workers: {error}");
                        thread::sleep(Duration::from_secs(1));
                    }
                    index = index.wrapping_add(1);
                }
            })
            .expect("Failed to start OHOS worker monitor");
    }

    fn spawn(
        queue: Arc<TaskQueue<RunnableVariant>>,
        workers: &Mutex<Vec<Weak<Worker>>>,
        index: usize,
        elastic: bool,
    ) -> io::Result<()> {
        let worker = Arc::new(Worker::default());
        workers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Arc::downgrade(&worker));
        thread::Builder::new()
            .name(format!("OhosWorker-{index}"))
            .spawn(move || {
                let native = WorkerState::current();
                if let Err(error) = &native {
                    log::warn!("Cannot inspect OHOS worker scheduling state: {error}");
                }
                let _ = worker.native.set(native);
                loop {
                    let runnable = if elastic {
                        queue.pop_timeout(Duration::from_secs(1))
                    } else {
                        queue.pop()
                    };
                    let Some(runnable) = runnable else { break };
                    worker.running.store(true, Ordering::Release);
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runnable.run()))
                        .is_err()
                    {
                        log::error!("OHOS background task panicked; worker remains available");
                    }
                    worker.running.store(false, Ordering::Release);
                }
            })?;
        Ok(())
    }
}
