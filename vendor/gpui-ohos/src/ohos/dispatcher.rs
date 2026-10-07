use super::task_queue::TaskQueue;
use crate::{PlatformDispatcher, Priority, PriorityQueueSender, RunnableVariant};
use openharmony_ability::OpenHarmonyWaker;
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, VecDeque},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
    },
    thread,
    time::{Duration, Instant},
};

struct TimerAfter {
    when: Instant,
    sequence: u64,
    runnable: RunnableVariant,
}
impl Ord for TimerAfter {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .when
            .cmp(&self.when)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}
impl PartialOrd for TimerAfter {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for TimerAfter {
    fn eq(&self, other: &Self) -> bool {
        self.when == other.when && self.sequence == other.sequence
    }
}
impl Eq for TimerAfter {}
#[derive(Default)]
struct TimerState {
    heap: BinaryHeap<TimerAfter>,
    next_sequence: u64,
    closed: bool,
}
#[derive(Default)]
pub(crate) struct MainWake {
    requested: AtomicBool,
    waker: Mutex<Option<OpenHarmonyWaker>>,
    stopped: AtomicBool,
    enqueue_gate: Mutex<()>,
}
impl MainWake {
    pub(crate) fn notify(&self) {
        if self.stopped.load(AtomicOrdering::Acquire)
            || self.requested.swap(true, AtomicOrdering::AcqRel)
        {
            return;
        }
        let waker = self.waker.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(waker) = waker {
            waker.wake();
        } else {
            self.requested.store(false, AtomicOrdering::Release);
        }
    }
}

pub(crate) struct OhosDispatcher {
    main_thread_id: thread::ThreadId,
    main_sender: PriorityQueueSender<RunnableVariant>,
    background: Arc<TaskQueue<RunnableVariant>>,
    timers: Arc<(Mutex<TimerState>, Condvar)>,
    ready_timers: Arc<Mutex<VecDeque<RunnableVariant>>>,
    wake: Arc<MainWake>,
}
impl OhosDispatcher {
    pub(crate) fn new(main_sender: PriorityQueueSender<RunnableVariant>) -> Self {
        let background = Arc::new(TaskQueue::<RunnableVariant>::default());
        super::workers::Workers::start(background.clone());
        let timers = Arc::new((Mutex::new(TimerState::default()), Condvar::new()));
        let ready_timers = Arc::new(Mutex::new(VecDeque::new()));
        let wake = Arc::new(MainWake::default());
        let timer_state = timers.clone();
        let ready = ready_timers.clone();
        let timer_wake = wake.clone();
        thread::Builder::new()
            .name("OhosTimer".into())
            .spawn(move || {
                let (lock, changed) = &*timer_state;
                loop {
                    let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
                    loop {
                        if state.closed {
                            return;
                        }
                        match state.heap.peek() {
                            Some(next) if next.when <= Instant::now() => break,
                            Some(next) => {
                                let timeout = next.when.saturating_duration_since(Instant::now());
                                state = changed
                                    .wait_timeout(state, timeout)
                                    .unwrap_or_else(|e| e.into_inner())
                                    .0;
                            }
                            None => state = changed.wait(state).unwrap_or_else(|e| e.into_inner()),
                        }
                    }
                    let mut target = ready.lock().unwrap_or_else(|e| e.into_inner());
                    let now = Instant::now();
                    while state.heap.peek().is_some_and(|next| next.when <= now) {
                        target.push_back(state.heap.pop().unwrap().runnable);
                    }
                    drop(target);
                    drop(state);
                    timer_wake.notify();
                }
            })
            .expect("Failed to start OHOS timer");
        Self {
            main_thread_id: thread::current().id(),
            main_sender,
            background,
            timers,
            ready_timers,
            wake,
        }
    }
    pub(crate) fn set_waker(&self, waker: OpenHarmonyWaker) {
        *self.wake.waker.lock().unwrap_or_else(|e| e.into_inner()) = Some(waker);
        self.wake.notify();
    }
    pub(crate) fn frame_waker(&self) -> Arc<MainWake> {
        self.wake.clone()
    }
    pub(crate) fn begin_main_turn(&self) {
        self.wake.requested.store(false, AtomicOrdering::Release);
    }
    pub(crate) fn wake_main_thread(&self) {
        self.wake.notify();
    }
    pub(crate) fn take_due_timer(&self) -> Option<RunnableVariant> {
        self.ready_timers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
    }
    #[cfg(test)]
    pub(crate) fn has_due_timers(&self) -> bool {
        !self
            .ready_timers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }
    pub(crate) fn execute_runnable(runnable: RunnableVariant) {
        runnable.run();
    }
    pub(crate) fn shutdown(&self) {
        {
            let _gate = self
                .wake
                .enqueue_gate
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if self.wake.stopped.swap(true, AtomicOrdering::AcqRel) {
                return;
            }
        }
        drop(self.background.close());
        let (lock, changed) = &*self.timers;
        let pending = {
            let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
            state.closed = true;
            let mut pending: Vec<_> = state.heap.drain().map(|entry| entry.runnable).collect();
            pending.extend(
                self.ready_timers
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .drain(..),
            );
            changed.notify_all();
            pending
        };
        // Timers can hold !Send foreground futures. Native session teardown runs on the UI thread.
        if self.is_main_thread() {
            drop(pending);
        } else {
            for runnable in pending {
                std::mem::forget(runnable);
            }
        }
        self.wake
            .waker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}
impl Drop for OhosDispatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}
impl PlatformDispatcher for OhosDispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.main_thread_id
    }
    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        if let Err(runnable) = self.background.push(priority, runnable) {
            drop(runnable);
        }
    }
    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) {
        let gate = self
            .wake
            .enqueue_gate
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if self.wake.stopped.load(AtomicOrdering::Acquire) {
            drop(gate);
            if self.is_main_thread() {
                drop(runnable);
            } else {
                std::mem::forget(runnable);
            }
            return;
        }
        let result = self.main_sender.send(priority, runnable);
        drop(gate);
        match result {
            Ok(()) => self.wake.notify(),
            Err(runnable) => std::mem::forget(runnable),
        }
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        let (lock, changed) = &*self.timers;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed {
            drop(state);
            if self.is_main_thread() {
                drop(runnable);
            } else {
                std::mem::forget(runnable);
            }
            return;
        }
        let sequence = state.next_sequence;
        state.next_sequence = sequence.wrapping_add(1);
        state.heap.push(TimerAfter {
            when: Instant::now() + duration,
            sequence,
            runnable,
        });
        changed.notify_one();
    }
    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        thread::spawn(f);
    }
    fn now(&self) -> Instant {
        Instant::now()
    }
}
