use log::{debug, warn};

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Instant,
};

use accesskit_ohos::Adapter as OhosA11yAdapter;
use anyhow::Result;
use futures::channel::oneshot;
use ohos_accessibility_binding::Provider;
use ohos_vsync_binding::Vsync;
use openharmony_ability::{
    ArkUiInputEvent, AvoidAreaType, Event, ImeEvent, InputEvent, OpenHarmonyApp, PointerInputData,
    XComponentInputEvent,
    arkui::arkui_input_binding::{UIInputAction, UIInputToolType},
    xcomponent::{
        MouseAction, MouseButton as OhosMouseButton, TouchEvent as OhosTouchEvent, TouchEventData,
        TouchPointData,
    },
};
use openharmony_ability_plugin_window::WindowClient;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use super::display::OhosDisplay;
use super::keyboard::OhosKeyState;
use super::platform::appearance_for_color_mode;
use super::touch_scroll::{NativePanInput, TouchScroll};
use super::wgpu_atlas::WgpuAtlas;
use super::wgpu_context::WgpuContext;
use super::wgpu_renderer::{WgpuRenderer, WgpuSurfaceConfig};
use crate::{
    A11yCallbacks, AnyWindowHandle, Bounds, Capslock, DevicePixels, Edges, ForegroundExecutor,
    GestureTuning, GpuSpecs, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    NavigationDirection, Pixels, PlatformAtlas, PlatformDisplay, PlatformInput,
    PlatformInputHandler, PlatformWindow, Point, PromptButton, PromptLevel, RequestFrameOptions,
    ResizeEdge, Scene, ScrollDelta, ScrollWheelEvent, Size, Task, TextInputStateChange, TouchEvent,
    TouchId, TouchPhase, WindowAppearance, WindowBackgroundAppearance, WindowBounds,
    WindowControlArea, WindowControls, WindowDecorations, WindowInsets, WindowParams,
    WindowVisibility, accesskit, point, px, size,
};
use openharmony_ability::FrameInputDelivery;

pub(crate) type BackHandler = Rc<RefCell<Option<Box<dyn FnMut()>>>>;
type ResizeCallback = Box<dyn FnMut(Size<Pixels>, f32)>;

pub(crate) struct OhosWindowContext {
    pub(crate) app: Rc<RefCell<Option<OpenHarmonyApp>>>,
    pub(crate) gpu_context: Arc<WgpuContext>,
    pub(crate) foreground_executor: ForegroundExecutor,
    pub(crate) frame_wake: Arc<super::dispatcher::MainWake>,
    pub(crate) cursor_hidden_until_move: Rc<Cell<bool>>,
    pub(crate) window_id: i64,
    pub(crate) fallback_atlas: Option<Arc<WgpuAtlas>>,
}

pub(crate) struct OhosWindow {
    app: Rc<RefCell<Option<OpenHarmonyApp>>>,
    pub(crate) handle: AnyWindowHandle,
    bounds: RefCell<Bounds<Pixels>>,
    viewport: super::viewport::ViewportPublisher,
    scale: RefCell<f32>,
    appearance: Cell<WindowAppearance>,
    window_id: i64,
    frame_scheduler: Option<Arc<FrameScheduler>>,
    closed: Cell<bool>,
    maximized: Rc<Cell<bool>>,
    fullscreen: Rc<Cell<bool>>,
    background_appearance: Rc<Cell<WindowBackgroundAppearance>>,
    active: Cell<bool>,
    hovered: Cell<bool>,
    visibility: Cell<WindowVisibility>,
    decorations: Rc<Cell<WindowDecorations>>,
    insets: RefCell<WindowInsets>,
    keyboard_overlap_device_px: Cell<i32>,
    input_handler: Rc<RefCell<Option<PlatformInputHandler>>>,
    callbacks: Rc<RefCell<WindowCallbacks>>,
    renderer: RefCell<Option<WgpuRenderer>>,
    surface_available: Cell<bool>,
    gpu_context: Arc<WgpuContext>,
    fallback_atlas: RefCell<Option<Arc<WgpuAtlas>>>,
    foreground_executor: ForegroundExecutor,
    cursor_hidden_until_move: Rc<Cell<bool>>,
    keyboard_visible: Rc<Cell<bool>>,
    pointer_position: Cell<Option<Point<Pixels>>>,
    pressed_mouse_button: Cell<Option<MouseButton>>,
    mouse_click: RefCell<Option<MouseClickState>>,
    back_enabled: Cell<bool>,
    back_handler: BackHandler,
    a11y_callbacks: RefCell<Option<Arc<Mutex<A11yCallbacks>>>>,
    a11y_adapter: RefCell<Option<OhosA11yAdapter<'static>>>,
    key_state: RefCell<OhosKeyState>,
    active_touches: RefCell<HashMap<i32, TouchId>>,
    touch_tap_candidates: RefCell<HashMap<i32, TouchTapCandidate>>,
    next_touch_id: Cell<u64>,
    touch_scroll: Rc<RefCell<TouchScroll>>,
    long_press_timer: RefCell<Option<(TouchId, Task<()>)>>,
}

/// GPUI invalidation requests a system VSync. The native callback only records
/// the tick; drawing stays on the Ability's main thread.
struct FrameScheduler {
    vsync: Vsync,
    state: Arc<super::frame_request::FrameRequest>,
    waker: Arc<super::dispatcher::MainWake>,
}
impl FrameScheduler {
    fn new(window_id: i64, waker: Arc<super::dispatcher::MainWake>) -> Option<Arc<Self>> {
        Some(Arc::new(Self {
            vsync: Vsync::try_new(format!("gpui-ohos-{window_id}"))?,
            state: Arc::new(super::frame_request::FrameRequest::default()),
            waker,
        }))
    }
    fn request_frame(&self) {
        let Some(ticket) = self.state.request() else {
            return;
        };
        let state = self.state.clone();
        let waker = self.waker.clone();
        let result = self.vsync.request_frame_once(move |_| {
            if state.complete(ticket) {
                waker.notify();
            }
        });
        if result != 0 {
            self.state.fail(ticket);
            self.waker.notify();
            warn!("Failed to request OHOS VSync frame: {result}");
        }
    }
    fn take_pending(&self) -> bool {
        self.state.take_pending()
    }
    fn set_active(&self, active: bool) {
        self.state.set_active(active);
        if active {
            self.request_frame();
        }
    }
}

pub(crate) struct OhosWindowHandle {
    inner: Rc<RefCell<OhosWindow>>,
    input_handler: Rc<RefCell<Option<PlatformInputHandler>>>,
}

impl Drop for OhosWindow {
    fn drop(&mut self) {
        self.release_accessibility();
        if self.window_id == 0 || self.closed.get() {
            return;
        }
        let Some(client) = self.window_client() else {
            return;
        };
        let window_id = self.window_id;
        self.foreground_executor
            .spawn(async move {
                if let Err(error) = client.destroy_window(window_id).await {
                    warn!("Failed to destroy OHOS sub-window {window_id}: {error}");
                }
            })
            .detach();
    }
}

impl OhosWindowHandle {
    pub(crate) fn new(inner: Rc<RefCell<OhosWindow>>) -> Self {
        let input_handler = inner.borrow().input_handler.clone();
        Self {
            inner,
            input_handler,
        }
    }

    fn with_window<R>(&self, f: impl FnOnce(&OhosWindow) -> R) -> R {
        let window = self.inner.borrow();
        f(&window)
    }

    fn with_window_mut<R>(&self, f: impl FnOnce(&mut OhosWindow) -> R) -> R {
        let mut window = self.inner.borrow_mut();
        f(&mut window)
    }
}

struct WindowCallbacks {
    request_frame: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    input: Option<Box<dyn FnMut(PlatformInput) -> crate::DispatchEventResult>>,
    active_status_change: Option<Box<dyn FnMut(bool)>>,
    visibility_change: Option<Box<dyn FnMut(WindowVisibility)>>,
    insets_changed: Option<Box<dyn FnMut(WindowInsets)>>,
    hover_status_change: Option<Box<dyn FnMut(bool)>>,
    resize: Option<ResizeCallback>,
    moved: Option<Box<dyn FnMut()>>,
    should_close: Option<Box<dyn FnMut() -> bool>>,
    close: Option<Box<dyn FnOnce()>>,
    appearance_changed: Option<Box<dyn FnMut()>>,
    hit_test_window_control: Option<Box<dyn FnMut() -> Option<WindowControlArea>>>,
}

#[derive(Clone, Copy)]
struct TouchTapCandidate {
    start_position: Point<Pixels>,
    started_in_text_input: bool,
}

struct MouseClickState {
    button: MouseButton,
    position: Point<Pixels>,
    time: Instant,
    count: usize,
}

struct OhosA11yActivation(Arc<Mutex<A11yCallbacks>>);

impl accesskit::ActivationHandler for OhosA11yActivation {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        self.0
            .lock()
            .ok()
            .and_then(|callbacks| (callbacks.activation)())
    }
}

struct OhosA11yAction(Arc<Mutex<A11yCallbacks>>);

impl accesskit::ActionHandler for OhosA11yAction {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        if let Ok(callbacks) = self.0.lock() {
            (callbacks.action)(request);
        }
    }
}

