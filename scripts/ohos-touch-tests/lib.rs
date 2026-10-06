// Native wake and the main queue are host shims; GPUI futures and production queues are real.
extern crate self as openharmony_ability;
pub use gpui::{PlatformDispatcher, Priority, RunnableVariant};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[derive(Clone, Default)]
pub struct OpenHarmonyWaker {
    calls: Arc<AtomicUsize>,
}
impl OpenHarmonyWaker {
    pub fn wake(&self) {
        self.calls.fetch_add(1, Ordering::Relaxed);
    }
}
#[allow(dead_code)]
#[path = "../../vendor/gpui-ohos/src/ohos/capture_pixels.rs"]
mod capture_pixels;
#[allow(dead_code)]
#[path = "../../vendor/gpui-ohos/src/ohos/dispatcher.rs"]
mod dispatcher;
#[allow(dead_code)]
#[path = "../../vendor/gpui-ohos/src/ohos/frame_request.rs"]
mod frame_request;
#[allow(dead_code)]
#[path = "../../vendor/gpui-ohos/src/ohos/render_cache.rs"]
mod render_cache;
#[path = "../../vendor/gpui-ohos/src/ohos/task_queue.rs"]
mod task_queue;
#[allow(dead_code)]
#[path = "../../vendor/gpui-ohos/src/ohos/touch_scroll.rs"]
mod touch_scroll;

