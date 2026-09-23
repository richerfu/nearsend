use gpui::{
    hsla, px, size, Anchor, App, AppContext, Application, ApplicationHandle, Bounds, Global,
    WindowBounds, WindowOptions,
};
use gpui_component::theme::Theme;
use gpui_component::Root;
use gpui_kit_assets::Assets as ComponentAssets;

use log::LevelFilter;
use ohos_hilog_binding::log::Config;
use openharmony_ability::OpenHarmonyApp;
use openharmony_ability_plugin_files::FilesBridgePlugin;
use openharmony_ability_plugin_permission::PermissionBridgePlugin;

mod app;
mod assets;
mod core;
mod platform;
mod state;
mod ui;

use ui::router_history::RouterHistoryState;

#[cfg(feature = "ohos-multiwindow-smoke")]
struct MultiWindowSmoke {
    clicks: u32,
    clipboard: String,
    picker: String,
    capture: String,
    capture_stream: Option<Box<dyn gpui::ScreenCaptureStream>>,
    focus: gpui::FocusHandle,
    key_input: String,
}

#[cfg(feature = "ohos-multiwindow-smoke")]
impl gpui::Render for MultiWindowSmoke {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::prelude::*;
        gpui::div()
            .size_full()
            .bg(gpui::rgb(0xfafafa))
            .p(px(24.))
            .pt(px(72.))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                gpui::div()
                    .id("multi-window-click")
                    .child(format!("Second GPUI window — clicks: {}", self.clicks))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.clicks += 1;
                        cx.notify();
                    })),
            )
            .child(
                gpui::div()
                    .id("multi-window-clipboard")
                    .child(format!("Clipboard check: {}", self.clipboard))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            "gpui-ohos clipboard smoke".into(),
                        ));
                        let read = cx.read_from_clipboard_async();
                        this.clipboard = "reading".into();
                        cx.notify();
                        cx.spawn(async move |this, cx| {
                            let result = read.await;
                            let _ = this.update(cx, |this, cx| {
                                this.clipboard = format!("{result:?}");
                                cx.notify();
                            });
                        })
                        .detach();
                    })),
            )
            .child(
                gpui::div()
                    .id("multi-window-picker")
                    .child(format!("Open file picker: {}", self.picker))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        let dialog = cx.prompt_for_paths(gpui::PathPromptOptions {
                            files: true,
                            directories: false,
                            multiple: false,
                            prompt: None,
                        });
                        this.picker = "opening".into();
                        cx.notify();
                        cx.spawn(async move |this, cx| {
                            let result = dialog.await;
                            let _ = this.update(cx, |this, cx| {
                                this.picker = format!("{result:?}");
                                cx.notify();
                            });
                        })
                        .detach();
                    })),
            )
            .child(
                gpui::div()
                    .id("multi-window-capture")
                    .child(format!("Screen capture: {}", self.capture))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        if this.capture_stream.take().is_some() {
                            this.capture = "stopped".into();
                            cx.notify();
                            return;
                        }
                        let sources = cx.screen_capture_sources();
                        let executor = cx.foreground_executor().clone();
                        this.capture = "requesting consent".into();
                        cx.notify();
                        cx.spawn(async move |this, cx| {
                            let result: anyhow::Result<Box<dyn gpui::ScreenCaptureStream>> =
                                async {
                                    let source = sources
                                        .await??
                                        .into_iter()
                                        .next()
                                        .ok_or_else(|| anyhow::anyhow!("No capture display"))?;
                                    let count =
                                        std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                                    let stream = source
                                        .stream(
                                            &executor,
                                            Box::new(move |frame| {
                                                let number = count.fetch_add(
                                                    1,
                                                    std::sync::atomic::Ordering::Relaxed,
                                                ) + 1;
                                                if number <= 3 {
                                                    log::info!(
                                                        "OHOS screen capture frame {number}: {}x{}",
                                                        frame.0.width(),
                                                        frame.0.height()
                                                    );
                                                }
                                            }),
                                        )
                                        .await??;
                                    Ok(stream)
                                }
                                .await;
                            let _ = this.update(cx, |this, cx| {
                                match result {
                                    Ok(stream) => {
                                        this.capture_stream = Some(stream);
                                        this.capture = "requested; tap to stop".into();
                                    }
                                    Err(error) => this.capture = format!("error: {error}"),
                                }
                                cx.notify();
                            });
                        })
                        .detach();
                    })),
            )
            .child(
                gpui::div()
                    .id("multi-window-keyboard")
                    .track_focus(&self.focus)
                    .child(format!("Keyboard check: {}", self.key_input))
                    .on_click(cx.listener(|this, _event, window, cx| {
                        window.focus(&this.focus, cx);
                    }))
                    .on_key_down(
                        cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                            this.key_input = format!("{:?}", event.keystroke);
                            cx.notify();
                        }),
                    ),
            )
            .child(
                gpui::div()
                    .id("multi-window-resize")
                    .child("Resize window to 500 x 300")
                    .on_click(cx.listener(|_this, _event, window, _cx| {
                        window.resize(size(px(500.), px(300.)));
                    })),
            )
    }
}