impl OhosWindow {
    fn initialize_accessibility(&self) {
        if self.a11y_adapter.borrow().is_some() {
            return;
        }
        let Some(callbacks) = self.a11y_callbacks.borrow().clone() else {
            return;
        };
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        let Some(provider_info) = app.with_xcomponent_for(self.window_id, |component| {
            let native = component.native_xcomponent();
            let provider = native
                .accessibility_provider()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let id = native
                .id()
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            Ok::<_, anyhow::Error>((provider.as_raw() as usize, id))
        }) else {
            return;
        };
        let (provider_raw, instance_id) = match provider_info {
            Ok(info) => info,
            Err(error) => {
                warn!("Failed to find OHOS accessibility provider: {error}");
                return;
            }
        };
        // The adapter is released on SurfaceDestroy, before the ability drops
        // the render XComponent that owns this provider.
        let provider: Provider<'static> =
            match unsafe { Provider::from_raw(provider_raw as *mut _) } {
                Ok(provider) => provider,
                Err(error) => {
                    warn!("Failed to retain OHOS accessibility provider: {error}");
                    return;
                }
            };
        match OhosA11yAdapter::new_with_instance(
            provider,
            &instance_id,
            OhosA11yActivation(callbacks.clone()),
            OhosA11yAction(callbacks),
        ) {
            Ok(adapter) => {
                if let Err(error) = adapter.set_host_focus_state(self.active.get()) {
                    warn!("Failed to set OHOS accessibility focus: {error}");
                }
                *self.a11y_adapter.borrow_mut() = Some(adapter);
            }
            Err(error) => warn!("Failed to register OHOS accessibility adapter: {error}"),
        }
    }

    fn release_accessibility(&self) {
        let adapter = self.a11y_adapter.borrow_mut().take();
        if adapter.is_some() {
            drop(adapter);
            if let Some(callbacks) = self.a11y_callbacks.borrow().as_ref()
                && let Ok(callbacks) = callbacks.lock()
            {
                (callbacks.deactivation)();
            }
        }
    }

    pub(crate) fn new(
        handle: AnyWindowHandle,
        params: WindowParams,
        context: OhosWindowContext,
    ) -> Result<Self> {
        let OhosWindowContext {
            app,
            gpu_context,
            foreground_executor,
            frame_wake,
            cursor_hidden_until_move,
            window_id,
            fallback_atlas,
        } = context;
        let scale = app.borrow().as_ref().map(|a| a.scale()).unwrap_or(1.0);
        let appearance = app
            .borrow()
            .as_ref()
            .map(|app| appearance_for_color_mode(app.config().color_mode))
            .unwrap_or_default();
        let bounds = params.bounds;
        let frame_scheduler = FrameScheduler::new(window_id, frame_wake);
        if frame_scheduler.is_none() {
            warn!("OHOS VSync is unavailable for window {window_id}");
        }

        // Don't create renderer immediately - native_window may not be available yet.
        // Renderer will be initialized lazily in draw() or when SurfaceCreate event is received.
        // At that point, native_window from OpenHarmonyApp will be available.

        Ok(Self {
            app: app.clone(),
            handle,
            bounds: RefCell::new(bounds),
            viewport: super::viewport::ViewportPublisher::default(),
            scale: RefCell::new(scale),
            appearance: Cell::new(appearance),
            window_id,
            frame_scheduler,
            closed: Cell::new(false),
            maximized: Rc::new(Cell::new(false)),
            fullscreen: Rc::new(Cell::new(false)),
            background_appearance: Rc::new(Cell::new(WindowBackgroundAppearance::Opaque)),
            active: Cell::new(window_id == 0),
            hovered: Cell::new(false),
            visibility: Cell::new(WindowVisibility::Visible),
            decorations: Rc::new(Cell::new(WindowDecorations::Server)),
            insets: RefCell::new(WindowInsets::default()),
            keyboard_overlap_device_px: Cell::new(0),
            input_handler: Rc::new(RefCell::new(None)),
            callbacks: Rc::new(RefCell::new(WindowCallbacks {
                request_frame: None,
                input: None,
                active_status_change: None,
                visibility_change: None,
                insets_changed: None,
                hover_status_change: None,
                resize: None,
                moved: None,
                should_close: None,
                close: None,
                appearance_changed: None,
                hit_test_window_control: None,
            })),
            renderer: RefCell::new(None),
            surface_available: Cell::new(false),
            gpu_context,
            fallback_atlas: RefCell::new(fallback_atlas),
            foreground_executor,
            cursor_hidden_until_move,
            keyboard_visible: Rc::new(Cell::new(false)),
            pointer_position: Cell::new(None),
            pressed_mouse_button: Cell::new(None),
            mouse_click: RefCell::new(None),
            back_enabled: Cell::new(false),
            back_handler: Rc::new(RefCell::new(None)),
            a11y_callbacks: RefCell::new(None),
            a11y_adapter: RefCell::new(None),
            key_state: RefCell::new(OhosKeyState::default()),
            active_touches: RefCell::new(HashMap::new()),
            touch_tap_candidates: RefCell::new(HashMap::new()),
            next_touch_id: Cell::new(0),
            touch_scroll: Rc::new(RefCell::new(TouchScroll::default())),
            long_press_timer: RefCell::new(None),
        })
    }

    pub(crate) fn atlas(&self) -> Option<Arc<WgpuAtlas>> {
        self.fallback_atlas.borrow().clone()
    }

    pub(crate) fn window_id(&self) -> i64 {
        self.window_id
    }