pub struct PriorityQueueSender<T>(Arc<task_queue::TaskQueue<T>>);
#[cfg(test)]
pub struct PriorityQueueReceiver<T>(Arc<task_queue::TaskQueue<T>>);
impl<T> PriorityQueueSender<T> {
    fn send(&self, priority: Priority, value: T) -> Result<(), T> {
        self.0.push(priority, value)
    }
}
#[cfg(test)]
impl<T> PriorityQueueReceiver<T> {
    fn new() -> (PriorityQueueSender<T>, Self) {
        let state = Arc::new(task_queue::TaskQueue::default());
        (PriorityQueueSender(state.clone()), Self(state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{BackgroundExecutor, ForegroundExecutor};
    use std::{
        collections::HashSet,
        future::Future,
        pin::Pin,
        sync::Mutex,
        task::{Context, Poll},
        time::{Duration, Instant},
    };
    struct YieldOnce(bool);
    impl Future for YieldOnce {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.0 {
                Poll::Ready(())
            } else {
                self.0 = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
    #[test]
    fn cooperative_background_rescheduling_reuses_workers() {
        let (sender, _) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(dispatcher::OhosDispatcher::new(sender));
        let executor = BackgroundExecutor::new(dispatcher.clone());
        let ids = Arc::new(Mutex::new(HashSet::new()));
        let (tx, rx) = std::sync::mpsc::channel();
        for _ in 0..16 {
            let ids = ids.clone();
            let tx = tx.clone();
            executor
                .spawn(async move {
                    for _ in 0..64 {
                        ids.lock().unwrap().insert(std::thread::current().id());
                        YieldOnce(false).await;
                    }
                    tx.send(()).unwrap();
                })
                .detach();
        }
        for _ in 0..16 {
            rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        assert!(ids.lock().unwrap().len() <= 8);
        dispatcher.shutdown();
    }
    #[test]
    fn blocking_background_parents_can_wait_for_queued_children() {
        let (sender, _) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(dispatcher::OhosDispatcher::new(sender));
        let executor = BackgroundExecutor::new(dispatcher.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        for _ in 0..16 {
            let executor = executor.clone();
            let tx = tx.clone();
            executor
                .clone()
                .spawn(async move {
                    let (child_tx, child_rx) = std::sync::mpsc::channel();
                    executor
                        .spawn(async move {
                            child_tx.send(()).unwrap();
                        })
                        .detach();
                    child_rx.recv_timeout(Duration::from_secs(3)).unwrap();
                    tx.send(()).unwrap();
                })
                .detach();
        }
        for _ in 0..16 {
            rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        dispatcher.shutdown();
    }
    #[test]
    fn external_work_progresses_while_all_base_workers_are_blocked() {
        let (sender, _) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(dispatcher::OhosDispatcher::new(sender));
        let executor = BackgroundExecutor::new(dispatcher.clone());
        let count = std::thread::available_parallelism().map_or(2, |n| n.get().clamp(2, 8));
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let mut releases = Vec::new();
        for _ in 0..count {
            let (tx, rx) = std::sync::mpsc::channel();
            releases.push(tx);
            let started = started_tx.clone();
            executor
                .spawn(async move {
                    started.send(()).unwrap();
                    rx.recv_timeout(Duration::from_secs(3)).unwrap();
                })
                .detach();
        }
        for _ in 0..count {
            started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        executor
            .spawn(async move {
                done_tx.send(()).unwrap();
            })
            .detach();
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        for release in releases {
            release.send(()).unwrap();
        }
        dispatcher.shutdown();
    }
    #[test]
    fn main_queue_wakes_once_per_burst_and_rearms_after_pumping() {
        let (sender, queue) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(dispatcher::OhosDispatcher::new(sender));
        let wake = OpenHarmonyWaker::default();
        dispatcher.set_waker(wake.clone());
        dispatcher.begin_main_turn();
        let before = wake.calls.load(Ordering::Relaxed);
        let foreground = ForegroundExecutor::new(dispatcher.clone());
        let count = Arc::new(AtomicUsize::new(0));
        for _ in 0..100 {
            let count = count.clone();
            foreground
                .spawn(async move {
                    count.fetch_add(1, Ordering::Relaxed);
                })
                .detach();
        }
        assert_eq!(wake.calls.load(Ordering::Relaxed) - before, 1);
        dispatcher.begin_main_turn();
        for _ in 0..100 {
            queue.0.pop().unwrap().run();
        }
        assert_eq!(count.load(Ordering::Relaxed), 100);
        let before = wake.calls.load(Ordering::Relaxed);
        foreground.spawn(async {}).detach();
        assert_eq!(wake.calls.load(Ordering::Relaxed) - before, 1);
        queue.0.pop().unwrap().run();
        dispatcher.shutdown();
    }
    #[test]
    fn due_timer_wakes_and_runs_on_the_ui_thread() {
        let (sender, _) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(dispatcher::OhosDispatcher::new(sender));
        let wake = OpenHarmonyWaker::default();
        dispatcher.set_waker(wake.clone());
        dispatcher.begin_main_turn();
        let executor = BackgroundExecutor::new(dispatcher.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        executor
            .spawn(async move {
                tx.send(()).unwrap();
            })
            .detach();
        rx.recv_timeout(Duration::from_secs(1)).unwrap();
        // A foreground future arms its timer from the creating UI thread.
        let (sender, queue) = PriorityQueueReceiver::new();
        let timer_dispatcher = Arc::new(dispatcher::OhosDispatcher::new(sender));
        timer_dispatcher.set_waker(wake.clone());
        timer_dispatcher.begin_main_turn();
        let foreground = ForegroundExecutor::new(timer_dispatcher.clone());
        let background = BackgroundExecutor::new(timer_dispatcher.clone());
        let done = Arc::new(AtomicUsize::new(0));
        let observed = done.clone();
        foreground
            .spawn(async move {
                background.timer(Duration::from_millis(10)).await;
                observed.store(1, Ordering::Relaxed);
            })
            .detach();
        queue.0.pop().unwrap().run();
        let deadline = Instant::now() + Duration::from_secs(1);
        while !timer_dispatcher.has_due_timers() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        timer_dispatcher.take_due_timer().unwrap().run();
        queue.0.pop().unwrap().run();
        assert_eq!(done.load(Ordering::Relaxed), 1);
        timer_dispatcher.shutdown();
        dispatcher.shutdown();
    }
    #[test]
    fn shutdown_releases_a_waiting_timer_without_waiting_for_its_deadline() {
        let (sender, queue) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(dispatcher::OhosDispatcher::new(sender));
        let foreground = ForegroundExecutor::new(dispatcher.clone());
        let background = BackgroundExecutor::new(dispatcher.clone());
        let task = foreground.spawn(async move {
            background.timer(Duration::from_secs(3600)).await;
        });
        queue.0.pop().unwrap().run();
        let start = Instant::now();
        dispatcher.shutdown();
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(task);
    }
}
