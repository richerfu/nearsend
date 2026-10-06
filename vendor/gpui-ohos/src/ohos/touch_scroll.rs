//! ArkUI pan recognition adapted to GPUI's public platform input interface.
//!
//! Raw XComponent contacts offer control capture, taps and long presses.
//! Only system-recognized Pan events move scroll containers. ArkUI supplies
//! their deltas and release velocity; there is no touch velocity estimator.

use std::time::{Duration, Instant};

use gpui::{
    DispatchEventResult, GestureTuning, LongPressEvent, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, Pixels, PlatformInput, Point, ScrollDelta, ScrollWheelEvent, TouchEvent, TouchId,
    TouchPhase, point, px,
};

const MAX_FLING_SPEED: f32 = 8000.;
const STOP_SPEED: f32 = 10.;
// ArkUI's API 12 friction motion: exp(-4.2 * 0.75 * seconds).
const DECELERATION: f32 = 4.2 * 0.75;

#[derive(Clone, Copy)]
enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    fn from_delta(delta: Point<Pixels>) -> Self {
        if delta.x.abs() > delta.y.abs() {
            Self::Horizontal
        } else {
            Self::Vertical
        }
    }

    fn lock(self, delta: &mut Point<Pixels>) {
        match self {
            Self::Horizontal => delta.y = px(0.),
            Self::Vertical => delta.x = px(0.),
        }
    }

    fn lock_velocity(self, velocity: &mut Point<f32>) {
        match self {
            Self::Horizontal => velocity.y = 0.,
            Self::Vertical => velocity.x = 0.,
        }
    }
}

pub(crate) struct NativePanInput {
    pub(crate) id: Option<TouchId>,
    pub(crate) phase: TouchPhase,
    /// Already converted from native device pixels to GPUI logical pixels.
    pub(crate) delta: Point<Pixels>,
    /// ArkUI release velocity, converted once from px/s to logical px/s.
    pub(crate) velocity: Point<f32>,
}

enum ContactKind {
    Pending {
        deadline: Instant,
        long_press_offered: bool,
    },
    CoreDrag,
    Pan {
        axis: Option<Axis>,
    },
    PanFinished,
    LongPress,
}

struct Contact {
    event: TouchEvent,
    start: Point<Pixels>,
    kind: ContactKind,
    caught: bool,
    // Raw Up and native Pan End may arrive in either order. Retain ownership
    // until both have finished, so neither a click nor a second fling leaks.
    raw_ended: bool,
}

struct Momentum {
    position: Point<Pixels>,
    direction: Point<f32>,
    speed: f32,
    started_at: Instant,
    duration: Duration,
    emitted_distance: f32,
}

struct LastTap {
    position: Point<Pixels>,
    time: Instant,
    count: usize,
}

#[derive(Default)]
pub(crate) struct TouchScroll {
    tuning: GestureTuning,
    contact: Option<Contact>,
    momentum: Option<Momentum>,
    last_tap: Option<LastTap>,
}

