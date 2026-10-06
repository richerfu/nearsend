use log::{debug, warn};

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    path::PathBuf,
    rc::{Rc, Weak},
    sync::atomic::{AtomicUsize, Ordering},
    sync::{Arc, Mutex},
};

use anyhow::Result;
use futures::channel::oneshot;
use openharmony_ability::{
    ColorMode, Event, OpenHarmonyApp, TouchInputDelivery, WindowCreateParams, create_os_window,
    drain_pending_window_closes, drain_pending_window_status,
};
use openharmony_ability_plugin_app_control::{
    AppControlBridgePlugin, TerminateRequest, TerminateResponse,
};
use openharmony_ability_plugin_clipboard::{ClipboardBridgePlugin, ClipboardClient};
use openharmony_ability_plugin_files::{
    FileDialogOptions, FilesBridgePlugin, FilesExt as _, dialog_type,
};
use openharmony_ability_plugin_menu::{
    MenuBridgePlugin, MenuClient, MenuItemData, MenuSetMenubarRequest, register_menu_event_sender,
};
use openharmony_ability_plugin_process::{ProcessBridgePlugin, ProcessExt as _};
use openharmony_ability_plugin_url::{UrlBridgePlugin, UrlExt as _};
use openharmony_ability_plugin_window::{WindowBridgePlugin, WindowClient};
use sha2::{Digest, Sha256};

use crate::{
    Action, ActivityGuard, AnyWindowHandle, AppLifecyclePhase, BackgroundExecutor, ClipboardEntry,
    ClipboardItem, ClipboardReadError, CursorStyle, ExternalPaths, ForegroundExecutor,
    GestureKinds, GestureTuning, Image, ImageFormat, Keymap, Menu, MenuItem, OwnedMenu,
    OwnedMenuItem, PathPromptOptions, Platform, PlatformDisplay, PlatformGestures,
    PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem, PlatformWindow,
    PriorityQueueReceiver, Result as GpuiResult, RunnableVariant, ScrollPhysics, Task,
    ThermalState, WindowAppearance, WindowParams,
};

use super::{
    dispatcher::OhosDispatcher, display::OhosDisplay, screen_capture, text_system::OhosTextSystem,
    wgpu_context::WgpuContext, window::OhosWindow,
};

pub(crate) struct OhosPlatform {
    app: Rc<RefCell<Option<OpenHarmonyApp>>>,
    dispatcher: Arc<OhosDispatcher>,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    text_system: Arc<dyn PlatformTextSystem>,
    primary_display: Rc<RefCell<Option<OhosDisplay>>>,
    main_receiver: PriorityQueueReceiver<RunnableVariant>,
    gpu_context: Arc<WgpuContext>,
    windows: Rc<RefCell<Vec<Weak<RefCell<OhosWindow>>>>>,
    open_urls: Rc<RefCell<Option<Box<dyn FnMut(Vec<String>)>>>>,
    app_lifecycle: Rc<RefCell<Option<Box<dyn FnMut(AppLifecyclePhase)>>>>,
    memory_warning: Rc<RefCell<Option<Box<dyn FnMut()>>>>,
    clipboard_cache: Rc<RefCell<Option<ClipboardItem>>>,
    cursor_hidden_until_move: Rc<Cell<bool>>,
    cursor_window_id: Rc<Cell<i64>>,
    idle_sleep_guards: Arc<AtomicUsize>,
    menus: Rc<RefCell<MenuState>>,
    menu_events: Arc<Mutex<VecDeque<String>>>,
}

#[derive(Default)]
struct MenuState {
    menus: Option<Vec<OwnedMenu>>,
    json: String,
    actions: HashMap<String, Box<dyn Action>>,
    next_id: u64,
    on_action: Option<Box<dyn FnMut(&dyn Action)>>,
}

fn menu_items(
    items: &[OwnedMenuItem],
    state: &mut MenuState,
    keymap: &Keymap,
) -> Vec<MenuItemData> {
    items
        .iter()
        .map(|item| {
            let id = format!("gpui-menu-{}", state.next_id);
            state.next_id += 1;
            let (item_type, text, enabled, checked, submenu_items, accelerator) = match item {
                OwnedMenuItem::Separator => ("separator", None, None, None, None, None),
                OwnedMenuItem::Submenu(menu) => (
                    "submenu",
                    Some(menu.name.to_string()),
                    Some(!menu.disabled),
                    None,
                    Some(menu_items(&menu.items, state, keymap)),
                    None,
                ),
                OwnedMenuItem::SystemMenu(menu) => (
                    "submenu",
                    Some(menu.name.to_string()),
                    Some(true),
                    None,
                    Some(Vec::new()),
                    None,
                ),
                OwnedMenuItem::Action {
                    name,
                    action,
                    checked,
                    disabled,
                    ..
                } => {
                    let accelerator = keymap
                        .bindings_for_action(action.as_ref())
                        .filter(|binding| binding.keystrokes().len() == 1)
                        .last()
                        .map(|binding| {
                            let key = &binding.keystrokes()[0];
                            let modifiers = key.modifiers();
                            let mut parts = Vec::new();
                            if modifiers.control || modifiers.platform {
                                parts.push("Ctrl");
                            }
                            if modifiers.shift {
                                parts.push("Shift");
                            }
                            if modifiers.alt {
                                parts.push("Alt");
                            }
                            parts.push(key.key());
                            parts.join("+")
                        });
                    state.actions.insert(id.clone(), action.boxed_clone());
                    (
                        "item",
                        Some(name.clone()),
                        Some(!disabled),
                        Some(*checked),
                        None,
                        accelerator,
                    )
                }
            };
            MenuItemData {
                id,
                item_type: item_type.into(),
                text,
                enabled,
                accelerator,
                predefined_type: None,
                checked,
                icon: None,
                native_icon: None,
                submenu_items,
                about_metadata: None,
            }
        })
        .collect()
}