    pub(crate) fn take_pending_frame(&self) -> bool {
        self.sync_frame_delivery();
        !self.closed.get()
            && self.surface_available.get()
            && self.visibility.get() == WindowVisibility::Visible
            && self
                .frame_scheduler
                .as_ref()
                .is_some_and(|scheduler| scheduler.take_pending())
    }

    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        let scheduler = self.frame_scheduler.as_ref()?.clone();
        Some(Rc::new(move || scheduler.request_frame()))
    }

    fn sync_frame_delivery(&self) {
        let delivery = if self.surface_available.get()
            && self.visibility.get() == WindowVisibility::Visible
            && self
                .frame_scheduler
                .as_ref()
                .is_none_or(|scheduler| scheduler.state.failed())
        {
            FrameInputDelivery::Continuous
        } else {
            FrameInputDelivery::OnDemand
        };
        if let Some(app) = self.app.borrow().as_ref()
            && let Err(error) = app.set_frame_input_delivery_for(self.window_id, delivery)
        {
            warn!("Cannot configure OHOS frame delivery: {error}");
        }
    }

    pub(crate) fn draw_requested_frame(&self) {
        if self.closed.get()
            || !self.surface_available.get()
            || self.visibility.get() != WindowVisibility::Visible
        {
            return;
        }
        let momentum_input = self.touch_scroll.borrow_mut().tick(Instant::now());
        if let Some(input) = momentum_input {
            self.dispatch_input(input);
        }
        let mut callback = self.callbacks.borrow_mut().request_frame.take();
        if let Some(ref mut callback) = callback {
            callback(RequestFrameOptions {
                require_presentation: false,
                force_render: false,
            });
        }
        self.callbacks.borrow_mut().request_frame = callback;
        self.request_touch_momentum_frame();
    }

    pub(crate) fn apply_window_status(&self, status: i32) {
        // OHOS WindowStatusType: FULL_SCREEN=1, MAXIMIZE=2,
        // MINIMIZE=3, FLOATING=4, SPLIT_SCREEN=5.
        if !(1..=5).contains(&status) {
            warn!(
                "Unknown OHOS window status {status} for window {}",
                self.window_id
            );
            return;
        }
        self.maximized.set(status == 2);
        self.fullscreen.set(status == 1);
        self.update_visibility(if status == 3 {
            WindowVisibility::Hidden
        } else {
            WindowVisibility::Visible
        });
    }

    fn dispatch_input_with_callbacks(
        callbacks: &Rc<RefCell<WindowCallbacks>>,
        input: PlatformInput,
    ) -> crate::DispatchEventResult {
        let mut callback = callbacks.borrow_mut().input.take();
        let mut result = crate::DispatchEventResult::default();
        if let Some(ref mut cb) = callback {
            result = cb(input);
        }
        callbacks.borrow_mut().input = callback;
        result
    }

    fn point_from_device_pixels(&self, x: f32, y: f32) -> Point<Pixels> {
        let scale = (*self.scale.borrow()).max(f32::EPSILON);
        point(px(x / scale), px(y / scale))
    }

    fn pointer_position_from_arkui(&self, pointer: PointerInputData) -> Point<Pixels> {
        self.point_from_device_pixels(pointer.x, pointer.y)
    }

    fn allocate_touch_id(&self) -> TouchId {
        let raw_id = self.next_touch_id.get();
        self.next_touch_id
            .set(raw_id.checked_add(1).expect("touch ID exhausted"));
        TouchId(raw_id)
    }

    fn dispatch_raw_touch_point(&self, raw_id: i32, x: f32, y: f32, force: f32, phase: TouchPhase) {
        let position = self.point_from_device_pixels(x, y);
        let id = if phase == TouchPhase::Started {
            let id = self.allocate_touch_id();
            let mut active_touches = self.active_touches.borrow_mut();
            if active_touches.is_empty() {
                self.touch_tap_candidates.borrow_mut().insert(
                    raw_id,
                    TouchTapCandidate {
                        start_position: position,
                        // Capture this before GPUI translates the touch into its
                        // compatibility mouse gesture. A newly focused input will
                        // use the regular FocusGained path; only an input that was
                        // already focused needs an explicit IME reopen request.
                        started_in_text_input: self.pointer_targets_text_input(position),
                    },
                );
            } else {
                self.touch_tap_candidates.borrow_mut().clear();
            }
            active_touches.insert(raw_id, id);
            id
        } else {
            let Some(id) = self.active_touches.borrow().get(&raw_id).copied() else {
                return;
            };
            id
        };
        self.pointer_position.set(Some(position));
        let should_reopen_keyboard = match phase {
            TouchPhase::Moved => {
                if self.touch_moved_beyond_tap_slop(raw_id, position) {
                    self.touch_tap_candidates.borrow_mut().remove(&raw_id);
                }
                false
            }
            TouchPhase::Ended => self
                .touch_tap_candidates
                .borrow_mut()
                .remove(&raw_id)
                .is_some_and(|candidate| {
                    candidate.started_in_text_input
                        && (position - candidate.start_position).magnitude()
                            <= f64::from(GestureTuning::default().touch_slop)
                }),
            TouchPhase::Cancelled => {
                self.touch_tap_candidates.borrow_mut().remove(&raw_id);
                false
            }
            TouchPhase::Started => false,
        };
        let tap_allowed = self.touch_scroll.borrow_mut().relay(
            TouchEvent {
                id,
                phase,
                position,
                predicted_position: None,
                force: force.is_finite().then(|| force.clamp(0.0, 1.0)),
            },
            Instant::now(),
            |input| self.dispatch_input(input),
        );
        self.update_touch_long_press_timer();
        self.request_touch_momentum_frame();
        if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.active_touches.borrow_mut().remove(&raw_id);
        }
        if should_reopen_keyboard && tap_allowed {
            // Focusing an already-focused GPUI input is otherwise a no-op.
            // OHOS can dismiss its IME without changing that focus, so an
            // editable tap must always be treated as a fresh show request.
            // Do not inspect DispatchEventResult::default_prevented here:
            // focusable GPUI elements set it specifically to stop ancestors
            // from stealing focus.
            self.request_keyboard();
        }
    }

    fn request_touch_momentum_frame(&self) {
        if self.touch_scroll.borrow().has_momentum()
            && let Some(scheduler) = &self.frame_scheduler
        {
            scheduler.request_frame();
        }
    }

    fn update_touch_long_press_timer(&self) {
        let pending = self
            .touch_scroll
            .borrow()
            .pending_long_press(Instant::now());
        let Some((id, delay)) = pending else {
            self.long_press_timer.borrow_mut().take();
            return;
        };
        if self
            .long_press_timer
            .borrow()
            .as_ref()
            .is_some_and(|(scheduled, _)| *scheduled == id)
        {
            return;
        }
        let touch_scroll = self.touch_scroll.clone();
        let callbacks = self.callbacks.clone();
        let task = self.foreground_executor.spawn(async move {
            smol::Timer::after(delay).await;
            touch_scroll.borrow_mut().offer_long_press(id, |input| {
                Self::dispatch_input_with_callbacks(&callbacks, input)
            });
        });
        *self.long_press_timer.borrow_mut() = Some((id, task));
    }

    fn cancel_touch_input(&self) {
        self.long_press_timer.borrow_mut().take();
        self.touch_scroll
            .borrow_mut()
            .cancel(|input| self.dispatch_input(input));
        self.active_touches.borrow_mut().clear();
        self.touch_tap_candidates.borrow_mut().clear();
    }

    fn touch_moved_beyond_tap_slop(&self, raw_id: i32, position: Point<Pixels>) -> bool {
        self.touch_tap_candidates
            .borrow()
            .get(&raw_id)
            .is_some_and(|candidate| {
                (position - candidate.start_position).magnitude()
                    > f64::from(GestureTuning::default().touch_slop)
            })
    }

    fn pointer_targets_text_input(&self, position: Point<Pixels>) -> bool {
        let Some(mut handler) = self.input_handler.borrow_mut().take() else {
            return false;
        };
        let targets_text_input = handler.query_accepts_text_input()
            && handler
                .element_bounds()
                .is_some_and(|bounds| bounds.contains(&position));
        *self.input_handler.borrow_mut() = Some(handler);
        targets_text_input
    }

    fn dispatch_raw_touch_data(&self, event: &TouchEventData, point: &TouchPointData) {
        let phase = match event.event_type {
            OhosTouchEvent::Down => TouchPhase::Started,
            OhosTouchEvent::Move => TouchPhase::Moved,
            OhosTouchEvent::Up => TouchPhase::Ended,
            OhosTouchEvent::Cancel => TouchPhase::Cancelled,
            OhosTouchEvent::Unknown => return,
        };
        self.dispatch_raw_touch_point(point.id, point.x, point.y, point.force, phase);
    }

    fn dispatch_raw_touch_event(&self, event: &TouchEventData) {
        match event.event_type {
            OhosTouchEvent::Move | OhosTouchEvent::Cancel if !event.touch_points.is_empty() => {
                for point in &event.touch_points {
                    self.dispatch_raw_touch_data(event, point);
                }
            }
            OhosTouchEvent::Down | OhosTouchEvent::Up => {
                if let Some(point) = event.touch_points.iter().find(|point| point.id == event.id) {
                    self.dispatch_raw_touch_data(event, point);
                } else {
                    let phase = if event.event_type == OhosTouchEvent::Down {
                        TouchPhase::Started
                    } else {
                        TouchPhase::Ended
                    };
                    self.dispatch_raw_touch_point(event.id, event.x, event.y, event.force, phase);
                }
            }
            OhosTouchEvent::Move | OhosTouchEvent::Cancel => {
                let phase = if event.event_type == OhosTouchEvent::Move {
                    TouchPhase::Moved
                } else {
                    TouchPhase::Cancelled
                };
                self.dispatch_raw_touch_point(event.id, event.x, event.y, event.force, phase);
            }
            OhosTouchEvent::Unknown => {}
        }
    }

    fn dispatch_scroll(
        &self,
        position: Point<Pixels>,
        delta: Point<Pixels>,
        touch_phase: TouchPhase,
    ) -> crate::DispatchEventResult {
        Self::dispatch_input_with_callbacks(
            &self.callbacks,
            PlatformInput::ScrollWheel(ScrollWheelEvent {
                position,
                delta: ScrollDelta::Pixels(delta),
                modifiers: self.key_state.borrow().modifiers(),
                touch_phase,
            }),
        )
    }

    fn mouse_button(button: OhosMouseButton) -> Option<MouseButton> {
        match button {
            OhosMouseButton::NoneButton => None,
            OhosMouseButton::LeftButton => Some(MouseButton::Left),
            OhosMouseButton::RightButton => Some(MouseButton::Right),
            OhosMouseButton::MiddleButton => Some(MouseButton::Middle),
            OhosMouseButton::BackButton => Some(MouseButton::Navigate(NavigationDirection::Back)),
            OhosMouseButton::ForwardButton => {
                Some(MouseButton::Navigate(NavigationDirection::Forward))
            }
        }
    }

    fn show_keyboard_if_needed(&self) {
        if !self.keyboard_visible.get() {
            self.request_keyboard();
        }
    }

    fn request_keyboard(&self) {
        if let Some(app) = self.app.borrow().as_ref() {
            app.show_keyboard_for(self.window_id);
            self.keyboard_visible.set(true);
        }
    }

    fn hide_keyboard_if_needed(&self) {
        if self.keyboard_visible.replace(false)
            && let Some(app) = self.app.borrow().as_ref()
        {
            app.hide_keyboard_for(self.window_id);
        }
    }

    fn notify_keyboard_hidden_by_user_if_needed(&self) {
        self.keyboard_visible.set(false);
    }

    fn keyboard_inset_for_overlap(&self, overlap_device_px: i32) -> Pixels {
        const MIN_CONTENT_HEIGHT: f32 = 64.0;

        let overlap = overlap_device_px.max(0) as f32;
        let scale = self.scale_factor().max(1.0);
        let mut inset = (overlap / scale).max(0.0);
        let bounds_height = self.bounds.borrow().size.height.as_f32().max(0.0);
        let max_inset = (bounds_height - MIN_CONTENT_HEIGHT).max(0.0);
        if inset > max_inset {
            inset = max_inset;
        }
        px(inset)
    }

    fn keyboard_overlap_from_avoid_area_device_px(&self) -> Option<i32> {
        let app_ref = self.app.borrow();
        let app = app_ref.as_ref()?;

        let content_rect = app.content_rect_for(self.window_id);
        if content_rect.height <= 0 {
            return Some(0);
        }

        // Use actual XComponent rect as layout basis for keyboard-avoid computation.
        // This keeps behavior correct for embedded/non-fullscreen XComponents.
        let layout_top = content_rect.top;
        let layout_height = content_rect.height.max(0);
        if layout_height <= 0 {
            return Some(0);
        }
        let window_rect = app.window_rect_for(self.window_id);
        let window_top = window_rect.top;
        let window_bottom = window_rect.top.saturating_add(window_rect.height.max(0));

        let keyboard_area = app.avoid_area_for(self.window_id, AvoidAreaType::Keyboard);
        let system_area = app.avoid_area_for(self.window_id, AvoidAreaType::System);
        let system_gesture_area = app.avoid_area_for(self.window_id, AvoidAreaType::SystemGesture);
        let navigation_indicator_area =
            app.avoid_area_for(self.window_id, AvoidAreaType::NavigationIndicator);

        // OHOS avoid-area bottomRect coordinates are in window/screen space.
        // XComponent's content_rect can be reported in safe-content coordinates on some devices.
        // For root full-width layouts, infer top-safe offset so intersection uses a consistent space.
        let root_layout_width_matches_window = content_rect.width > 0
            && window_rect.width > 0
            && (content_rect.width - window_rect.width).abs() <= 1;
        let can_infer_root_safe_top = layout_top == 0
            && layout_height > 0
            && window_rect.height >= layout_height
            && root_layout_width_matches_window;
        let inferred_outside_bottom_safe = if can_infer_root_safe_top {
            let bottom_safe_overlap = |area: Option<openharmony_ability::AvoidArea>| -> i32 {
                let Some(area) = area else {
                    return 0;
                };
                if !area.visible || area.bottom_rect.height <= 0 {
                    return 0;
                }
                let start = area.bottom_rect.top;
                let end = area
                    .bottom_rect
                    .top
                    .saturating_add(area.bottom_rect.height.max(0));
                if end < window_bottom {
                    return 0;
                }
                (window_bottom - start)
                    .max(0)
                    .min(area.bottom_rect.height.max(0))
            };

            bottom_safe_overlap(system_area)
                .max(bottom_safe_overlap(system_gesture_area))
                .max(bottom_safe_overlap(navigation_indicator_area))
        } else {
            0
        };
        let inferred_top_safe = if can_infer_root_safe_top {
            (window_rect.height.max(0) - layout_height - inferred_outside_bottom_safe).max(0)
        } else {
            0
        };
        // Convert GPUI layout bounds to screen space before intersection.
        let layout_top_screen = window_top
            .saturating_add(inferred_top_safe)
            .saturating_add(layout_top);
        let layout_bottom_screen = layout_top_screen.saturating_add(layout_height);

        let keyboard_avoid_visible = keyboard_area.map(|a| a.visible).unwrap_or(false);
        // A show request does not prove that an on-screen keyboard occupies
        // the window. Without keyboard geometry, system/navigation bars are
        // only safe-area insets; counting them here shrinks the viewport twice.
        if !keyboard_area.is_some_and(|area| area.bottom_rect.height > 0) {
            return Some(0);
        }
        if !(self.keyboard_visible.get() || keyboard_avoid_visible) {
            return Some(0);
        }

        // Keyboard event only determines show/hide state.
        // Actual inset is derived from avoid-area geometry.
        // When keyboard is shown, include bottom occlusion union of:
        // - Keyboard area
        // - System bottom area (3-button navigation etc.)
        // - System gesture area
        // - Navigation indicator area
        // This prevents under-subtraction where keyboard area excludes nav area.
        let mut intervals: Vec<(i32, i32)> = Vec::with_capacity(4);
        let mut push_bottom_overlap_interval =
            |area: openharmony_ability::AvoidArea, require_visible: bool| {
                if area.bottom_rect.height <= 0 {
                    return;
                }
                if require_visible && !area.visible {
                    return;
                }
                let start = area.bottom_rect.top.max(layout_top_screen);
                let end = area
                    .bottom_rect
                    .top
                    .saturating_add(area.bottom_rect.height.max(0))
                    .min(layout_bottom_screen);
                if end > start {
                    intervals.push((start, end));
                }
            };

        if let Some(area) = keyboard_area {
            push_bottom_overlap_interval(area, true);
        }
        if let Some(area) = system_area {
            push_bottom_overlap_interval(area, false);
        }
        if let Some(area) = system_gesture_area {
            push_bottom_overlap_interval(area, false);
        }
        if let Some(area) = navigation_indicator_area {
            push_bottom_overlap_interval(area, false);
        }

        if intervals.is_empty() {
            return Some(0);
        }

        intervals.sort_unstable_by_key(|(start, _)| *start);
        let mut union_overlap = 0i32;
        let mut current = intervals[0];
        for &(start, end) in intervals.iter().skip(1) {
            if start <= current.1 {
                current.1 = current.1.max(end);
            } else {
                union_overlap = union_overlap.saturating_add(current.1 - current.0);
                current = (start, end);
            }
        }
        union_overlap = union_overlap.saturating_add(current.1 - current.0);

        let geometric_overlap = union_overlap.min(layout_height.max(0));
        let clamped_overlap = geometric_overlap;

        Some(clamped_overlap)
    }

    fn refresh_keyboard_overlap_device_px(&self) -> bool {
        let previous_overlap = self.keyboard_overlap_device_px.get();
        let next_overlap = self
            .keyboard_overlap_from_avoid_area_device_px()
            .unwrap_or(0)
            .max(0);
        if previous_overlap != next_overlap {
            self.keyboard_overlap_device_px.set(next_overlap);
            true
        } else {
            false
        }
    }

    fn current_safe_area_insets(&self) -> WindowInsets {
        let Some(app) = self.app.borrow().clone() else {
            return WindowInsets::default();
        };
        let content = app.content_rect_for(self.window_id);
        let window = app.window_rect_for(self.window_id);
        let mut top = 0_i32;
        let mut right = 0_i32;
        let mut bottom = 0_i32;
        let mut left = 0_i32;
        for kind in [
            AvoidAreaType::System,
            AvoidAreaType::Cutout,
            AvoidAreaType::NavigationIndicator,
        ] {
            if let Some(area) = app.avoid_area_for(self.window_id, kind)
                && area.visible
            {
                top = top.max(area.top_rect.height.max(0));
                right = right.max(area.right_rect.width.max(0));
                bottom = bottom.max(area.bottom_rect.height.max(0));
                left = left.max(area.left_rect.width.max(0));
            }
        }

        let scale = self.scale_factor().max(1.0);
        let applied_top = content.top.max(0);
        let applied_left = content.left.max(0);
        let applied_right = (window.width - content.width - content.left).max(0);
        let applied_bottom = (window.height - content.height - content.top).max(0);
        WindowInsets {
            safe_area: Edges {
                top: px((top - applied_top).max(0) as f32 / scale),
                right: px((right - applied_right).max(0) as f32 / scale),
                bottom: px((bottom - applied_bottom).max(0) as f32 / scale),
                left: px((left - applied_left).max(0) as f32 / scale),
            },
            // Keyboard overlap already reduces content_size in this backend.
            ime: Edges::default(),
        }
    }

    fn refresh_insets(&self) {
        let next = self.current_safe_area_insets();
        if *self.insets.borrow() == next {
            return;
        }
        *self.insets.borrow_mut() = next.clone();
        let mut callback = self.callbacks.borrow_mut().insets_changed.take();
        if let Some(ref mut callback) = callback {
            callback(next);
        }
        self.callbacks.borrow_mut().insets_changed = callback;
    }

    fn update_visibility(&self, next: WindowVisibility) {
        if next != WindowVisibility::Visible {
            self.cancel_touch_input();
        }
        if let Some(scheduler) = &self.frame_scheduler {
            scheduler.set_active(next == WindowVisibility::Visible && self.surface_available.get());
        }
        let changed = self.visibility.replace(next) != next;
        self.sync_frame_delivery();
        if !changed {
            return;
        }
        let mut callback = self.callbacks.borrow_mut().visibility_change.take();
        if let Some(ref mut callback) = callback {
            callback(next);
        }
        self.callbacks.borrow_mut().visibility_change = callback;
    }

    pub(crate) fn back_handler_state(&self) -> (bool, BackHandler) {
        (self.back_enabled.get(), self.back_handler.clone())
    }

    fn set_hovered(&self, hovered: bool) {
        if self.hovered.replace(hovered) == hovered {
            return;
        }
        let mut callback = self.callbacks.borrow_mut().hover_status_change.take();
        if let Some(ref mut callback) = callback {
            callback(hovered);
        }
        self.callbacks.borrow_mut().hover_status_change = callback;
    }

    fn window_client(&self) -> Option<WindowClient> {
        let app = self.app.borrow().clone()?;
        match WindowClient::new(&app) {
            Ok(client) => Some(client),
            Err(error) => {
                warn!("Cannot access OHOS window bridge: {error}");
                None
            }
        }
    }

    fn request_resize(&self, size: Size<Pixels>) {
        let Some(client) = self.window_client() else {
            return;
        };
        let scale = self.scale_factor();
        let width = (size.width.as_f32() * scale).round() as i64;
        let height = (size.height.as_f32() * scale).round() as i64;
        if width <= 0 || height <= 0 {
            return;
        }
        let window_id = self.window_id;
        self.foreground_executor
            .spawn(async move {
                if let Err(error) = client.resize_window(window_id, width, height).await {
                    warn!("Failed to resize OHOS window {window_id}: {error}");
                }
            })
            .detach();
    }

    fn restore_cursor_after_move(&self) {
        if !self.cursor_hidden_until_move.replace(false) {
            return;
        }
        let Some(client) = self.window_client() else {
            return;
        };
        self.foreground_executor
            .spawn(async move {
                if let Err(error) = client.set_cursor_visible(true).await {
                    warn!("Failed to restore OHOS cursor: {error}");
                }
            })
            .detach();
    }

    fn effective_content_size(&self) -> Size<Pixels> {
        let bounds_size = self.bounds.borrow().size;
        let bounds_height = bounds_size.height.as_f32().max(0.0);
        let keyboard_inset = self
            .keyboard_inset_for_overlap(self.keyboard_overlap_device_px.get())
            .as_f32();
        size(
            bounds_size.width,
            px((bounds_height - keyboard_inset).max(0.0)),
        )
    }

    fn emit_resize_callback(&self) {
        let scale = *self.scale.borrow();
        let content_size = self.effective_content_size();
        let mut callback = self.callbacks.borrow_mut().resize.take();
        self.viewport.publish(content_size, scale, &mut callback);
        self.callbacks.borrow_mut().resize = callback;
    }

    /// Initialize the renderer when native_window becomes available (after SurfaceCreate event).
    /// This method gets the raw_window_handle from OpenHarmonyApp's native_window.
    pub(crate) fn initialize_renderer(&self) -> Result<()> {
        let mut renderer_guard = self.renderer.borrow_mut();
        if renderer_guard.is_some() {
            // Already initialized
            return Ok(());
        }

        // Get native_window from OpenHarmonyApp - it should be available after SurfaceCreate
        let app = self.app.borrow();
        let app_ref = app.as_ref().ok_or_else(|| {
            anyhow::anyhow!("OpenHarmonyApp not available when initializing renderer")
        })?;

        // Check that native_window is available - this is required for the renderer to work.
        // The actual window handle is obtained via HasWindowHandle trait implementation.
        let _native_window = app_ref.native_window_for(self.window_id).ok_or_else(|| {
            anyhow::anyhow!(
                "native_window not available yet - SurfaceCreate event may not have been received"
            )
        })?;

        // Get the actual window size from content_rect.
        // Using the correct size is important because mismatched sizes between
        // the surface configuration and the actual native_window can cause
        // rendering issues (stretched/cropped content, black borders, etc.)
        // even though create_platform_window_surface itself won't fail.
        let content_rect = app_ref.content_rect_for(self.window_id);
        let scale = app_ref.scale();
        *self.scale.borrow_mut() = scale;
        let device_width = if content_rect.width > 0 {
            content_rect.width as u32
        } else {
            // Fallback to bounds if content_rect is not available yet
            (self.bounds.borrow().size.width.as_f32() * scale) as u32
        };
        let device_height = if content_rect.height > 0 {
            content_rect.height as u32
        } else {
            (self.bounds.borrow().size.height.as_f32() * scale) as u32
        };

        debug!(
            "OhosWindow: Initializing renderer with size {}x{}",
            device_width, device_height
        );

        // Update window bounds to match actual content_rect (convert device px -> logical px)
        if content_rect.width > 0 && content_rect.height > 0 {
            let logical_size = size(
                px(device_width as f32 / scale),
                px(device_height as f32 / scale),
            );
            let logical_origin = point(
                px(content_rect.left as f32 / scale),
                px(content_rect.top as f32 / scale),
            );
            *self.bounds.borrow_mut() = Bounds::new(logical_origin, logical_size);
        }

        let config = WgpuSurfaceConfig {
            size: Size {
                width: DevicePixels(device_width as i32),
                height: DevicePixels(device_height as i32),
            },
            transparent: true,
        };

        debug!(
            "OhosWindow: Surface config - width: {}, height: {}, transparent: true",
            device_width, device_height
        );

        // Debug: Check window handle before creating renderer
        match self.window_handle() {
            Ok(handle) => {
                debug!(
                    "OhosWindow: Window handle obtained successfully: {:?}",
                    handle.as_raw()
                );
            }
            Err(e) => {
                warn!("OhosWindow: Failed to get window handle: {:?}", e);
                return Err(anyhow::anyhow!("Window handle not available: {:?}", e));
            }
        }

        debug!("OhosWindow: Creating WgpuRenderer...");

        // Create renderer using the window's HasWindowHandle and HasDisplayHandle implementation
        // which will get the raw_window_handle from native_window
        let renderer = WgpuRenderer::new(&self.gpu_context, self, config, self.fallback_atlas.borrow().clone())
            .map_err(|e| {
                warn!("OhosWindow: WgpuRenderer::new failed: {}", e);
                anyhow::anyhow!("Failed to create Wgpu renderer: {}. Make sure native_window is available from OpenHarmonyApp.", e)
            })?;

        *self.fallback_atlas.borrow_mut() = Some(renderer.sprite_atlas().clone());
        *renderer_guard = Some(renderer);
        debug!("OhosWindow: Renderer initialized successfully");
        Ok(())
    }

    pub(crate) fn handle_event(&self, event: &Event) {
        match event {
            Event::SurfaceCreate => {
                self.surface_available.set(true);
                if let Some(scheduler) = &self.frame_scheduler {
                    scheduler.set_active(self.visibility.get() == WindowVisibility::Visible);
                }
                self.sync_frame_delivery();
                debug!("OhosWindow: SurfaceCreate event received - initializing renderer");
                self.initialize_accessibility();
                // Initialize renderer when SurfaceCreate event is received
                // Note: on_finish_launching is handled at the platform level (OhosPlatform::handle_ohos_event)
                // before windows are created.
                match self.initialize_renderer() {
                    Ok(()) => {
                        debug!("OhosWindow: Renderer initialized successfully");
                    }
                    Err(e) => {
                        warn!(
                            "OhosWindow: Failed to initialize renderer: {}. Make sure native_window is available from OpenHarmonyApp.",
                            e
                        );
                    }
                }
                self.refresh_keyboard_overlap_device_px();
                self.emit_resize_callback();
                self.refresh_insets();
            }
            Event::SurfaceDestroy => {
                self.cancel_touch_input();
                self.surface_available.set(false);
                if let Some(scheduler) = &self.frame_scheduler {
                    scheduler.set_active(false);
                }
                self.release_accessibility();
                self.renderer.borrow_mut().take();
                self.set_hovered(false);
            }
            Event::WindowResize {
                window_id,
                size: ohos_size,
            } if *window_id == self.window_id => {
                // openharmony-ability currently maps both the ArkTS windowSizeChange callback and
                // the XComponent surface callback to WindowResize. In a floating 2-in-1 window the
                // former includes the server-side title bar, while the native render surface does
                // not. Prefer the active XComponent rect whenever it is available so that the
                // renderer and GPUI viewport always use the drawable content size.
                let content_rect = self
                    .app
                    .borrow()
                    .as_ref()
                    .map(|app| app.content_rect_for(self.window_id))
                    .unwrap_or_default();
                let device_width = if content_rect.width > 0 {
                    content_rect.width
                } else {
                    ohos_size.width
                };
                let device_height = if content_rect.height > 0 {
                    content_rect.height
                } else {
                    ohos_size.height
                };
                if device_width != ohos_size.width || device_height != ohos_size.height {
                    debug!(
                        "OhosWindow: Normalizing window resize {}x{} to XComponent surface {}x{}",
                        ohos_size.width, ohos_size.height, device_width, device_height,
                    );
                }
                let scale = *self.scale.borrow();
                let width = device_width as f32;
                let height = device_height as f32;
                let new_size = size(px(width / scale), px(height / scale));
                let origin = self.bounds.borrow().origin;
                *self.bounds.borrow_mut() = Bounds::new(origin, new_size);
                self.refresh_keyboard_overlap_device_px();

                // Update renderer's drawable size
                if let Some(ref mut renderer) = *self.renderer.borrow_mut() {
                    let device_size = Size {
                        width: DevicePixels(width as i32),
                        height: DevicePixels(height as i32),
                    };
                    renderer.update_drawable_size(device_size);
                }
                self.emit_resize_callback();
                self.refresh_insets();
            }
            Event::ContentRectChange(info) if info.window_id == self.window_id => {
                let scale = self.scale_factor();
                let content_rect = self
                    .app
                    .borrow()
                    .as_ref()
                    .map(|app| app.content_rect_for(self.window_id))
                    .unwrap_or_default();
                let next_origin = point(
                    px((info.rect.left + content_rect.left) as f32 / scale),
                    px((info.rect.top + content_rect.top) as f32 / scale),
                );
                let mut bounds = self.bounds.borrow_mut();
                let moved = bounds.origin != next_origin;
                bounds.origin = next_origin;
                drop(bounds);
                if moved {
                    let mut callback = self.callbacks.borrow_mut().moved.take();
                    if let Some(ref mut callback) = callback {
                        callback();
                    }
                    self.callbacks.borrow_mut().moved = callback;
                }
                if self.refresh_keyboard_overlap_device_px() {
                    self.emit_resize_callback();
                }
                self.refresh_insets();
            }
            Event::AvoidAreaChange(info) => {
                if matches!(
                    info.area_type,
                    AvoidAreaType::Keyboard
                        | AvoidAreaType::System
                        | AvoidAreaType::SystemGesture
                        | AvoidAreaType::NavigationIndicator
                ) && self.refresh_keyboard_overlap_device_px()
                {
                    self.emit_resize_callback();
                }
                self.refresh_insets();
            }
            Event::Start => self.update_visibility(WindowVisibility::Visible),
            Event::Stop => self.update_visibility(WindowVisibility::Hidden),
            Event::WindowRedraw(_) => {
                // The platform event loop already drains pending GPUI VSync
                // frames. Drawing again for XComponent's continuous callback
                // can advance momentum and submit a second frame in one VSync.
                if self
                    .frame_scheduler
                    .as_ref()
                    .is_none_or(|scheduler| scheduler.state.failed())
                {
                    self.draw_requested_frame();
                }
            }
            Event::Input(input_event) => {
                self.handle_input_event(input_event);
            }
            Event::WindowFocusChanged { window_id, focused } => {
                let next = *focused && *window_id == self.window_id;
                if let Some(adapter) = self.a11y_adapter.borrow().as_ref()
                    && let Err(error) = adapter.set_host_focus_state(next)
                {
                    warn!("Failed to update OHOS accessibility focus: {error}");
                }
                if self.active.replace(next) != next {
                    let mut callback = self.callbacks.borrow_mut().active_status_change.take();
                    if let Some(ref mut cb) = callback {
                        cb(next);
                    }
                    self.callbacks.borrow_mut().active_status_change = callback;
                }
                if !next {
                    self.set_hovered(false);
                    self.pressed_mouse_button.set(None);
                    self.mouse_click.borrow_mut().take();
                    let modifiers_changed = self.key_state.borrow_mut().clear_pressed();
                    if let Some(event) = modifiers_changed {
                        self.dispatch_input(event);
                    }
                    self.hide_keyboard_if_needed();
                    if self.refresh_keyboard_overlap_device_px() {
                        self.emit_resize_callback();
                    }
                }
            }
            Event::ConfigChanged(configuration) => {
                let appearance = appearance_for_color_mode(configuration.color_mode);
                if self.appearance.replace(appearance) != appearance {
                    let mut callback = self.callbacks.borrow_mut().appearance_changed.take();
                    if let Some(ref mut callback) = callback {
                        callback();
                    }
                    self.callbacks.borrow_mut().appearance_changed = callback;
                }
                let new_scale = self.app.borrow().as_ref().map(|a| a.scale()).unwrap_or(1.0);
                let old_scale = self.scale.replace(new_scale);
                if old_scale != new_scale {
                    let mut bounds = self.bounds.borrow_mut();
                    let ratio = old_scale / new_scale;
                    bounds.size = size(bounds.size.width * ratio, bounds.size.height * ratio);
                    bounds.origin *= ratio;
                }
                self.refresh_keyboard_overlap_device_px();
                self.emit_resize_callback();
                self.refresh_insets();
            }
            Event::WindowDestroy => {
                if self.closed.replace(true) {
                    return;
                }
                if let Some(scheduler) = &self.frame_scheduler {
                    scheduler.set_active(false);
                }
                self.update_visibility(WindowVisibility::Hidden);
                self.active.set(false);
                self.set_hovered(false);
                if self.refresh_keyboard_overlap_device_px() {
                    self.emit_resize_callback();
                }
                // The native window has already been destroyed. A close veto
                // cannot restore it, so always release GPUI's window state.
                if let Some(callback) = self.callbacks.borrow_mut().close.take() {
                    callback();
                }
            }
            Event::KeyboardEvent(height) => {
                if *height <= 0 {
                    self.notify_keyboard_hidden_by_user_if_needed();
                } else {
                    self.keyboard_visible.set(true);
                }
                if self.refresh_keyboard_overlap_device_px() {
                    self.emit_resize_callback();
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_input_event(&self, event: &InputEvent) {
        match event {
            InputEvent::Ime(ime_event) => {
                if matches!(
                    ime_event,
                    ImeEvent::ImeStatusEvent(openharmony_ability::ime::KeyboardStatus::Hide)
                ) {
                    self.notify_keyboard_hidden_by_user_if_needed();
                    if self.refresh_keyboard_overlap_device_px() {
                        self.emit_resize_callback();
                    }
                }

                let handler_ref = self.input_handler.clone();
                let ime_event = ime_event.clone();
                let executor = self.foreground_executor.clone();

                executor
                    .spawn(async move {
                        let mut handler_guard = handler_ref.borrow_mut();
                        let Some(handler) = handler_guard.as_mut() else {
                            return;
                        };

                        match ime_event {
                            ImeEvent::TextInputEvent(data) => {
                                handler.replace_text_in_range(None, &data.text);
                                handler.unmark_text();
                            }
                            ImeEvent::PreviewTextEvent { text, start, end } => {
                                let range = (start >= 0 && end >= start)
                                    .then_some(start as usize..end as usize);
                                handler.replace_and_mark_text_in_range(range, &text, None);
                            }
                            ImeEvent::FinishPreviewEvent => handler.unmark_text(),
                            ImeEvent::EnterEvent(_action) => {
                                handler.replace_text_in_range(None, "\n");
                                handler.unmark_text();
                            }
                            ImeEvent::BackspaceEvent(len) => {
                                let len = (len).max(0) as usize;
                                if len == 0 {
                                    return;
                                }

                                if let Some(selection) = handler.selected_text_range(true) {
                                    let range = if selection.range.start != selection.range.end {
                                        selection.range
                                    } else {
                                        let caret = if selection.reversed {
                                            selection.range.start
                                        } else {
                                            selection.range.end
                                        };
                                        let start = caret.saturating_sub(len);
                                        start..caret
                                    };
                                    handler.replace_text_in_range(Some(range), "");
                                } else {
                                    handler.replace_text_in_range(None, "");
                                }
                            }
                            ImeEvent::ImeStatusEvent(status) => {
                                if matches!(status, openharmony_ability::ime::KeyboardStatus::Hide)
                                {
                                    handler.unmark_text();
                                }
                            }
                        }
                    })
                    .detach();
            }
            InputEvent::XComponent(XComponentInputEvent::Mouse(mouse_event)) => {
                let position = self.point_from_device_pixels(mouse_event.x, mouse_event.y);
                self.pointer_position.set(Some(position));
                let event_button = Self::mouse_button(mouse_event.button);
                match mouse_event.action {
                    MouseAction::Press => {
                        let Some(button) = event_button else {
                            return;
                        };
                        let now = Instant::now();
                        let tuning = GestureTuning::default();
                        let mut click = self.mouse_click.borrow_mut();
                        let click_count = click
                            .as_ref()
                            .filter(|last| {
                                last.button == button
                                    && now.duration_since(last.time) <= tuning.multi_tap_interval
                                    && (position - last.position).magnitude()
                                        <= f64::from(tuning.multi_tap_slop)
                            })
                            .map_or(1, |last| last.count.saturating_add(1));
                        *click = Some(MouseClickState {
                            button,
                            position,
                            time: now,
                            count: click_count,
                        });
                        drop(click);
                        self.pressed_mouse_button.set(Some(button));
                        self.dispatch_input(PlatformInput::MouseDown(MouseDownEvent {
                            button,
                            position,
                            modifiers: self.key_state.borrow().modifiers(),
                            click_count,
                            first_mouse: false,
                        }));
                    }
                    MouseAction::Release => {
                        let button = event_button.or(self.pressed_mouse_button.get());
                        self.pressed_mouse_button.set(None);
                        let Some(button) = button else {
                            return;
                        };
                        self.dispatch_input(PlatformInput::MouseUp(MouseUpEvent {
                            button,
                            position,
                            modifiers: self.key_state.borrow().modifiers(),
                            click_count: self
                                .mouse_click
                                .borrow()
                                .as_ref()
                                .filter(|click| click.button == button)
                                .map_or(1, |click| click.count),
                        }));
                    }
                    MouseAction::Move => {
                        self.restore_cursor_after_move();
                        self.dispatch_input(PlatformInput::MouseMove(MouseMoveEvent {
                            position,
                            pressed_button: self.pressed_mouse_button.get().or(event_button),
                            modifiers: self.key_state.borrow().modifiers(),
                        }));
                    }
                    MouseAction::None => {}
                }
            }
            InputEvent::XComponent(XComponentInputEvent::Hover(hovered)) => {
                self.set_hovered(*hovered);
            }
            InputEvent::ArkUi(ArkUiInputEvent::Axis(axis_event)) => {
                let position = self.pointer_position_from_arkui(axis_event.pointer);
                self.pointer_position.set(Some(position));
                let raw_delta = point(axis_event.delta_x as f32, axis_event.delta_y as f32);
                let delta = if axis_event.pointer.tool_type == UIInputToolType::Touchpad {
                    self.point_from_device_pixels(raw_delta.x, raw_delta.y)
                } else {
                    raw_delta.map(px)
                };
                let touch_phase = match axis_event.pointer.action {
                    UIInputAction::Down => TouchPhase::Started,
                    UIInputAction::Up | UIInputAction::Cancel => TouchPhase::Ended,
                    UIInputAction::Move => TouchPhase::Moved,
                };
                if delta.x != px(0.0)
                    || delta.y != px(0.0)
                    || matches!(touch_phase, TouchPhase::Started | TouchPhase::Ended)
                {
                    self.dispatch_scroll(position, delta, touch_phase);
                }
            }
            InputEvent::ArkUi(ArkUiInputEvent::Gesture(
                openharmony_ability::GestureEvent::Pan(pan),
            )) => {
                let phase = match pan.phase {
                    openharmony_ability::GesturePhase::Start => TouchPhase::Started,
                    openharmony_ability::GesturePhase::Update => TouchPhase::Moved,
                    openharmony_ability::GesturePhase::End => TouchPhase::Ended,
                    openharmony_ability::GesturePhase::Cancel => TouchPhase::Cancelled,
                };
                let id = pan
                    .pointer
                    .pointer_id
                    .and_then(|id| self.active_touches.borrow().get(&id).copied());
                let delta = self.point_from_device_pixels(pan.delta_x, pan.delta_y);
                let velocity = self
                    .point_from_device_pixels(pan.velocity_x, pan.velocity_y)
                    .map(f32::from);
                self.touch_scroll.borrow_mut().native_pan(
                    NativePanInput {
                        id,
                        phase,
                        delta,
                        velocity,
                    },
                    Instant::now(),
                    |input| self.dispatch_input(input),
                );
                self.update_touch_long_press_timer();
                self.request_touch_momentum_frame();
            }
            // Raw contacts retain click/long-press and control capture support.
            // Pan End already contains ArkUI's velocity; Swipe must not launch
            // a second momentum curve for the same contact.
            InputEvent::ArkUi(ArkUiInputEvent::Gesture(_)) => {}
            InputEvent::XComponent(XComponentInputEvent::Touch(touch_event)) => {
                self.dispatch_raw_touch_event(touch_event);
            }
            InputEvent::XComponent(XComponentInputEvent::Key(key_event)) => {
                let inputs = self.key_state.borrow_mut().handle(key_event);
                for input in inputs {
                    self.dispatch_input(input);
                }
            }
        }
    }

    fn dispatch_input(&self, input: PlatformInput) -> crate::DispatchEventResult {
        Self::dispatch_input_with_callbacks(&self.callbacks, input)
    }
}

impl HasWindowHandle for OhosWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.app
            .borrow()
            .as_ref()
            .and_then(|app| app.native_window_for(self.window_id))
            .and_then(|native_window| native_window.raw_window_handle())
            .map(|raw_handle| unsafe { raw_window_handle::WindowHandle::borrow_raw(raw_handle) })
            .ok_or(raw_window_handle::HandleError::Unavailable)
    }
}

impl HasWindowHandle for OhosWindowHandle {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let window = self.inner.borrow();
        window
            .app
            .borrow()
            .as_ref()
            .and_then(|app| app.native_window_for(window.window_id))
            .and_then(|native_window| native_window.raw_window_handle())
            .map(|raw_handle| unsafe { raw_window_handle::WindowHandle::borrow_raw(raw_handle) })
            .ok_or(raw_window_handle::HandleError::Unavailable)
    }
}