impl TouchScroll {
    /// Raw input only offers direct control capture and click/long-press
    /// compatibility. It never synthesizes a pan or computes fling velocity.
    pub(crate) fn relay(
        &mut self,
        event: TouchEvent,
        now: Instant,
        mut emit: impl FnMut(PlatformInput) -> DispatchEventResult,
    ) -> bool {
        if event.phase == TouchPhase::Started {
            if self
                .contact
                .as_ref()
                .is_some_and(|contact| contact.raw_ended)
            {
                self.cancel_contact(&mut emit);
            }
            // Secondary fingers cannot take over the primary capture.
            if self.contact.is_some() {
                return false;
            }
            let caught = self
                .momentum
                .take()
                .map(|momentum| {
                    emit(scroll(
                        momentum.position,
                        Point::default(),
                        TouchPhase::Ended,
                    ));
                })
                .is_some();
            let claimed = emit(PlatformInput::Touch(event.clone())).default_prevented;
            let kind = if claimed {
                ContactKind::CoreDrag
            } else {
                let mut cancel = event.clone();
                cancel.phase = TouchPhase::Cancelled;
                emit(PlatformInput::Touch(cancel));
                ContactKind::Pending {
                    deadline: now + self.tuning.long_press_duration,
                    long_press_offered: false,
                }
            };
            self.contact = Some(Contact {
                start: event.position,
                event,
                kind,
                caught,
                raw_ended: false,
            });
            return false;
        }

        let Some(mut contact) = self.contact.take() else {
            return false;
        };
        if contact.event.id != event.id {
            self.contact = Some(contact);
            return false;
        }
        let ended = matches!(event.phase, TouchPhase::Ended | TouchPhase::Cancelled);
        let mut tapped = false;
        match contact.kind {
            ContactKind::CoreDrag => {
                emit(PlatformInput::Touch(event.clone()));
                tapped = event.phase == TouchPhase::Ended
                    && !contact.caught
                    && (event.position - contact.start).magnitude()
                        <= f64::from(self.tuning.touch_slop);
            }
            ContactKind::Pending { .. } => {
                if event.phase == TouchPhase::Ended
                    && !contact.caught
                    && (event.position - contact.start).magnitude()
                        <= f64::from(self.tuning.touch_slop)
                {
                    let count = self
                        .last_tap
                        .as_ref()
                        .filter(|tap| {
                            now.saturating_duration_since(tap.time)
                                <= self.tuning.multi_tap_interval
                                && (event.position - tap.position).magnitude()
                                    <= f64::from(self.tuning.multi_tap_slop)
                        })
                        .map_or(1, |tap| tap.count.saturating_add(1));
                    self.last_tap = Some(LastTap {
                        position: event.position,
                        time: now,
                        count,
                    });
                    emit(PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position: event.position,
                        modifiers: Modifiers::default(),
                        click_count: count,
                        first_mouse: false,
                    }));
                    emit(PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: event.position,
                        modifiers: Modifiers::default(),
                        click_count: count,
                    }));
                    // Restore touch modality after compatibility mouse events.
                    let mut cancel = event.clone();
                    cancel.phase = TouchPhase::Cancelled;
                    emit(PlatformInput::Touch(cancel));
                    tapped = true;
                }
            }
            ContactKind::Pan { .. } | ContactKind::PanFinished => {}
            ContactKind::LongPress => {
                emit(PlatformInput::LongPress(LongPressEvent {
                    phase: event.phase,
                    start_position: contact.start,
                    position: event.position,
                }));
            }
        }
        contact.event = event;
        if !ended || matches!(contact.kind, ContactKind::Pan { .. }) {
            contact.raw_ended = ended;
            self.contact = Some(contact);
        }
        tapped
    }

    /// ArkUI owns acceptance, displacement and velocity. Its first accepted
    /// delta must move immediately; later packets are already incremental.
    pub(crate) fn native_pan(
        &mut self,
        input: NativePanInput,
        now: Instant,
        mut emit: impl FnMut(PlatformInput) -> DispatchEventResult,
    ) {
        let Some(mut contact) = self.contact.take() else {
            return;
        };
        if input.id.is_some_and(|id| id != contact.event.id) {
            self.contact = Some(contact);
            return;
        }
        match input.phase {
            TouchPhase::Started
                if matches!(contact.kind, ContactKind::Pending { .. }) && !contact.raw_ended =>
            {
                let axis = (input.delta != Point::default()).then(|| Axis::from_delta(input.delta));
                contact.kind = ContactKind::Pan { axis };
                let mut delta = input.delta;
                if let Some(axis) = axis {
                    axis.lock(&mut delta);
                }
                // Dropping Start loses movement and delays invalidation until
                // Update, which need not arrive for a short accepted gesture.
                emit(scroll(contact.start, delta, TouchPhase::Started));
            }
            TouchPhase::Moved | TouchPhase::Ended | TouchPhase::Cancelled => {
                if let ContactKind::Pan { axis } = &mut contact.kind {
                    if axis.is_none() && input.delta != Point::default() {
                        *axis = Some(Axis::from_delta(input.delta));
                    }
                    let mut delta = input.delta;
                    if let Some(axis) = axis {
                        axis.lock(&mut delta);
                    }
                    if input.phase == TouchPhase::Cancelled {
                        delta = Point::default();
                    }
                    emit(scroll(contact.start, delta, input.phase));
                    if input.phase == TouchPhase::Ended {
                        let mut velocity = input.velocity;
                        if let Some(axis) = axis {
                            axis.lock_velocity(&mut velocity);
                        }
                        self.start_momentum(contact.start, velocity, now);
                    }
                    if matches!(input.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                        contact.kind = ContactKind::PanFinished;
                    }
                }
            }
            _ => {}
        }
        if !contact.raw_ended || !matches!(contact.kind, ContactKind::PanFinished) {
            self.contact = Some(contact);
        }
    }

    fn start_momentum(&mut self, position: Point<Pixels>, velocity: Point<f32>, now: Instant) {
        let speed = velocity.x.hypot(velocity.y);
        if !speed.is_finite() || speed < self.tuning.min_fling_velocity {
            return;
        }
        let capped_speed = speed.min(MAX_FLING_SPEED);
        self.momentum = Some(Momentum {
            position,
            direction: point(velocity.x / speed, velocity.y / speed),
            speed: capped_speed,
            // Native velocity is already estimated by ArkUI. Start the curve
            // at dispatch time so queued delivery cannot skip its first frame.
            started_at: now,
            duration: Duration::from_secs_f32((capped_speed / STOP_SPEED).ln() / DECELERATION),
            emitted_distance: 0.,
        });
    }

    pub(crate) fn pending_long_press(&self, now: Instant) -> Option<(TouchId, Duration)> {
        let contact = self.contact.as_ref()?;
        if contact.raw_ended
            || (contact.event.position - contact.start).magnitude()
                > f64::from(self.tuning.touch_slop)
        {
            return None;
        }
        match contact.kind {
            ContactKind::Pending {
                deadline,
                long_press_offered: false,
            } => Some((contact.event.id, deadline.saturating_duration_since(now))),
            _ => None,
        }
    }

    pub(crate) fn offer_long_press(
        &mut self,
        id: TouchId,
        mut emit: impl FnMut(PlatformInput) -> DispatchEventResult,
    ) {
        if self.pending_long_press(Instant::now()).is_none() {
            return;
        }
        let Some(contact) = self
            .contact
            .as_mut()
            .filter(|contact| contact.event.id == id)
        else {
            return;
        };
        let ContactKind::Pending {
            long_press_offered, ..
        } = &mut contact.kind
        else {
            return;
        };
        if *long_press_offered {
            return;
        }
        *long_press_offered = true;
        if emit(PlatformInput::LongPress(LongPressEvent {
            phase: TouchPhase::Started,
            start_position: contact.start,
            position: contact.event.position,
        }))
        .default_prevented
        {
            contact.kind = ContactKind::LongPress;
        }
    }

    pub(crate) fn has_momentum(&self) -> bool {
        self.momentum.is_some()
    }

    /// Pan does not emit post-release deltas. GPUI painting still needs one
    /// window-owned inertia curve, advanced once per native VSync.
    pub(crate) fn tick(&mut self, now: Instant) -> Option<PlatformInput> {
        let momentum = self.momentum.as_mut()?;
        let elapsed = now
            .saturating_duration_since(momentum.started_at)
            .min(momentum.duration);
        let distance =
            momentum.speed / DECELERATION * (1. - (-DECELERATION * elapsed.as_secs_f32()).exp());
        let step = (distance - momentum.emitted_distance).max(0.);
        momentum.emitted_distance = distance.max(momentum.emitted_distance);
        let input = scroll(
            momentum.position,
            point(
                px(momentum.direction.x * step),
                px(momentum.direction.y * step),
            ),
            if elapsed >= momentum.duration {
                TouchPhase::Ended
            } else {
                TouchPhase::Moved
            },
        );
        if elapsed >= momentum.duration {
            self.momentum = None;
        }
        Some(input)
    }

    fn cancel_contact(&mut self, emit: &mut impl FnMut(PlatformInput) -> DispatchEventResult) {
        if let Some(mut contact) = self.contact.take() {
            match contact.kind {
                ContactKind::CoreDrag => {
                    contact.event.phase = TouchPhase::Cancelled;
                    emit(PlatformInput::Touch(contact.event));
                }
                ContactKind::Pan { .. } => {
                    emit(scroll(
                        contact.start,
                        Point::default(),
                        TouchPhase::Cancelled,
                    ));
                }
                ContactKind::LongPress => {
                    emit(PlatformInput::LongPress(LongPressEvent {
                        phase: TouchPhase::Cancelled,
                        start_position: contact.start,
                        position: contact.event.position,
                    }));
                }
                ContactKind::Pending { .. } | ContactKind::PanFinished => {}
            }
        }
    }

    pub(crate) fn cancel(&mut self, mut emit: impl FnMut(PlatformInput) -> DispatchEventResult) {
        self.cancel_contact(&mut emit);
        if let Some(momentum) = self.momentum.take() {
            emit(scroll(
                momentum.position,
                Point::default(),
                TouchPhase::Cancelled,
            ));
        }
    }
}

