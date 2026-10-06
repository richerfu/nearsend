//! One atomic state orders native requests across render-session changes.
use std::sync::atomic::{AtomicU64, Ordering};
const ACTIVE: u64 = 1;
const REQUESTED: u64 = 2;
const PENDING: u64 = 4;
const FAILED: u64 = 8;
const FLAGS: u64 = 15;
const GENERATION: u64 = 16;
#[derive(Clone, Copy)]
pub(crate) struct FrameTicket(u64);
#[derive(Default)]
pub(crate) struct FrameRequest {
    state: AtomicU64,
}
impl FrameRequest {
    pub(crate) fn set_active(&self, active: bool) {
        let _ = self
            .state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                if (old & ACTIVE != 0) == active {
                    return None;
                }
                Some((old.wrapping_add(GENERATION) & !FLAGS) | if active { ACTIVE } else { 0 })
            });
    }
    pub(crate) fn request(&self) -> Option<FrameTicket> {
        self.state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                (old & ACTIVE != 0 && old & (REQUESTED | FAILED) == 0).then_some(old | REQUESTED)
            })
            .ok()
            .map(|old| FrameTicket(old & !FLAGS))
    }
    pub(crate) fn complete(&self, ticket: FrameTicket) -> bool {
        self.state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                (old & !FLAGS == ticket.0 && old & (ACTIVE | REQUESTED) == ACTIVE | REQUESTED)
                    .then_some((old & !REQUESTED) | PENDING)
            })
            .is_ok()
    }
    pub(crate) fn fail(&self, ticket: FrameTicket) {
        let _ = self
            .state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                (old & !FLAGS == ticket.0).then_some((old & !REQUESTED) | FAILED)
            });
    }
    pub(crate) fn failed(&self) -> bool {
        self.state.load(Ordering::Acquire) & FAILED != 0
    }
    pub(crate) fn take_pending(&self) -> bool {
        self.state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                (old & (ACTIVE | PENDING) == ACTIVE | PENDING).then_some(old & !PENDING)
            })
            .is_ok()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_late_callback_cannot_clear_the_resumed_request() {
        let state = FrameRequest::default();
        state.set_active(true);
        let old = state.request().unwrap();
        state.set_active(false);
        state.set_active(true);
        let resumed = state.request().unwrap();
        assert!(!state.complete(old));
        assert!(state.request().is_none());
        assert!(!state.take_pending());
        assert!(state.complete(resumed));
        assert!(state.take_pending());
        assert!(!state.take_pending());
    }
    #[test]
    fn bursts_coalesce_and_ready_and_requested_can_coexist() {
        let state = FrameRequest::default();
        state.set_active(true);
        let first = state.request().unwrap();
        for _ in 0..1000 {
            assert!(state.request().is_none());
        }
        assert!(state.complete(first));
        let next = state.request().unwrap();
        assert!(state.take_pending());
        assert!(state.request().is_none());
        assert!(state.complete(next));
        assert!(state.take_pending());
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