impl HasDisplayHandle for OhosWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(raw_window_handle::DisplayHandle::ohos())
    }
}

impl HasDisplayHandle for OhosWindowHandle {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(raw_window_handle::DisplayHandle::ohos())
    }
}

impl PlatformWindow for OhosWindowHandle {
    fn insets(&self) -> WindowInsets {
        self.with_window(|window| window.insets())
    }

    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.with_window(|window| window.on_insets_changed(callback))
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.with_window(|window| window.bounds())
    }

    fn is_maximized(&self) -> bool {
        self.with_window(|window| window.is_maximized())
    }

    fn window_bounds(&self) -> WindowBounds {
        self.with_window(|window| window.window_bounds())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.with_window(|window| window.content_size())
    }

    fn resize(&mut self, size: Size<Pixels>) {
        self.with_window(|window| window.request_resize(size))
    }

    fn scale_factor(&self) -> f32 {
        self.with_window(|window| window.scale_factor())
    }

    fn appearance(&self) -> WindowAppearance {
        self.with_window(|window| window.appearance())
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.with_window(|window| window.display())
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.with_window(|window| window.mouse_position())
    }

    fn modifiers(&self) -> Modifiers {
        self.with_window(|window| window.modifiers())
    }

    fn capslock(&self) -> Capslock {
        self.with_window(|window| window.capslock())
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        *self.input_handler.borrow_mut() = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.borrow_mut().take()
    }

    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        self.with_window(|window| window.prompt(level, msg, detail, answers))
    }

    fn activate(&self) {
        self.with_window(|window| window.activate())
    }

    fn is_active(&self) -> bool {
        self.with_window(|window| window.is_active())
    }

    fn is_hovered(&self) -> bool {
        self.with_window(|window| window.is_hovered())
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.with_window(|window| window.background_appearance())
    }

    fn set_title(&mut self, title: &str) {
        self.with_window_mut(|window| window.set_title(title))
    }

    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        self.with_window(|window| window.set_background_appearance(background_appearance))
    }

    fn minimize(&self) {
        self.with_window(|window| window.minimize())
    }

    fn zoom(&self) {
        self.with_window(|window| window.zoom())
    }

    fn toggle_fullscreen(&self) {
        self.with_window(|window| window.toggle_fullscreen())
    }

    fn is_fullscreen(&self) -> bool {
        self.with_window(|window| window.is_fullscreen())
    }

    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        self.with_window(OhosWindow::frame_waker)
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.with_window(|window| window.on_request_frame(callback))
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> crate::DispatchEventResult>) {
        self.with_window(|window| window.on_input(callback))
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.with_window(|window| window.on_active_status_change(callback))
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.with_window(|window| window.on_hover_status_change(callback))
    }

    fn set_back_handler(&self, callback: Box<dyn FnMut()>) {
        self.with_window(|window| window.set_back_handler(callback))
    }

    fn set_back_enabled(&self, enabled: bool) {
        self.with_window(|window| window.set_back_enabled(enabled))
    }

    fn a11y_init(&self, callbacks: A11yCallbacks) {
        self.with_window(|window| window.a11y_init(callbacks))
    }

    fn a11y_tree_update(&self, update: accesskit::TreeUpdate) {
        self.with_window(|window| window.a11y_tree_update(update))
    }

    fn visibility(&self) -> WindowVisibility {
        self.with_window(|window| window.visibility())
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.with_window(|window| window.on_visibility_change(callback))
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.with_window(|window| window.on_resize(callback))
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.with_window(|window| window.on_moved(callback))
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.with_window(|window| window.on_should_close(callback))
    }

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.with_window(|window| window.on_hit_test_window_control(callback))
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.with_window(|window| window.on_close(callback))
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.with_window(|window| window.on_appearance_changed(callback))
    }

    fn draw(&self, scene: &Scene) {
        self.with_window(|window| window.draw(scene))
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.with_window(|window| window.sprite_atlas())
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.with_window(|window| window.gpu_specs())
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        self.with_window(|window| window.is_subpixel_rendering_supported())
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        self.with_window(|window| window.update_ime_position(bounds))
    }

    fn show_soft_keyboard(&self) {
        self.with_window(|window| window.show_soft_keyboard())
    }

    fn hide_soft_keyboard(&self) {
        self.with_window(|window| window.hide_soft_keyboard())
    }

    fn text_input_state_changed(&self, change: TextInputStateChange) {
        self.with_window(|window| window.text_input_state_changed(change))
    }
}