pub(crate) fn appearance_for_color_mode(mode: ColorMode) -> WindowAppearance {
    match mode {
        ColorMode::Dark => WindowAppearance::Dark,
        ColorMode::Light | ColorMode::NoSet => WindowAppearance::Light,
    }
}

fn ohos_cursor_style(style: CursorStyle) -> i32 {
    match style {
        CursorStyle::Arrow | CursorStyle::DragLink | CursorStyle::ContextualMenu => 0,
        CursorStyle::IBeam | CursorStyle::IBeamCursorForVerticalLayout => 26,
        CursorStyle::Crosshair => 13,
        CursorStyle::ClosedHand => 17,
        CursorStyle::OpenHand => 18,
        CursorStyle::PointingHand => 19,
        CursorStyle::ResizeLeft => 2,
        CursorStyle::ResizeRight => 1,
        CursorStyle::ResizeLeftRight | CursorStyle::ResizeColumn => 5,
        CursorStyle::ResizeUp => 4,
        CursorStyle::ResizeDown => 3,
        CursorStyle::ResizeUpDown | CursorStyle::ResizeRow => 6,
        CursorStyle::ResizeUpLeftDownRight => 12,
        CursorStyle::ResizeUpRightDownLeft => 11,
        CursorStyle::OperationNotAllowed => 15,
        CursorStyle::DragCopy => 14,
    }
}

fn credential_alias(url: &str) -> String {
    format!("gpui-ohos:{:x}", Sha256::digest(url.as_bytes()))
}