fn scroll(position: Point<Pixels>, delta: Point<Pixels>, touch_phase: TouchPhase) -> PlatformInput {
    PlatformInput::ScrollWheel(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Pixels(delta),
        modifiers: Modifiers::default(),
        touch_phase,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(id: u64, phase: TouchPhase, x: f32, y: f32) -> TouchEvent {
        TouchEvent {
            id: TouchId(id),
            phase,
            position: point(px(x), px(y)),
            predicted_position: None,
            force: None,
        }
    }

    fn raw(
        scroll: &mut TouchScroll,
        phase: TouchPhase,
        now: Instant,
        x: f32,
        y: f32,
    ) -> (bool, Vec<PlatformInput>) {
        let mut events = Vec::new();
        let tapped = scroll.relay(touch(1, phase, x, y), now, |event| {
            events.push(event);
            DispatchEventResult::default()
        });
        (tapped, events)
    }

    fn pan(
        scroll: &mut TouchScroll,
        phase: TouchPhase,
        delta: Point<Pixels>,
        velocity: Point<f32>,
        now: Instant,
    ) -> Vec<PlatformInput> {
        let mut events = Vec::new();
        scroll.native_pan(
            NativePanInput {
                id: Some(TouchId(1)),
                phase,
                delta,
                velocity,
            },
            now,
            |event| {
                events.push(event);
                DispatchEventResult::default()
            },
        );
        events
    }

    fn deltas(events: &[PlatformInput]) -> Vec<Point<Pixels>> {
        events
            .iter()
            .filter_map(|event| match event {
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    delta: ScrollDelta::Pixels(delta),
                    ..
                }) => Some(*delta),
                _ => None,
            })
            .collect()
    }

    fn start(scroll: &mut TouchScroll, now: Instant) {
        raw(scroll, TouchPhase::Started, now, 100., 300.);
        pan(
            scroll,
            TouchPhase::Started,
            Point::default(),
            Point::default(),
            now,
        );
    }

    fn fling(scroll: &mut TouchScroll, now: Instant, horizontal: bool) {
        start(scroll, now);
        pan(
            scroll,
            TouchPhase::Moved,
            if horizontal {
                point(px(-2.), px(0.))
            } else {
                point(px(0.), px(-2.))
            },
            Point::default(),
            now,
        );
        pan(
            scroll,
            TouchPhase::Ended,
            Point::default(),
            if horizontal {
                point(-600., 0.)
            } else {
                point(0., -600.)
            },
            now,
        );
        raw(scroll, TouchPhase::Ended, now, 100., 298.);
        assert!(scroll.has_momentum());
    }

    #[test]
    fn raw_movements_never_recognize_or_estimate_scroll() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        raw(&mut scroll, TouchPhase::Started, now, 100., 300.);
        for y in [299., 295., 280., 200.] {
            let (_, events) = raw(&mut scroll, TouchPhase::Moved, now, 100., y);
            assert!(deltas(&events).is_empty());
        }
        let (tapped, _) = raw(&mut scroll, TouchPhase::Ended, now, 100., 200.);
        assert!(!tapped);
        assert!(!scroll.has_momentum());
    }

    #[test]
    fn native_acceptance_moves_immediately_without_replaying_on_update() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        raw(&mut scroll, TouchPhase::Started, now, 100., 300.);
        let start = pan(
            &mut scroll,
            TouchPhase::Started,
            point(px(0.), px(-80.)),
            Point::default(),
            now,
        );
        assert_eq!(deltas(&start), [point(px(0.), px(-80.))]);
        for y in [-0.5, -2., 3.] {
            let events = pan(
                &mut scroll,
                TouchPhase::Moved,
                point(px(0.), px(y)),
                Point::default(),
                now,
            );
            assert_eq!(deltas(&events), [point(px(0.), px(y))]);
            let PlatformInput::ScrollWheel(event) = &events[0] else {
                panic!();
            };
            assert_eq!(event.position, point(px(100.), px(300.)));
        }
    }

    #[test]
    fn accepted_short_pan_moves_even_without_an_update_packet() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        raw(&mut scroll, TouchPhase::Started, now, 100., 300.);
        let accepted = pan(
            &mut scroll,
            TouchPhase::Started,
            point(px(0.25), px(-3.5)),
            Point::default(),
            now,
        );
        assert_eq!(deltas(&accepted), [point(px(0.), px(-3.5))]);
        let ended = pan(
            &mut scroll,
            TouchPhase::Ended,
            Point::default(),
            Point::default(),
            now,
        );
        assert_eq!(deltas(&ended), [Point::default()]);
        let (tapped, events) = raw(&mut scroll, TouchPhase::Ended, now, 100., 296.5);
        assert!(!tapped);
        assert!(events.is_empty());
        assert!(!scroll.has_momentum());
    }

    #[test]
    fn release_uses_system_velocity_and_dispatch_time() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        start(&mut scroll, now);
        pan(
            &mut scroll,
            TouchPhase::Moved,
            point(px(0.), px(-1.)),
            Point::default(),
            now,
        );
        let release = now + Duration::from_secs(2);
        pan(
            &mut scroll,
            TouchPhase::Ended,
            Point::default(),
            point(0., -250.),
            release,
        );
        assert_eq!(scroll.momentum.as_ref().unwrap().speed, 250.);
        let first = scroll.tick(release + Duration::from_millis(16)).unwrap();
        let delta = deltas(&[first])[0];
        assert!(delta.y < px(0.) && delta.y > px(-5.));
    }

    #[test]
    fn native_zero_velocity_after_hold_does_not_fling() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        start(&mut scroll, now);
        pan(
            &mut scroll,
            TouchPhase::Moved,
            point(px(0.), px(-100.)),
            point(0., -6000.),
            now,
        );
        pan(
            &mut scroll,
            TouchPhase::Ended,
            Point::default(),
            Point::default(),
            now + Duration::from_millis(500),
        );
        assert!(!scroll.has_momentum());
    }

    #[test]
    fn raw_up_and_native_end_work_in_either_order_without_clicks() {
        for up_first in [true, false] {
            let mut scroll = TouchScroll::default();
            let now = Instant::now();
            start(&mut scroll, now);
            if up_first {
                assert!(!raw(&mut scroll, TouchPhase::Ended, now, 100., 299.).0);
            }
            pan(
                &mut scroll,
                TouchPhase::Ended,
                Point::default(),
                point(0., -250.),
                now,
            );
            if !up_first {
                assert!(!raw(&mut scroll, TouchPhase::Ended, now, 100., 299.).0);
            }
            assert!(scroll.contact.is_none());
            assert!(scroll.has_momentum());
            pan(
                &mut scroll,
                TouchPhase::Ended,
                Point::default(),
                point(0., -8000.),
                now,
            );
            assert_eq!(scroll.momentum.as_ref().unwrap().speed, 250.);
        }
    }

    #[test]
    fn caught_fling_chooses_a_new_axis_and_never_clicks() {
        for horizontal in [true, false] {
            let mut scroll = TouchScroll::default();
            let now = Instant::now();
            fling(&mut scroll, now, horizontal);
            raw(&mut scroll, TouchPhase::Started, now, 100., 300.);
            assert!(!scroll.has_momentum());
            pan(
                &mut scroll,
                TouchPhase::Started,
                Point::default(),
                Point::default(),
                now,
            );
            let next = if horizontal {
                point(px(0.), px(-3.))
            } else {
                point(px(-3.), px(0.))
            };
            assert_eq!(
                deltas(&pan(
                    &mut scroll,
                    TouchPhase::Moved,
                    next,
                    Point::default(),
                    now
                )),
                [next]
            );
            assert!(!raw(&mut scroll, TouchPhase::Ended, now, 100., 300.).0);
            pan(
                &mut scroll,
                TouchPhase::Ended,
                Point::default(),
                Point::default(),
                now,
            );
            assert!(!scroll.has_momentum());
        }
    }

    #[test]
    fn catching_without_native_pan_never_clicks_or_reopens_keyboard() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        fling(&mut scroll, now, false);
        raw(&mut scroll, TouchPhase::Started, now, 100., 300.);
        let (tapped, events) = raw(&mut scroll, TouchPhase::Ended, now, 100., 300.);
        assert!(!tapped);
        assert!(events.is_empty());
        assert!(!scroll.has_momentum());
    }

    #[test]
    fn claimed_control_keeps_raw_drag_and_ignores_native_pan() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        scroll.relay(touch(1, TouchPhase::Started, 100., 300.), now, |_| {
            DispatchEventResult {
                default_prevented: true,
                ..Default::default()
            }
        });
        assert!(
            pan(
                &mut scroll,
                TouchPhase::Started,
                point(px(0.), px(-10.)),
                Point::default(),
                now
            )
            .is_empty()
        );
        let (_, events) = raw(&mut scroll, TouchPhase::Moved, now, 100., 200.);
        assert!(matches!(
            events.as_slice(),
            [PlatformInput::Touch(TouchEvent {
                phase: TouchPhase::Moved,
                ..
            })]
        ));
        assert!(
            pan(
                &mut scroll,
                TouchPhase::Ended,
                Point::default(),
                point(0., -1000.),
                now
            )
            .is_empty()
        );
        raw(&mut scroll, TouchPhase::Ended, now, 100., 200.);
        assert!(!scroll.has_momentum());
    }

    #[test]
    fn claimed_long_press_keeps_capture_and_stale_timer_is_ignored() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        raw(&mut scroll, TouchPhase::Started, now, 100., 300.);
        scroll.offer_long_press(TouchId(2), |_| panic!("wrong contact"));
        scroll.offer_long_press(TouchId(1), |_| DispatchEventResult {
            default_prevented: true,
            ..Default::default()
        });
        assert!(
            pan(
                &mut scroll,
                TouchPhase::Started,
                Point::default(),
                Point::default(),
                now
            )
            .is_empty()
        );
        let (_, events) = raw(&mut scroll, TouchPhase::Moved, now, 100., 200.);
        assert!(matches!(
            events.as_slice(),
            [PlatformInput::LongPress(LongPressEvent {
                phase: TouchPhase::Moved,
                ..
            })]
        ));
        raw(&mut scroll, TouchPhase::Ended, now, 100., 200.);
        scroll.offer_long_press(TouchId(1), |_| panic!("finished contact"));
    }

    #[test]
    fn declined_long_press_can_be_claimed_by_native_pan() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        raw(&mut scroll, TouchPhase::Started, now, 100., 300.);
        scroll.offer_long_press(TouchId(1), |_| DispatchEventResult::default());
        pan(
            &mut scroll,
            TouchPhase::Started,
            Point::default(),
            Point::default(),
            now,
        );
        assert_eq!(
            deltas(&pan(
                &mut scroll,
                TouchPhase::Moved,
                point(px(0.), px(-2.)),
                Point::default(),
                now
            )),
            [point(px(0.), px(-2.))]
        );
        assert!(scroll.pending_long_press(now).is_none());
    }

    #[test]
    fn secondary_finger_and_unstarted_native_updates_do_not_take_over() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        raw(&mut scroll, TouchPhase::Started, now, 100., 300.);
        assert!(
            pan(
                &mut scroll,
                TouchPhase::Moved,
                point(px(0.), px(-2.)),
                Point::default(),
                now
            )
            .is_empty()
        );
        scroll.relay(touch(2, TouchPhase::Started, 100., 300.), now, |_| {
            panic!("secondary start dispatched")
        });
        scroll.native_pan(
            NativePanInput {
                id: Some(TouchId(2)),
                phase: TouchPhase::Started,
                delta: Point::default(),
                velocity: Point::default(),
            },
            now,
            |_| panic!("secondary pan dispatched"),
        );
        assert!(matches!(
            scroll.contact.as_ref().unwrap().kind,
            ContactKind::Pending { .. }
        ));
    }

    #[test]
    fn taps_keep_double_click_count_and_touch_modality() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        for count in 1..=2 {
            raw(&mut scroll, TouchPhase::Started, now, 100., 300.);
            let (tapped, events) = raw(&mut scroll, TouchPhase::Ended, now, 102., 301.);
            assert!(tapped);
            let [
                PlatformInput::MouseDown(down),
                PlatformInput::MouseUp(up),
                PlatformInput::Touch(cancel),
            ] = events.as_slice()
            else {
                panic!();
            };
            assert_eq!(down.click_count, count);
            assert_eq!(up.click_count, count);
            assert_eq!(cancel.phase, TouchPhase::Cancelled);
        }
    }

    #[test]
    fn native_cancel_never_flings_and_raw_cancel_keeps_native_ownership() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        start(&mut scroll, now);
        raw(&mut scroll, TouchPhase::Cancelled, now, 100., 200.);
        assert!(scroll.contact.is_some());
        pan(
            &mut scroll,
            TouchPhase::Cancelled,
            point(px(0.), px(-2.)),
            point(0., -1000.),
            now,
        );
        assert!(scroll.contact.is_none());
        assert!(!scroll.has_momentum());
    }

    #[test]
    fn hiding_surface_cancels_contacts_and_every_future_momentum_frame() {
        let mut scroll = TouchScroll::default();
        let now = Instant::now();
        fling(&mut scroll, now, false);
        scroll.cancel(|_| DispatchEventResult::default());
        assert!(scroll.tick(now + Duration::from_secs(2)).is_none());
        raw(&mut scroll, TouchPhase::Started, now, 100., 300.);
        scroll.cancel(|_| DispatchEventResult::default());
        scroll.offer_long_press(TouchId(1), |_| panic!("cancelled long press"));
        assert!(
            pan(
                &mut scroll,
                TouchPhase::Started,
                Point::default(),
                Point::default(),
                now
            )
            .is_empty()
        );
    }

    #[test]
    fn inertia_distance_does_not_depend_on_frame_cadence() {
        let now = Instant::now();
        let mut totals = Vec::new();
        for interval in [16, 150] {
            let mut scroll = TouchScroll::default();
            fling(&mut scroll, now, false);
            let mut total = px(0.);
            for ms in (interval..=3000).step_by(interval as usize) {
                if let Some(event) = scroll.tick(now + Duration::from_millis(ms)) {
                    total += deltas(&[event])[0].y;
                }
            }
            totals.push(total);
            assert!(!scroll.has_momentum());
        }
        assert!((totals[0] - totals[1]).abs() < px(0.001));
    }

    #[test]
    fn invalid_or_tiny_velocity_is_ignored_and_system_velocity_is_capped() {
        let now = Instant::now();
        for velocity in [0., 1., f32::NAN, f32::INFINITY, -80000.] {
            let mut scroll = TouchScroll::default();
            start(&mut scroll, now);
            pan(
                &mut scroll,
                TouchPhase::Ended,
                Point::default(),
                point(0., velocity),
                now,
            );
            if velocity == -80000. {
                assert_eq!(scroll.momentum.as_ref().unwrap().speed, 8000.);
            } else {
                assert!(!scroll.has_momentum());
            }
        }
    }
}