impl PlatformWindow for OhosWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        *self.bounds.borrow()
    }

    fn is_maximized(&self) -> bool {
        self.maximized.get()
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(*self.bounds.borrow())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.effective_content_size()
    }

    fn resize(&mut self, size: Size<Pixels>) {
        self.request_resize(size);
    }

    fn scale_factor(&self) -> f32 {
        *self.scale.borrow()
    }

    fn appearance(&self) -> WindowAppearance {
        self.appearance.get()
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        if let Some(app) = self.app.borrow().clone() {
            Some(Rc::new(OhosDisplay::new(app)))
        } else {
            None
        }
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.pointer_position
            .get()
            .unwrap_or_else(|| point(px(0.0), px(0.0)))
    }

    fn modifiers(&self) -> Modifiers {
        self.key_state.borrow().modifiers()
    }

    fn capslock(&self) -> Capslock {
        self.key_state.borrow().capslock()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        *self.input_handler.borrow_mut() = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.borrow_mut().take()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        None
    }

    fn activate(&self) {
        let Some(client) = self.window_client() else {
            return;
        };
        let window_id = self.window_id;
        self.foreground_executor
            .spawn(async move {
                if let Err(error) = client.focus_window(window_id).await {
                    warn!("Failed to focus OHOS window: {error}");
                }
            })
            .detach();
    }

    fn is_active(&self) -> bool {
        self.active.get()
    }

    fn visibility(&self) -> WindowVisibility {
        self.visibility.get()
    }

    fn is_hovered(&self) -> bool {
        self.hovered.get()
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.background_appearance.get()
    }

    fn set_title(&mut self, title: &str) {
        let Some(client) = self.window_client() else {
            return;
        };
        let window_id = self.window_id;
        let title = title.to_owned();
        self.foreground_executor
            .spawn(async move {
                if let Err(error) = client.set_window_title(window_id, title).await {
                    warn!("Failed to set OHOS window title: {error}");
                }
            })
            .detach();
    }

    fn set_background_appearance(&self, appearance: WindowBackgroundAppearance) {
        // The default Opaque appearance does not specify a color. Keep the
        // native window's system background until an actual transition occurs.
        if self.background_appearance.get() == appearance {
            return;
        }
        let Some(client) = self.window_client() else {
            return;
        };
        let window_id = self.window_id;
        let current = self.background_appearance.clone();
        let window_appearance = self.appearance.get();
        self.foreground_executor
            .spawn(async move {
                let color = if appearance == WindowBackgroundAppearance::Opaque {
                    if matches!(
                        window_appearance,
                        WindowAppearance::Dark | WindowAppearance::VibrantDark
                    ) {
                        0xff000000
                    } else {
                        0xffffffff
                    }
                } else {
                    0x00000000
                };
                let result = client.set_window_background_color(window_id, color).await;
                match result {
                    Ok(()) => {
                        if appearance == WindowBackgroundAppearance::Blurred {
                            warn!("OHOS GPUI surface does not support backdrop blur; using transparency");
                            current.set(WindowBackgroundAppearance::Transparent);
                        } else {
                            current.set(appearance);
                        }
                    }
                    Err(error) => warn!("Failed to set OHOS window background: {error}"),
                }
            })
            .detach();
    }

    fn minimize(&self) {
        let Some(client) = self.window_client() else {
            return;
        };
        let window_id = self.window_id;
        self.foreground_executor
            .spawn(async move {
                if let Err(error) = client.minimize_window(window_id).await {
                    warn!("Failed to minimize OHOS window: {error}");
                }
            })
            .detach();
    }

    fn zoom(&self) {
        let Some(client) = self.window_client() else {
            return;
        };
        let window_id = self.window_id;
        let state = self.maximized.clone();
        self.foreground_executor
            .spawn(async move {
                let next = !state.get();
                let result = if next {
                    client.maximize_window(window_id).await
                } else {
                    client.restore_window(window_id).await
                };
                match result {
                    Ok(()) => state.set(next),
                    Err(error) => warn!("Failed to zoom OHOS window: {error}"),
                }
            })
            .detach();
    }

    fn toggle_fullscreen(&self) {
        let Some(client) = self.window_client() else {
            return;
        };
        let window_id = self.window_id;
        let state = self.fullscreen.clone();
        self.foreground_executor
            .spawn(async move {
                let next = !state.get();
                match client.set_fullscreen(window_id, next).await {
                    Ok(()) => state.set(next),
                    Err(error) => warn!("Failed to set OHOS fullscreen: {error}"),
                }
            })
            .detach();
    }

    fn is_fullscreen(&self) -> bool {
        self.fullscreen.get()
    }

    fn frame_waker(&self) -> Option<Rc<dyn Fn()>> {
        OhosWindow::frame_waker(self)
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.callbacks.borrow_mut().request_frame = Some(callback);
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> crate::DispatchEventResult>) {
        self.callbacks.borrow_mut().input = Some(callback);
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.callbacks.borrow_mut().active_status_change = Some(callback);
    }

    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.callbacks.borrow_mut().hover_status_change = Some(callback);
    }

    fn set_back_handler(&self, callback: Box<dyn FnMut()>) {
        *self.back_handler.borrow_mut() = Some(callback);
    }

    fn set_back_enabled(&self, enabled: bool) {
        self.back_enabled.set(enabled);
    }

    fn a11y_init(&self, callbacks: A11yCallbacks) {
        self.release_accessibility();
        *self.a11y_callbacks.borrow_mut() = Some(Arc::new(Mutex::new(callbacks)));
        self.initialize_accessibility();
    }

    fn a11y_tree_update(&self, update: accesskit::TreeUpdate) {
        self.initialize_accessibility();
        if let Some(adapter) = self.a11y_adapter.borrow().as_ref()
            && let Err(error) = adapter.update_if_active(|| update)
        {
            warn!("Failed to update OHOS accessibility tree: {error}");
        }
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.callbacks.borrow_mut().visibility_change = Some(callback);
    }

    fn insets(&self) -> WindowInsets {
        self.current_safe_area_insets()
    }

    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.callbacks.borrow_mut().insets_changed = Some(callback);
    }

    fn show_soft_keyboard(&self) {
        // This is an explicit user-gesture request. Do not suppress it based
        // on cached visibility: the system can dismiss the IME while GPUI
        // focus remains on the same input.
        self.request_keyboard();
    }

    fn hide_soft_keyboard(&self) {
        self.hide_keyboard_if_needed();
    }

    fn text_input_state_changed(&self, change: TextInputStateChange) {
        match change {
            TextInputStateChange::FocusGained => self.show_keyboard_if_needed(),
            TextInputStateChange::FocusLost => self.hide_keyboard_if_needed(),
            TextInputStateChange::SelectionChanged | TextInputStateChange::ContentChanged => {}
        }
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.callbacks.borrow_mut().resize = Some(callback);
        self.viewport.reset();
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().moved = Some(callback);
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        self.callbacks.borrow_mut().should_close = Some(callback);
    }

    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        self.callbacks.borrow_mut().hit_test_window_control = Some(callback);
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.callbacks.borrow_mut().close = Some(callback);
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().appearance_changed = Some(callback);
    }

    fn draw(&self, scene: &Scene) {
        if self.closed.get()
            || !self.surface_available.get()
            || self.visibility.get() != WindowVisibility::Visible
        {
            return;
        }
        // Initialize renderer lazily if not already initialized
        // This ensures native_window is available (after SurfaceCreate event)
        if self.renderer.borrow().is_none()
            && let Err(e) = self.initialize_renderer()
        {
            warn!("OhosWindow: Failed to initialize renderer in draw(): {}", e);
            return;
        }

        // Use WGPU renderer to render the scene.
        if let Some(ref mut renderer) = *self.renderer.borrow_mut() {
            renderer.draw(scene);
        } else {
            warn!("OhosWindow: draw called but renderer is not available");
        }
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        if let Some(ref renderer) = *self.renderer.borrow() {
            renderer.sprite_atlas().clone()
        } else if let Some(atlas) = self.fallback_atlas.borrow().as_ref() {
            atlas.clone()
        } else {
            if let Err(error) = self.initialize_renderer() {
                panic!("OhosWindow: renderer must be initialized before sprite_atlas: {error}");
            }
            self.renderer
                .borrow()
                .as_ref()
                .expect("renderer should be initialized after initialize_renderer")
                .sprite_atlas()
                .clone()
        }
    }

    fn request_decorations(&self, decorations: WindowDecorations) {
        if self.window_id == 0 {
            return;
        }
        let Some(client) = self.window_client() else {
            return;
        };
        let window_id = self.window_id;
        let state = self.decorations.clone();
        self.foreground_executor
            .spawn(async move {
                match client
                    .set_window_decorations(window_id, decorations == WindowDecorations::Server)
                    .await
                {
                    Ok(()) => state.set(decorations),
                    Err(error) => warn!("Failed to set OHOS window decorations: {error}"),
                }
            })
            .detach();
    }

    fn show_window_menu(&self, _position: Point<Pixels>) {
        // Not supported on OHOS
    }

    fn start_window_move(&self) {
        // Not supported on OHOS
    }

    fn start_window_resize(&self, _edge: ResizeEdge) {
        // Not supported on OHOS
    }

    fn window_decorations(&self) -> crate::Decorations {
        if self.decorations.get() == WindowDecorations::Client {
            crate::Decorations::Client {
                tiling: Default::default(),
            }
        } else {
            crate::Decorations::Server
        }
    }

    fn set_app_id(&mut self, _app_id: &str) {
        // Not supported on OHOS
    }

    fn map_window(&mut self) -> Result<()> {
        Ok(())
    }

    fn window_controls(&self) -> WindowControls {
        WindowControls {
            fullscreen: self.window_id != 0,
            maximize: self.window_id != 0,
            minimize: self.window_id != 0,
            window_menu: false,
        }
    }

    fn set_client_inset(&self, _inset: Pixels) {
        // Keyboard avoidance is driven by content_size updates from avoid-area overlap.
        // client_inset is intentionally ignored on OHOS.
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        // Return GPU specs from the WGPU renderer.
        self.renderer
            .borrow()
            .as_ref()
            .map(|renderer| renderer.gpu_specs())
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    fn update_ime_position(&self, bounds: Bounds<Pixels>) {
        let Some(client) = self.window_client() else {
            return;
        };
        let scale = self.scale_factor();
        let x = (bounds.origin.x.as_f32() * scale).round() as i64;
        let y = (bounds.origin.y.as_f32() * scale).round() as i64;
        let window_id = self.window_id;
        self.foreground_executor
            .spawn(async move {
                match client.set_ime_position(window_id, x, y).await {
                    Ok(response) if response.ok || response.code == 12800009 => {}
                    Ok(response) => warn!(
                        "Failed to update OHOS IME position for window {window_id}: {} ({})",
                        response.message, response.code
                    ),
                    Err(error) => {
                        warn!("Failed to update OHOS IME position for window {window_id}: {error}")
                    }
                }
            })
            .detach();
    }
}