impl OhosPlatform {
    pub(crate) fn new(app: OpenHarmonyApp) -> Result<Self> {
        let (main_sender, main_receiver) = PriorityQueueReceiver::new();
        let dispatcher = Arc::new(OhosDispatcher::new(main_sender));
        let background_executor = BackgroundExecutor::new(dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(dispatcher.clone());
        let text_system = Arc::new(OhosTextSystem::new());
        let menu_events = Arc::new(Mutex::new(VecDeque::new()));
        let (menu_sender, menu_receiver) = crossbeam_channel::unbounded();
        register_menu_event_sender(menu_sender);
        let pending_menu_events = menu_events.clone();
        let menu_waker = app.create_waker();
        std::thread::spawn(move || {
            while let Ok(id) = menu_receiver.recv() {
                log::info!("OHOS menu event received: {id}");
                pending_menu_events.lock().unwrap().push_back(id);
                menu_waker.wake();
            }
        });

        // Initialize GPU context for WGPU renderer.
        // Note: ZED_DEVICE_ID environment variable is optional - if not set, device_id defaults to 0
        let gpu_context = Arc::new(WgpuContext::new()
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create GPU context: {}. \
                    Note: ZED_DEVICE_ID environment variable is optional. \
                    If you need to specify a GPU device, set ZED_DEVICE_ID to a 4-digit hex PCI ID (e.g., '0x1234').",
                    e
                )
            })?);

        let platform = Self {
            app: Rc::new(RefCell::new(None)),
            dispatcher,
            background_executor,
            foreground_executor,
            text_system,
            primary_display: Rc::new(RefCell::new(None)),
            main_receiver,
            gpu_context,
            windows: Rc::new(RefCell::new(Vec::new())),
            open_urls: Rc::new(RefCell::new(None)),
            app_lifecycle: Rc::new(RefCell::new(None)),
            memory_warning: Rc::new(RefCell::new(None)),
            clipboard_cache: Rc::new(RefCell::new(None)),
            cursor_hidden_until_move: Rc::new(Cell::new(false)),
            cursor_window_id: Rc::new(Cell::new(0)),
            idle_sleep_guards: Arc::new(AtomicUsize::new(0)),
            menus: Rc::new(RefCell::new(MenuState::default())),
            menu_events,
        };
        platform.set_app(app);
        Ok(platform)
    }

    pub(crate) fn set_app(&self, app: OpenHarmonyApp) {
        let windows = self.windows.clone();
        app.on_back_press_intercept(move || {
            let handler = windows
                .borrow()
                .iter()
                .filter_map(Weak::upgrade)
                .find_map(|window| {
                    let window = window.borrow();
                    if window.is_active() {
                        let (enabled, handler) = window.back_handler_state();
                        enabled.then_some(handler)
                    } else {
                        None
                    }
                });
            let Some(handler) = handler else {
                return false;
            };
            let mut callback = handler.borrow_mut().take();
            let handled = if let Some(ref mut callback) = callback {
                callback();
                true
            } else {
                false
            };
            *handler.borrow_mut() = callback;
            handled
        });
        if let Err(error) = app.set_touch_input_delivery(TouchInputDelivery::Both) {
            warn!("Failed to configure system pan and raw control input for GPUI: {error}");
        }
        if let Err(error) = app.register_plugin(AppControlBridgePlugin) {
            warn!("Failed to register OpenHarmony app-control plugin: {error}");
        }
        if let Err(error) = app.register_plugin(UrlBridgePlugin) {
            warn!("Failed to register OpenHarmony URL plugin: {error}");
        }
        for result in [
            app.register_plugin(ClipboardBridgePlugin),
            app.register_plugin(FilesBridgePlugin),
            app.register_plugin(MenuBridgePlugin),
            app.register_plugin(ProcessBridgePlugin),
            app.register_plugin(WindowBridgePlugin),
        ] {
            if let Err(error) = result {
                warn!("Failed to register OpenHarmony platform plugin: {error}");
            }
        }
        *self.app.borrow_mut() = Some(app.clone());
        // Initialize primary display when app is set
        *self.primary_display.borrow_mut() = Some(OhosDisplay::new(app.clone()));
        self.dispatcher.set_waker(app.create_waker());
    }

    fn run_foreground_tasks(&self) {
        // Process GPUI tasks queued for the main thread
        // Similar to Windows' run_foreground_task, but simpler since OHOS doesn't have message timeouts
        let mut receiver = self.main_receiver.clone();
        while let Ok(Some(runnable)) = receiver.try_pop() {
            OhosDispatcher::execute_runnable(runnable);
        }
    }

    fn publish_menu(&self, window_id: i64) {
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        let json_data = self.menus.borrow().json.clone();
        if json_data.is_empty() {
            return;
        }
        self.background_executor
            .spawn(async move {
                let result = async {
                    MenuClient::new(&app)?
                        .set_menubar(MenuSetMenubarRequest {
                            json_data,
                            window_id: if window_id == 0 {
                                "main".into()
                            } else {
                                window_id.to_string()
                            },
                        })
                        .await
                }
                .await;
                if let Err(error) = result {
                    warn!("Failed to publish OHOS menu for window {window_id}: {error}");
                }
            })
            .detach();
    }

    fn dispatch_menu_events(&self) {
        loop {
            let id = self.menu_events.lock().unwrap().pop_front();
            let Some(id) = id else { break };
            let action = self
                .menus
                .borrow()
                .actions
                .get(&id)
                .map(|action| action.boxed_clone());
            let Some(action) = action else {
                warn!("Unknown OHOS menu item {id}");
                continue;
            };
            log::info!("Dispatching OHOS menu action {id}");
            let mut on_action = self.menus.borrow_mut().on_action.take();
            if let Some(ref mut callback) = on_action {
                callback(action.as_ref());
            }
            self.menus.borrow_mut().on_action = on_action;
        }
    }

    fn handle_ohos_event(&self, event: &Event, on_finish_launching: Option<Box<dyn FnOnce()>>) {
        let phase = match event {
            Event::Start => Some(AppLifecyclePhase::Foreground),
            Event::GainedFocus => Some(AppLifecyclePhase::Active),
            Event::LostFocus => Some(AppLifecyclePhase::Inactive),
            Event::Stop => Some(AppLifecyclePhase::Background),
            _ => None,
        };
        if let Some(phase) = phase {
            let mut callback = self.app_lifecycle.borrow_mut().take();
            if let Some(ref mut callback) = callback {
                callback(phase);
            }
            *self.app_lifecycle.borrow_mut() = callback;
        }
        if matches!(event, Event::LowMemory) {
            let mut callback = self.memory_warning.borrow_mut().take();
            if let Some(ref mut callback) = callback {
                callback();
            }
            *self.memory_warning.borrow_mut() = callback;
        }
        if let Event::NewWant { uri } = event
            && !uri.is_empty()
        {
            let mut callback = self.open_urls.borrow_mut().take();
            if let Some(ref mut callback) = callback {
                callback(vec![uri.clone()]);
            }
            *self.open_urls.borrow_mut() = callback;
        }
        // create_waker() snapshots the lifecycle's current ThreadsafeFunction. Refresh it once
        // the surface exists so timers scheduled during early startup can reliably wake the UI
        // thread even if the first snapshot was taken before lifecycle initialization finished.
        if matches!(event, Event::SurfaceCreate)
            && let Some(app) = self.app.borrow().as_ref()
        {
            self.dispatcher.set_waker(app.create_waker());
        }

        // First, process any GPUI tasks queued for the main thread
        // This ensures tasks are processed in the run_loop, integrating GPUI with OpenHarmony's event loop
        self.run_foreground_tasks();
        self.dispatcher.run_due_timers();
        self.dispatch_menu_events();

        // Handle on_finish_launching callback first, before routing to windows.
        // This is critical because windows are created INSIDE the on_finish_launching callback,
        // so we cannot depend on windows existing before calling it.
        // This is similar to how macOS handles did_finish_launching.
        // Note: The callback is only passed when event is SurfaceCreate (checked in run() method),
        // so we can safely call it here unconditionally.
        if let Some(callback) = on_finish_launching {
            debug!("OhosPlatform: Calling on_finish_launching on SurfaceCreate");
            callback();
        }

        // Route events to all known OHOS windows without borrowing App.
        // This avoids RefCell borrow conflicts when callbacks trigger app updates.
        let mut live_windows: Vec<Rc<RefCell<OhosWindow>>> = Vec::new();
        {
            let mut windows = self.windows.borrow_mut();
            windows.retain(|weak: &Weak<RefCell<OhosWindow>>| {
                if let Some(window) = weak.upgrade() {
                    live_windows.push(window);
                    true
                } else {
                    false
                }
            });
        }

        if live_windows.is_empty() {
            warn!("OhosPlatform: No active windows to handle event");
        }

        for (window_id, status) in drain_pending_window_status() {
            if let Some(window) = live_windows
                .iter()
                .find(|window| window.borrow().window_id() == i64::from(window_id))
            {
                window.borrow().apply_window_status(status);
            }
        }

        // FloatPage reports native close-button and system closes through this
        // queue. GPUI must consume it so its window registry and close observers
        // are updated even when no SubWindowClosed event is emitted.
        for window_id in drain_pending_window_closes() {
            if let Some(window) = live_windows
                .iter()
                .find(|window| window.borrow().window_id() == i64::from(window_id))
            {
                window.borrow().handle_event(&Event::WindowDestroy);
            }
        }

        for window in &live_windows {
            let id = window.borrow().window_id();
            match event {
                Event::SubWindowSurfaceCreate(window_id) if id == *window_id => {
                    window.borrow().handle_event(&Event::SurfaceCreate)
                }
                Event::SubWindowSurfaceDestroy(window_id) if id == *window_id => {
                    window.borrow().handle_event(&Event::SurfaceDestroy)
                }
                Event::SubWindowClosed(window_id) if id == *window_id => {
                    window.borrow().handle_event(&Event::WindowDestroy)
                }
                Event::SubWindowRedraw {
                    window_id,
                    interval,
                } if id == *window_id => window
                    .borrow()
                    .handle_event(&Event::WindowRedraw(interval.clone())),
                Event::SubWindowInput { window_id, event } if id == *window_id => {
                    self.cursor_window_id.set(id);
                    window.borrow().handle_event(&Event::Input(event.clone()))
                }
                Event::WindowResize { window_id, .. } if id == *window_id => {
                    window.borrow().handle_event(event)
                }
                Event::ContentRectChange(rect) if id == rect.window_id => {
                    window.borrow().handle_event(event)
                }
                Event::AvoidAreaChange(info) if id == info.window_id => {
                    window.borrow().handle_event(event)
                }
                Event::WindowFocusChanged { window_id, focused }
                    if *focused || id == *window_id =>
                {
                    window.borrow().handle_event(event)
                }
                Event::SurfaceCreate
                | Event::SurfaceDestroy
                | Event::WindowRedraw(_)
                | Event::WindowDestroy
                    if id == 0 =>
                {
                    window.borrow().handle_event(event)
                }
                Event::Input(_) if id == 0 => {
                    self.cursor_window_id.set(0);
                    window.borrow().handle_event(event)
                }
                Event::SubWindowSurfaceCreate(_)
                | Event::SubWindowSurfaceDestroy(_)
                | Event::SubWindowClosed(_)
                | Event::SubWindowRedraw { .. }
                | Event::SubWindowInput { .. }
                | Event::WindowResize { .. }
                | Event::ContentRectChange(_)
                | Event::AvoidAreaChange(_)
                | Event::WindowFocusChanged { .. }
                | Event::SurfaceCreate
                | Event::SurfaceDestroy
                | Event::WindowRedraw(_)
                | Event::Input(_)
                | Event::WindowDestroy => {}
                Event::GainedFocus | Event::LostFocus => {}
                Event::KeyboardEvent(_) if id != 0 => {}
                _ => window.borrow().handle_event(event),
            }
        }
        match event {
            Event::SurfaceCreate => self.publish_menu(0),
            Event::SubWindowSurfaceCreate(window_id) => self.publish_menu(*window_id),
            _ => {}
        }

        // Apply visibility and surface lifecycle before draining VSync. A
        // queued frame must not present to a surface the system just hid or
        // destroyed; GLES presentation can otherwise block requesting a buffer.
        for window in &live_windows {
            if window.borrow().take_pending_frame() {
                window.borrow().draw_requested_frame();
            }
        }
    }
}