thread_local! {
    static GPUI_APPLICATION: std::cell::RefCell<Option<ApplicationHandle>> = const {
        std::cell::RefCell::new(None)
    };
}

/// Canonical application version used by every in-app version label.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Global OpenHarmony app wrapper for accessing back press functionality
pub struct GlobalOpenHarmonyApp(OpenHarmonyApp);

impl Global for GlobalOpenHarmonyApp {}

#[openharmony_ability_derive::ability]
pub fn openharmony_app(app: OpenHarmonyApp) {
    let log_level = if cfg!(feature = "ohos-multiwindow-smoke") {
        LevelFilter::Info
    } else {
        LevelFilter::Debug
    };
    ohos_hilog_binding::log::init_once(Config::default().with_max_level(log_level));
    if let Err(error) = app.register_plugin(PermissionBridgePlugin) {
        log::error!("Failed to register OpenHarmony permission plugin: {error}");
    }
    if let Err(error) = app.register_plugin(FilesBridgePlugin) {
        log::error!("Failed to register OpenHarmony files plugin: {error}");
    }
    if let Err(error) = app.register_plugin(platform::openharmony::NearSendPlatformBridgePlugin) {
        log::error!("Failed to register NearSend platform plugin: {error}");
    }
    if let Err(error) = platform::openharmony::set_app(app.clone()) {
        log::error!("Failed to store OpenHarmony app: {error}");
    }
    if let Some(pref_path) = app.pref_path().or_else(|| app.base_path()) {
        if let Err(error) = platform::preferences_path::set_preferences_path(pref_path) {
            log::error!("Failed to initialize preferences path: {error}");
        }
    }

    let inner_app = app.clone();
    // Initialize and run GPUI application
    // The event loop is automatically integrated by the platform
    let application = Application::with_platform(gpui_ohos::current_platform(app.clone(), false))
        .with_assets(assets::NearSendAssets(ComponentAssets));

    let application_handle = application.run_embedded(move |cx: &mut App| {
        cx.set_global(GlobalOpenHarmonyApp(app.clone()));

        gpui_component::init(cx);
        gpui_router::init(cx);
        ui::theme::apply_nearsend_theme(cx);
        RouterHistoryState::init(cx, "/");
        Theme::global_mut(cx).overlay = hsla(0.0, 0.0, 0.0, 0.58);
        Theme::global_mut(cx).notification.placement = Anchor::BottomCenter;
        Theme::global_mut(cx).notification.margins.bottom = px(72.);

        // Create a shared tokio runtime on a background thread.
        // All async work (server, transfers, discovery) goes through this handle.
        let tokio_handle = {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(2)
                    .thread_name("near-send-tokio")
                    .build()
                    .expect("Failed to create tokio runtime");
                tx.send(rt.handle().clone()).unwrap();
                // Keep the runtime alive for the lifetime of the app
                rt.block_on(std::future::pending::<()>());
            });
            rx.recv().expect("Failed to receive tokio handle")
        };

        let info = inner_app.content_rect();
        let default_size = size(px(info.width as _), px(info.height as _));
        let bounds = Bounds::centered(None, default_size, cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| {
                    let server = cx.new(|_| core::server::ServerManager::new(53317));

                    // Generate self-signed cert for TLS
                    let cert = match core::cert::generate_self_signed_cert() {
                        Ok(cert) => {
                            log::info!("Generated self-signed TLS certificate");
                            Some(cert)
                        }
                        Err(e) => {
                            log::error!("Failed to generate TLS certificate: {}", e);
                            None
                        }
                    };

                    let app_state = cx.new(|_| {
                        let mut state =
                            state::app_state::AppState::new(server.clone(), tokio_handle.clone());
                        state.cert = cert;
                        state
                    });
                    let device_state = cx.new(|_| state::device_state::DeviceState::new());
                    let transfer_state = cx.new(|_| state::transfer_state::TransferState::new());
                    let history_state = cx.new(|_| state::history_state::HistoryState::new());
                    app::AppRoot::new(cx, app_state, device_state, transfer_state, history_state)
                });
                cx.new(|cx| Root::new(view, window, cx).window_shadow_size(px(0.)))
            },
        )
        .unwrap();
        #[cfg(feature = "ohos-multiwindow-smoke")]
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    gpui::point(px(520.), px(320.)),
                    size(px(600.), px(360.)),
                ))),
                ..Default::default()
            },
            |_window, cx| {
                cx.new(|cx| MultiWindowSmoke {
                    clicks: 0,
                    clipboard: "tap to test".into(),
                    picker: "tap to test".into(),
                    capture: "tap to test".into(),
                    capture_stream: None,
                    focus: cx.focus_handle(),
                    key_input: "click here, then press a key".into(),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });

    GPUI_APPLICATION.with(|application| {
        application.replace(Some(application_handle));
    });
}
