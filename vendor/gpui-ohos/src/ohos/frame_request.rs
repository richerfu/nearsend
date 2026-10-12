//! A request ticket and its frame time share one lock across render sessions.
use std::{sync::Mutex, time::Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameTicket(u64);

#[derive(Default)]
struct State {
    generation: u64,
    active: bool,
    requested: Option<FrameTicket>,
    pending: Option<Instant>,
    failed: bool,
}

#[derive(Default)]
pub(crate) struct FrameRequest {
    state: Mutex<State>,
}

impl FrameRequest {
    pub(crate) fn set_active(&self, active: bool) {
        let mut state = self.state.lock().unwrap();
        if state.active != active {
            state.active = active;
            state.requested = None;
            state.pending = None;
            state.failed = false;
        }
    }

    pub(crate) fn request(&self) -> Option<FrameTicket> {
        let mut state = self.state.lock().unwrap();
        if !state.active || state.requested.is_some() || state.failed {
            return None;
        }
        state.generation = state.generation.wrapping_add(1);
        let ticket = FrameTicket(state.generation);
        state.requested = Some(ticket);
        Some(ticket)
    }

    pub(crate) fn complete(&self, ticket: FrameTicket, frame_time: Instant) -> bool {
        let mut state = self.state.lock().unwrap();
        if !state.active || state.requested != Some(ticket) {
            return false;
        }
        state.requested = None;
        // Replace queued ticks instead of replaying missed animation frames.
        // Timestamp and ticket validation must be published together: a late
        // callback from a hidden/destroyed surface cannot overwrite a new tick.
        state.pending = Some(state.pending.map_or(frame_time, |old| old.max(frame_time)));
        true
    }

    pub(crate) fn fail(&self, ticket: FrameTicket) {
        let mut state = self.state.lock().unwrap();
        if state.requested == Some(ticket) {
            state.requested = None;
            state.failed = true;
        }
    }

    pub(crate) fn failed(&self) -> bool {
        self.state.lock().unwrap().failed
    }

    pub(crate) fn take_pending(&self) -> Option<Instant> {
        self.state.lock().unwrap().pending.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_late_callback_cannot_overwrite_the_resumed_frame() {
        let state = FrameRequest::default();
        let now = Instant::now();
        state.set_active(true);
        let old = state.request().unwrap();
        state.set_active(false);
        state.set_active(true);
        let resumed = state.request().unwrap();
        assert!(!state.complete(old, now));
        assert!(state.request().is_none());
        assert_eq!(state.take_pending(), None);
        assert!(state.complete(resumed, now + Duration::from_millis(16)));
        assert!(!state.complete(old, now + Duration::from_secs(1)));
        assert_eq!(state.take_pending(), Some(now + Duration::from_millis(16)));
        assert_eq!(state.take_pending(), None);
    }

    #[test]
    fn bursts_coalesce_and_ready_and_requested_can_coexist() {
        let state = FrameRequest::default();
        let now = Instant::now();
        state.set_active(true);
        let first = state.request().unwrap();
        for _ in 0..1000 {
            assert!(state.request().is_none());
        }
        assert!(state.complete(first, now));
        let next = state.request().unwrap();
        assert_eq!(state.take_pending(), Some(now));
        assert!(state.request().is_none());
        assert!(state.complete(next, now + Duration::from_millis(8)));
        assert_eq!(state.take_pending(), Some(now + Duration::from_millis(8)));
    }

    #[test]
    fn a_busy_main_thread_consumes_only_the_latest_tick() {
        let state = FrameRequest::default();
        let now = Instant::now();
        state.set_active(true);
        for ms in [0, 8, 16, 24] {
            let ticket = state.request().unwrap();
            assert!(state.complete(ticket, now + Duration::from_millis(ms)));
        }
        assert_eq!(state.take_pending(), Some(now + Duration::from_millis(24)));
        assert_eq!(state.take_pending(), None);
    }

    #[test]
    fn duplicate_callbacks_cannot_complete_a_different_request() {
        let state = FrameRequest::default();
        state.set_active(true);
        let old = state.request().unwrap();
        let now = Instant::now();
        assert!(state.complete(old, now));
        let next = state.request().unwrap();
        assert!(!state.complete(old, now));
        state.fail(old);
        assert!(!state.failed());
        assert!(state.request().is_none());
        assert!(state.complete(next, now));
        state.set_active(false);
        assert_eq!(state.take_pending(), None);
    }

    #[test]
    fn failure_retries_only_after_a_new_render_session() {
        let state = FrameRequest::default();
        state.set_active(true);
        let failed = state.request().unwrap();
        state.fail(failed);
        assert!(state.failed());
        assert!(state.request().is_none());
        state.set_active(false);
        state.set_active(true);
        assert!(!state.failed());
        assert!(state.request().is_some());
        state.fail(failed);
        assert!(!state.failed());
    }
}