fn selected_paths(uris: Vec<String>, writable: bool) -> Result<Option<Vec<PathBuf>>> {
    if uris.is_empty() {
        return Ok(None);
    }
    let operation_mode = if writable { 3 } else { 1 };
    let policies = uris
        .iter()
        .map(|uri| ohos_fileshare_binding::PolicyInfo {
            uri: uri.clone(),
            operation_mode,
        })
        .collect::<Vec<_>>();
    let failed = ohos_fileshare_binding::persist_permission(&policies)?;
    anyhow::ensure!(
        failed.is_empty(),
        "Could not persist file picker permission: {failed:?}"
    );
    let failed = ohos_fileshare_binding::activate_permission(&policies)?;
    anyhow::ensure!(
        failed.is_empty(),
        "Could not activate file picker permission: {failed:?}"
    );
    let paths = uris
        .iter()
        .map(|uri| {
            let native_path = ohos_fileuri_binding::get_path_from_uri(uri)?;
            let path = PathBuf::from(native_path);
            anyhow::ensure!(path.is_absolute(), "Picker returned a non-absolute path");
            Ok(path)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(paths))
}

struct OhosGestures;

impl PlatformGestures for OhosGestures {
    fn native_recognizers(&self) -> GestureKinds {
        GestureKinds {
            tap: true,
            long_press: true,
            pan: true,
            pinch: false,
        }
    }

    fn tuning(&self) -> GestureTuning {
        GestureTuning {
            // HarmonyOS touch coordinates are converted to logical pixels before
            // entering GPUI, matching the coordinate space used by Android's
            // portable gesture implementation.
            scroll_physics: ScrollPhysics::ohos(),
            ..GestureTuning::default()
        }
    }
}

impl Clone for OhosPlatform {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            dispatcher: self.dispatcher.clone(),
            background_executor: self.background_executor.clone(),
            foreground_executor: self.foreground_executor.clone(),
            text_system: self.text_system.clone(),
            primary_display: self.primary_display.clone(),
            main_receiver: self.main_receiver.clone(),
            gpu_context: self.gpu_context.clone(),
            windows: self.windows.clone(),
            open_urls: self.open_urls.clone(),
            app_lifecycle: self.app_lifecycle.clone(),
            memory_warning: self.memory_warning.clone(),
            clipboard_cache: self.clipboard_cache.clone(),
            cursor_hidden_until_move: self.cursor_hidden_until_move.clone(),
            cursor_window_id: self.cursor_window_id.clone(),
            idle_sleep_guards: self.idle_sleep_guards.clone(),
            menus: self.menus.clone(),
            menu_events: self.menu_events.clone(),
        }
    }
}

