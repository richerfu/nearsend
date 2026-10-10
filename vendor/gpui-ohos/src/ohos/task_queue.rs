//! Priority work queue with an explicit shutdown boundary.
use crate::Priority;
use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex},
    time::{Duration, Instant},
};

struct State<T> {
    queues: [VecDeque<T>; 3],
    cursor: usize,
    closed: bool,
    started: u64,
}

pub(crate) struct TaskQueue<T> {
    state: Mutex<State<T>>,
    ready: Condvar,
    changed: Condvar,
}

impl<T> Default for TaskQueue<T> {
    fn default() -> Self {
        Self {
            state: Mutex::new(State {
                queues: std::array::from_fn(|_| VecDeque::new()),
                cursor: 0,
                closed: false,
                started: 0,
            }),
            ready: Condvar::new(),
            changed: Condvar::new(),
        }
    }
}

impl<T> TaskQueue<T> {
    pub(crate) fn push(&self, priority: Priority, task: T) -> Result<(), T> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed {
            return Err(task);
        }
        let index = match priority {
            Priority::High => 0,
            Priority::Medium => 1,
            Priority::Low => 2,
            Priority::RealtimeAudio => unreachable!("realtime work has a dedicated thread"),
        };
        let was_empty = state.queues.iter().all(VecDeque::is_empty);
        state.queues[index].push_back(task);
        self.ready.notify_one();
        if was_empty {
            self.changed.notify_one();
        }
        Ok(())
    }

    fn take(state: &mut State<T>) -> Option<T> {
        // GPUI's priority weights are 60/30/10. FIFO service with the same
        // proportions also guarantees that low-priority work makes progress.
        const ORDER: [usize; 10] = [0, 0, 0, 0, 0, 0, 1, 1, 1, 2];
        for _ in 0..ORDER.len() {
            let index = ORDER[state.cursor];
            state.cursor = (state.cursor + 1) % ORDER.len();
            if let Some(task) = state.queues[index].pop_front() {
                state.started = state.started.wrapping_add(1);
                return Some(task);
            }
        }
        None
    }

    pub(crate) fn pop(&self) -> Option<T> {
        self.pop_until(None)
    }

    pub(crate) fn pop_timeout(&self, timeout: Duration) -> Option<T> {
        self.pop_until(Some(Instant::now() + timeout))
    }

    fn pop_until(&self, deadline: Option<Instant>) -> Option<T> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(task) = Self::take(&mut state) {
                return Some(task);
            }
            if state.closed {
                return None;
            }
            state = match deadline {
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return None;
                    }
                    self.ready
                        .wait_timeout(state, remaining)
                        .unwrap_or_else(|e| e.into_inner())
                        .0
                }
                None => self.ready.wait(state).unwrap_or_else(|e| e.into_inner()),
            };
        }
    }

    // The monitor sleeps when there is no backlog. Stalling requests an OS-state
    // observation; it does not by itself authorize another worker.
    pub(crate) fn wait_for_stalled_work(&self, interval: Duration) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            while !state.closed && state.queues.iter().all(VecDeque::is_empty) {
                state = self.changed.wait(state).unwrap_or_else(|e| e.into_inner());
            }
            if state.closed {
                return false;
            }
            let started = state.started;
            let (next, timeout) = self
                .changed
                .wait_timeout(state, interval)
                .unwrap_or_else(|e| e.into_inner());
            state = next;
            if state.closed {
                return false;
            }
            if timeout.timed_out()
                && started == state.started
                && state.queues.iter().any(|queue| !queue.is_empty())
            {
                return true;
            }
        }
    }

    pub(crate) fn close(&self) -> Vec<T> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        let remaining = state
            .queues
            .iter_mut()
            .flat_map(|queue| queue.drain(..))
            .collect();
        self.ready.notify_all();
        self.changed.notify_all();
        remaining
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn priorities_are_fifo_and_low_work_is_serviced() {
        let q = TaskQueue::default();
        for i in 0..6 {
            q.push(Priority::High, i).unwrap();
        }
        q.push(Priority::Medium, 10).unwrap();
        q.push(Priority::Medium, 11).unwrap();
        q.push(Priority::Low, 20).unwrap();
        let result: Vec<_> = (0..9).map(|_| q.pop().unwrap()).collect();
        assert_eq!(result, [0, 1, 2, 3, 4, 5, 10, 11, 20]);
    }
    #[test]
    fn closing_wakes_waiters_and_rejects_new_work() {
        let q = std::sync::Arc::new(TaskQueue::<u32>::default());
        let other = q.clone();
        let worker = std::thread::spawn(move || other.pop());
        assert!(q.close().is_empty());
        assert_eq!(worker.join().unwrap(), None);
        assert_eq!(q.push(Priority::Low, 7), Err(7));
    }
    #[test]
    fn an_idle_elastic_worker_can_retire_without_closing_the_queue() {
        let q = TaskQueue::default();
        assert_eq!(q.pop_timeout(Duration::from_millis(5)), None);
        q.push(Priority::Medium, 7).unwrap();
        assert_eq!(q.pop(), Some(7));
    }
    #[test]
    fn shutdown_wakes_the_idle_growth_monitor() {
        let q = std::sync::Arc::new(TaskQueue::<u32>::default());
        let other = q.clone();
        let monitor =
            std::thread::spawn(move || other.wait_for_stalled_work(Duration::from_secs(60)));
        assert!(q.close().is_empty());
        assert!(!monitor.join().unwrap());
    }
}