impl Platform for OhosPlatform {
    fn on_app_lifecycle(&self, callback: Box<dyn FnMut(AppLifecyclePhase)>) {
        *self.app_lifecycle.borrow_mut() = Some(callback);
    }

    fn on_memory_warning(&self, callback: Box<dyn FnMut()>) {
        *self.memory_warning.borrow_mut() = Some(callback);
    }

    fn gestures(&self) -> Option<Rc<dyn PlatformGestures>> {
        Some(Rc::new(OhosGestures))
    }

    fn background_executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        let platform = self.clone();
        let on_finish = Rc::new(RefCell::new(Some(on_finish_launching)));
        if let Some(app) = self.app.borrow().clone() {
            let on_finish_clone = on_finish.clone();
            app.run_loop(move |event: Event| {
                // Only take on_finish_launching when we receive SurfaceCreate event
                let callback = if matches!(event, Event::SurfaceCreate { .. }) {
                    on_finish_clone.borrow_mut().take()
                } else {
                    None
                };
                platform.handle_ohos_event(&event, callback);
            });
        } else {
            warn!("OhosPlatform: App not set");
        }
    }

    fn quit(&self) {
        let Some(app) = self.app.borrow().clone() else {
            return;
        };

        self.background_executor
            .spawn(async move {
                let result = async {
                    let response = app
                        .bridge()?
                        .call_sync_from_worker::<
                            AppControlBridgePlugin,
                            TerminateRequest,
                            TerminateResponse,
                        >("terminate", TerminateRequest { code: 0 })
                        .await?;
                    anyhow::ensure!(
                        response.accepted,
                        "OpenHarmony app-control plugin rejected termination"
                    );
                    Ok::<(), anyhow::Error>(())
                }
                .await;

                if let Err(error) = result {
                    warn!("Failed to terminate OpenHarmony application: {error}");
                }
            })
            .detach();
    }

    fn restart(&self, binary_path: Option<PathBuf>, arguments: Vec<std::ffi::OsString>) {
        if binary_path.is_some() || !arguments.is_empty() {
            warn!("OHOS restart resumes the current Ability without replacement arguments");
        }
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        self.background_executor
            .spawn(async move {
                let result = async { app.process()?.restart().await }.await;
                if let Err(error) = result {
                    warn!("Failed to restart OpenHarmony application: {error}");
                }
            })
            .detach();
    }

    fn activate(&self, _ignoring_other_apps: bool) {
        // Not supported on OHOS
    }

    fn hide_cursor_until_mouse_moves(&self) {
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        self.cursor_hidden_until_move.set(true);
        let hidden = self.cursor_hidden_until_move.clone();
        self.foreground_executor
            .spawn(async move {
                let result = async {
                    let client = WindowClient::new(&app)?;
                    client.set_cursor_visible(false).await?;
                    if !hidden.get() {
                        client.set_cursor_visible(true).await?;
                    }
                    Ok::<(), anyhow::Error>(())
                }
                .await;
                if let Err(error) = result {
                    warn!("Failed to hide OHOS cursor: {error}");
                }
            })
            .detach();
    }

    fn is_cursor_visible(&self) -> bool {
        !self.cursor_hidden_until_move.get()
    }

    fn hide(&self) {
        // Not supported on OHOS
    }

    fn hide_other_apps(&self) {
        // Not supported on OHOS
    }

    fn unhide_other_apps(&self) {
        // Not supported on OHOS
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        if let Some(display) = self.primary_display.borrow().as_ref() {
            vec![Rc::new(display.clone()) as Rc<dyn PlatformDisplay>]
        } else {
            vec![]
        }
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        self.primary_display
            .borrow()
            .as_ref()
            .map(|d| Rc::new(d.clone()) as Rc<dyn PlatformDisplay>)
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.windows
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .find_map(|window| {
                let window = window.borrow();
                window.is_active().then_some(window.handle)
            })
    }

    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        Some(
            self.windows
                .borrow()
                .iter()
                .filter_map(Weak::upgrade)
                .map(|window| window.borrow().handle)
                .collect(),
        )
    }

    fn is_screen_capture_supported(&self) -> bool {
        self.app.borrow().as_ref().is_some_and(|app| {
            let (width, height) = app.display_size();
            i32::try_from(width).is_ok_and(|width| width > 0)
                && i32::try_from(height).is_ok_and(|height| height > 0)
        })
    }

    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<GpuiResult<Vec<Rc<dyn crate::ScreenCaptureSource>>>> {
        if let Some(app) = self.app.borrow().as_ref() {
            let (width, height) = app.display_size();
            screen_capture::sources(
                i32::try_from(width).unwrap_or(0),
                i32::try_from(height).unwrap_or(0),
            )
        } else {
            screen_capture::sources(0, 0)
        }
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        if let Some(app) = self.app.borrow().clone() {
            let existing = self
                .windows
                .borrow()
                .iter()
                .filter_map(Weak::upgrade)
                .collect::<Vec<_>>();
            let (window_id, fallback_atlas) = if let Some(primary) = existing.first() {
                let atlas = primary
                    .borrow()
                    .atlas()
                    .ok_or_else(|| anyhow::anyhow!("Primary OHOS window has no GPU atlas"))?;
                let scale = app.scale() as f32;
                let bounds = options.bounds;
                let window_id = create_os_window(WindowCreateParams {
                    name: format!("gpui-{}", uuid::Uuid::new_v4()),
                    native_module_name: Some(app.module_name().ok_or_else(|| {
                        anyhow::anyhow!("OHOS native module name is unavailable")
                    })?),
                    width: (bounds.size.width.as_f32() * scale).max(1.0) as i32,
                    height: (bounds.size.height.as_f32() * scale).max(1.0) as i32,
                    x: (bounds.origin.x.as_f32() * scale) as i32,
                    y: (bounds.origin.y.as_f32() * scale) as i32,
                    ..Default::default()
                })?;
                (window_id, Some(atlas))
            } else {
                (0, None)
            };
            let window = OhosWindow::new(
                self.app.clone(),
                handle,
                options,
                self.gpu_context.clone(),
                self.foreground_executor.clone(),
                self.cursor_hidden_until_move.clone(),
                window_id,
                fallback_atlas,
            )?;

            // GPUI fetches sprite_atlas during window initialization and caches it.
            // Renderer must be ready at open_window time to avoid caching a broken atlas.
            if window_id == 0 {
                window.initialize_renderer()?;
            }

            let window = Rc::new(RefCell::new(window));
            self.windows.borrow_mut().push(Rc::downgrade(&window));
            Ok(Box::new(super::window::OhosWindowHandle::new(window)))
        } else {
            Err(anyhow::anyhow!("OpenHarmonyApp not set"))
        }
    }

    fn window_appearance(&self) -> WindowAppearance {
        self.app
            .borrow()
            .as_ref()
            .map(|app| appearance_for_color_mode(app.config().color_mode))
            .unwrap_or_default()
    }

    fn open_url(&self, url: &str) {
        let Some(app) = self.app.borrow().clone() else {
            warn!("Cannot open URL before OpenHarmonyApp is set: {url}");
            return;
        };
        let url = url.to_owned();
        self.background_executor
            .spawn(async move {
                if let Err(error) = app.open_url(url).await {
                    warn!("Failed to open URL on OHOS: {error}");
                }
            })
            .detach();
    }

    fn on_open_urls(&self, mut callback: Box<dyn FnMut(Vec<String>)>) {
        let initial_uri = self
            .app
            .borrow()
            .as_ref()
            .map(OpenHarmonyApp::take_initial_want_uri)
            .unwrap_or_default();
        if !initial_uri.is_empty() {
            callback(vec![initial_uri]);
        }
        *self.open_urls.borrow_mut() = Some(callback);
    }

    fn register_url_scheme(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "URL scheme registration not supported on OHOS"
        )))
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        let Some(app) = self.app.borrow().clone() else {
            tx.send(Err(anyhow::anyhow!("OpenHarmonyApp not set"))).ok();
            return rx;
        };
        self.background_executor
            .spawn(async move {
                let result = async {
                    anyhow::ensure!(
                        options.files != options.directories,
                        "Select files or directories, not both"
                    );
                    let kind = if options.files {
                        dialog_type::OPEN_FILE
                    } else {
                        dialog_type::OPEN_FOLDER
                    };
                    let response = app
                        .show_file_dialog(FileDialogOptions::new(kind).allow_many(options.multiple))
                        .await?;
                    selected_paths(response.files, false)
                }
                .await;
                tx.send(result).ok();
            })
            .detach();
        rx
    }

    fn prompt_for_new_path(
        &self,
        directory: &std::path::Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let (tx, rx) = oneshot::channel();
        let Some(app) = self.app.borrow().clone() else {
            tx.send(Err(anyhow::anyhow!("OpenHarmonyApp not set"))).ok();
            return rx;
        };
        let directory = directory.to_path_buf();
        let suggested_name = suggested_name.map(str::to_owned);
        self.background_executor
            .spawn(async move {
                let result = async {
                    let location = directory
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Save directory is not UTF-8"))?;
                    let uri = ohos_fileuri_binding::get_uri_from_path(location)?;
                    let mut options =
                        FileDialogOptions::new(dialog_type::SAVE_FILE).default_location(uri);
                    options.suggested_name = suggested_name;
                    let response = app.show_file_dialog(options).await?;
                    Ok(selected_paths(response.files, true)?.and_then(|mut paths| paths.pop()))
                }
                .await;
                tx.send(result).ok();
            })
            .detach();
        rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }

    fn reveal_path(&self, path: &std::path::Path) {
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        let directory = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        let Some(directory) = directory.to_str() else {
            warn!("Cannot reveal a non-UTF-8 path on OHOS");
            return;
        };
        let directory = directory.to_owned();
        self.background_executor
            .spawn(async move {
                if let Err(error) = app.reveal_in_dir(directory).await {
                    warn!("Failed to reveal OHOS directory: {error}");
                }
            })
            .detach();
    }

    fn open_with_system(&self, path: &std::path::Path) {
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        let Some(path) = path.to_str() else {
            warn!("Cannot open a non-UTF-8 path on OHOS");
            return;
        };
        let uri = match ohos_fileuri_binding::get_uri_from_path(path) {
            Ok(uri) => uri,
            Err(error) => {
                warn!("Cannot make OHOS file URI for {path}: {error}");
                return;
            }
        };
        self.background_executor
            .spawn(async move {
                if let Err(error) = app.open_file(uri).await {
                    warn!("Failed to open OHOS file with system: {error}");
                }
            })
            .detach();
    }

    fn on_quit(&self, _callback: Box<dyn FnMut() -> bool>) {
        // Handled by OpenHarmonyApp lifecycle
    }

    fn on_reopen(&self, _callback: Box<dyn FnMut()>) {
        // Not supported on OHOS
    }

    fn on_system_wake(&self, _callback: Box<dyn FnMut()>) {
        // Not supported on OHOS
    }

    fn on_system_sleep(&self, _callback: Box<dyn FnMut()>) {
        // Not supported on OHOS
    }

    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap) {
        let owned: Vec<OwnedMenu> = menus.into_iter().map(Menu::owned).collect();
        {
            let mut state = self.menus.borrow_mut();
            state.actions.clear();
            let mut items = Vec::with_capacity(owned.len());
            for menu in &owned {
                let id = format!("gpui-menu-{}", state.next_id);
                state.next_id += 1;
                items.push(MenuItemData {
                    id,
                    item_type: "submenu".into(),
                    text: Some(menu.name.to_string()),
                    enabled: Some(!menu.disabled),
                    accelerator: None,
                    predefined_type: None,
                    checked: None,
                    icon: None,
                    native_icon: None,
                    submenu_items: Some(menu_items(&menu.items, &mut state, keymap)),
                    about_metadata: None,
                });
            }
            match serde_json::to_string(&items) {
                Ok(json) => {
                    state.json = json;
                    state.menus = Some(owned);
                }
                Err(error) => {
                    warn!("Failed to serialize OHOS app menu: {error}");
                    return;
                }
            }
        }
        self.publish_menu(0);
        for window in self.windows.borrow().iter().filter_map(Weak::upgrade) {
            let id = window.borrow().window_id();
            if id != 0 {
                self.publish_menu(id);
            }
        }
    }

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        self.menus.borrow().menus.clone()
    }

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {
        // Not supported on OHOS
    }

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.menus.borrow_mut().on_action = Some(callback);
    }

    fn on_will_open_app_menu(&self, _callback: Box<dyn FnMut()>) {
        // Not supported on OHOS
    }

    fn on_validate_app_menu_command(&self, _callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        // Not supported on OHOS
    }

    fn compositor_name(&self) -> &'static str {
        "OHOS"
    }

    fn app_path(&self) -> Result<PathBuf> {
        Err(anyhow::anyhow!("app_path not available on OHOS"))
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> Result<PathBuf> {
        Err(anyhow::anyhow!(
            "path_for_auxiliary_executable not available on OHOS"
        ))
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        let style = ohos_cursor_style(style);
        let window_id = self.cursor_window_id.get();
        self.background_executor
            .spawn(async move {
                let result = async {
                    WindowClient::new(&app)?
                        .set_cursor_icon(window_id, style)
                        .await
                }
                .await;
                if let Err(error) = result {
                    warn!("Failed to set OHOS cursor style: {error}");
                }
            })
            .detach();
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        false
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.clipboard_cache.borrow().clone()
    }

    fn read_from_clipboard_async(
        &self,
    ) -> Task<std::result::Result<Option<ClipboardItem>, ClipboardReadError>> {
        let Some(app) = self.app.borrow().clone() else {
            return Task::ready(Err(ClipboardReadError::Unavailable));
        };
        let cache = self.clipboard_cache.clone();
        self.foreground_executor.spawn(async move {
            let client = ClipboardClient::new(&app)
                .map_err(|error| ClipboardReadError::Denied(error.to_string()))?;
            let content = client
                .read_content()
                .await
                .map_err(|error| ClipboardReadError::Denied(error.to_string()))?;
            let mut entries = Vec::new();
            if let Some(text) = content.text.filter(|text| !text.is_empty()) {
                entries.push(ClipboardEntry::from(text));
            }
            if let Some(png) = content.png.filter(|png| !png.is_empty()) {
                entries.push(ClipboardEntry::Image(Image::from_bytes(
                    ImageFormat::Png,
                    png,
                )));
            }
            let paths = content
                .uris
                .iter()
                .filter_map(|uri| match ohos_fileuri_binding::get_path_from_uri(uri) {
                    Ok(path) => Some(PathBuf::from(path)),
                    Err(error) => {
                        warn!("Cannot map OHOS clipboard URI {uri}: {error}");
                        None
                    }
                })
                .collect();
            if !content.uris.is_empty() {
                entries.push(ClipboardEntry::ExternalPaths(ExternalPaths(paths)));
            }
            let item = (!entries.is_empty()).then_some(ClipboardItem { entries });
            *cache.borrow_mut() = item.clone();
            Ok(item)
        })
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        enum Write {
            Text(String),
            Image(Vec<u8>),
            Uris(Vec<String>),
        }
        let write = if let Some(ClipboardEntry::ExternalPaths(paths)) = item
            .entries()
            .iter()
            .find(|entry| matches!(entry, ClipboardEntry::ExternalPaths(_)))
        {
            let uris = paths
                .paths()
                .iter()
                .map(|path| {
                    let path = path
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Clipboard path is not UTF-8"))?;
                    ohos_fileuri_binding::get_uri_from_path(path).map_err(anyhow::Error::from)
                })
                .collect::<Result<Vec<_>>>();
            match uris {
                Ok(uris) => Write::Uris(uris),
                Err(error) => {
                    warn!("Cannot write OHOS clipboard paths: {error}");
                    return;
                }
            }
        } else if let Some(ClipboardEntry::Image(image)) = item
            .entries()
            .iter()
            .find(|entry| matches!(entry, ClipboardEntry::Image(_)))
        {
            Write::Image(image.bytes.clone())
        } else if let Some(text) = item.text() {
            Write::Text(text)
        } else {
            warn!("OHOS clipboard item has no supported entries");
            return;
        };
        let cache = self.clipboard_cache.clone();
        self.foreground_executor
            .spawn(async move {
                let result = async {
                    let client = ClipboardClient::new(&app)?;
                    match write {
                        Write::Text(text) => client.write_text(text).await,
                        Write::Image(bytes) => client.write_encoded_image(&bytes).await,
                        Write::Uris(uris) => client.write_uris(uris).await,
                    }
                }
                .await;
                match result {
                    Ok(()) => *cache.borrow_mut() = Some(item),
                    Err(error) => warn!("Failed to write OHOS clipboard: {error}"),
                }
            })
            .detach();
    }

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>> {
        let alias = credential_alias(url);
        let username = username.to_owned();
        let password = password.to_vec();
        self.background_executor
            .spawn(async move { super::credentials::write(&alias, &username, &password) })
    }

    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        let alias = credential_alias(url);
        self.background_executor
            .spawn(async move { super::credentials::read(&alias) })
    }

    fn delete_credentials(&self, url: &str) -> Task<Result<()>> {
        let alias = credential_alias(url);
        self.background_executor
            .spawn(async move { super::credentials::delete(&alias) })
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(super::keyboard::OhosKeyboardLayout)
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(super::keyboard::OhosKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, _callback: Box<dyn FnMut()>) {
        // Not supported on OHOS
    }

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }

    fn on_thermal_state_change(&self, _callback: Box<dyn FnMut()>) {}

    fn prevent_idle_sleep(&self, _reason: &str) -> Task<Result<ActivityGuard>> {
        let Some(app) = self.app.borrow().clone() else {
            return Task::ready(Err(anyhow::anyhow!("OpenHarmonyApp not set")));
        };
        let guards = self.idle_sleep_guards.clone();
        let background = self.background_executor.clone();
        self.background_executor.spawn(async move {
            let client = WindowClient::new(&app)?;
            client.set_keep_screen_on(0, true).await?;
            guards.fetch_add(1, Ordering::AcqRel);
            Ok(ActivityGuard::new(move || {
                if guards.fetch_sub(1, Ordering::AcqRel) == 1 {
                    background
                        .spawn(async move {
                            if let Err(error) = client.set_keep_screen_on(0, false).await {
                                warn!("Failed to restore OHOS idle sleep: {error}");
                            }
                        })
                        .detach();
                }
            }))
        })
    }

    fn read_from_primary(&self) -> Option<ClipboardItem> {
        None
    }

    fn write_to_primary(&self, _item: ClipboardItem) {}
}
