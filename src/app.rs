//! App entry: router root. Uses gpui-router to show Home (tabs), History,
//! Progress, or Selected Files based on pathname.

use crate::state::{
    app_state::AppState, device_state::DeviceState, history_state::HistoryState,
    receive_inbox_state::ReceiveInboxState, send_selection_state::SendSelectionState,
    transfer_state::TransferState,
};
use crate::ui::pages::{
    AboutPage, ChangelogPage, DonatePage, HistoryPage, HomePage, OpenSourceLicensesPage,
    ProgressPage, ReceiveIncomingPage, SelectedFilesPage, WebSendPage,
};
use crate::ui::router_history::{HistoryEntry, RouterHistoryState};
use crate::ui::routes;
use gpui::{div, prelude::*, Context, Entity, IntoElement, MouseButton, ReadGlobal, Window};
use gpui_component::{v_flex, ActiveTheme as _, Root, WindowExt as _};
use gpui_router::{Route, RouterState, Routes};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::unbounded_channel;

/// Router root: shows Home (tabs), History, Progress, or Selected Files.
pub struct AppRoot {
    home_entity: Entity<HomePage>,
    history_entity: Entity<HistoryPage>,
    about_entity: Entity<AboutPage>,
    changelog_entity: Entity<ChangelogPage>,
    donate_entity: Entity<DonatePage>,
    open_source_licenses_entity: Entity<OpenSourceLicensesPage>,
    progress_entity: Entity<ProgressPage>,
    selected_files_entity: Entity<SelectedFilesPage>,
    receive_incoming_entity: Entity<ReceiveIncomingPage>,
    web_send_entity: Entity<WebSendPage>,
    incoming_event_listener_started: bool,
    back_press_can_intercept: Arc<AtomicBool>,
}

impl AppRoot {
    fn is_home_path(pathname: &str) -> bool {
        pathname.is_empty() || pathname == routes::HOME
    }

    pub fn new(
        cx: &mut Context<Self>,
        app_state: Entity<AppState>,
        device_state: Entity<DeviceState>,
        transfer_state: Entity<TransferState>,
        history_state: Entity<HistoryState>,
    ) -> Self {
        // Get root entity before creating child entities
        let root = cx.entity();

        let send_selection_state = cx.new(|_| SendSelectionState::default());
        let receive_inbox_state = cx.new(|_| ReceiveInboxState::default());
        let home_entity = cx.new(|_| {
            HomePage::new(
                app_state.clone(),
                device_state.clone(),
                transfer_state.clone(),
                history_state.clone(),
                send_selection_state.clone(),
                receive_inbox_state.clone(),
            )
        });
        let history_entity = cx.new(|_| {
            HistoryPage::new()
                .with_root(root.clone())
                .with_history_state(history_state)
                .with_receive_inbox_state(receive_inbox_state.clone())
        });
        let about_entity = cx.new(|_| AboutPage::new(root.clone()));
        let changelog_entity = cx.new(|_| ChangelogPage::new(root.clone()));
        let donate_entity = cx.new(|_| DonatePage::new(root.clone()));
        let open_source_licenses_entity = cx.new(|_| OpenSourceLicensesPage::new(root.clone()));
        let progress_entity = cx.new(|_| {
            ProgressPage::new(
                root.clone(),
                transfer_state,
                crate::state::transfer_state::TransferDirection::Send,
            )
        });
        let selected_files_entity = cx.new(|_| {
            SelectedFilesPage::new(
                root.clone(),
                app_state.clone(),
                send_selection_state.clone(),
            )
        });
        let receive_incoming_entity = cx.new(|_| {
            ReceiveIncomingPage::new(root.clone(), app_state.clone(), receive_inbox_state)
        });
        let web_send_entity = cx.new(|_| WebSendPage::new(root.clone(), home_entity.clone()));

        let back_press_can_intercept = Arc::new(AtomicBool::new(false));

        // Initialize back press handler
        let this = Self {
            home_entity,
            history_entity,
            about_entity,
            changelog_entity,
            donate_entity,
            open_source_licenses_entity,
            progress_entity,
            selected_files_entity,
            receive_incoming_entity,
            web_send_entity,
            incoming_event_listener_started: false,
            back_press_can_intercept: back_press_can_intercept.clone(),
        };

        // Initialize router history with initial path
        let initial_path = RouterState::global(cx).location.pathname.clone();
        let normalized_initial = if Self::is_home_path(initial_path.as_ref()) {
            routes::HOME.to_string()
        } else {
            initial_path.to_string()
        };
        RouterHistoryState::global_mut(cx)
            .history
            .reset(HistoryEntry::new(normalized_initial));
        this.sync_back_press_capability(cx);

        // Setup back press interceptor
        let back_press_can_intercept_clone = this.back_press_can_intercept.clone();
        let (back_press_tx, mut back_press_rx) = unbounded_channel::<()>();

        cx.spawn(async move |this, cx| {
            while back_press_rx.recv().await.is_some() {
                let _ = this.update(cx, |this, cx| {
                    this.handle_system_back(cx);
                });
            }
        })
        .detach();

        let app = cx.global::<crate::GlobalOpenHarmonyApp>();
        let ohos_app = &app.0;
        let back_press_waker = ohos_app.create_waker();
        ohos_app.on_back_press_intercept(move || {
            let can_intercept = back_press_can_intercept_clone.load(Ordering::SeqCst);
            if !can_intercept {
                false
            } else {
                let _ = back_press_tx.send(());
                back_press_waker.wake();
                true
            }
        });

        this
    }

    fn sync_back_press_capability(&self, cx: &Context<Self>) {
        let can_go_back = RouterHistoryState::global(cx).history.can_go_back();
        let is_home = Self::is_home_path(RouterState::global(cx).location.pathname.as_ref());
        self.back_press_can_intercept
            .store(can_go_back || !is_home, Ordering::SeqCst);
    }

    fn handle_system_back(&mut self, cx: &mut Context<Self>) {
        if self.go_back(cx) {
            return;
        }

        let current_path = RouterState::global(cx).location.pathname.clone();
        if !Self::is_home_path(current_path.as_ref()) {
            RouterHistoryState::global_mut(cx)
                .history
                .reset(HistoryEntry::new(routes::HOME));
            RouterState::global_mut(cx).location.pathname = routes::HOME.into();
            self.sync_back_press_capability(cx);
            cx.notify();
        }
    }

    /// Navigate to a new path and add to history
    pub fn navigate_to(&mut self, pathname: &str, cx: &mut Context<Self>) {
        let current = RouterState::global(cx).location.pathname.clone();
        if current.as_ref() != pathname {
            let pathname_owned = pathname.to_string();
            RouterHistoryState::global_mut(cx)
                .history
                .push(HistoryEntry::new(pathname_owned.clone()));
            RouterState::global_mut(cx).location.pathname = pathname_owned.into();
            self.sync_back_press_capability(cx);
            cx.notify();
        }
    }

    /// Go back to previous path in history
    /// Returns true if successfully went back, false if at root
    pub fn go_back(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(entry) = RouterHistoryState::global_mut(cx).history.go_back() {
            RouterState::global_mut(cx).location.pathname = entry.pathname;
            self.sync_back_press_capability(cx);
            cx.notify();
            true
        } else {
            self.sync_back_press_capability(cx);
            false
        }
    }

    /// Pop one history entry. If history is unavailable, navigate to fallback path.
    pub fn go_back_or_navigate(&mut self, fallback: &str, cx: &mut Context<Self>) {
        if !self.go_back(cx) {
            self.navigate_to(fallback, cx);
        }
    }
}

impl gpui::Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_back_press_capability(cx);

        if !self.incoming_event_listener_started {
            self.incoming_event_listener_started = true;
            cx.spawn(async move |this, cx| loop {
                crate::core::receive_events::wait_for_incoming_event().await;
                if this
                    .update(cx, |this, cx| {
                        let _ = this.home_entity.update(cx, |home, cx| {
                            home.poll_incoming_events(cx);
                        });
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            })
            .detach();
        }

        let _ = self.home_entity.update(cx, |home, cx| {
            home.poll_send_cancel_event(window, cx);
            home.poll_send_retry_event(window, cx);
        });

        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let has_dialog = dialog_layer.is_some();

        // Clone entities for use in closures
        let home_entity = self.home_entity.clone();
        let history_entity = self.history_entity.clone();
        let about_entity = self.about_entity.clone();
        let changelog_entity = self.changelog_entity.clone();
        let donate_entity = self.donate_entity.clone();
        let open_source_licenses_entity = self.open_source_licenses_entity.clone();
        let progress_entity = self.progress_entity.clone();
        let selected_files_entity = self.selected_files_entity.clone();
        let receive_incoming_entity = self.receive_incoming_entity.clone();
        let web_send_entity = self.web_send_entity.clone();

        // Routes component automatically reads from RouterState
        let routed_page = Routes::new()
            .basename("/")
            .child(
                Route::new()
                    .path(routes::to_pattern(routes::RECEIVE_HISTORY))
                    .element({
                        let history_entity = history_entity.clone();
                        move |_window, _cx| history_entity.clone().into_any_element()
                    }),
            )
            .child(
                Route::new()
                    .path(routes::to_pattern(routes::SETTINGS_ABOUT))
                    .element({
                        let about_entity = about_entity.clone();
                        move |_window, _cx| about_entity.clone().into_any_element()
                    }),
            )
            .child(
                Route::new()
                    .path(routes::to_pattern(routes::SETTINGS_CHANGELOG))
                    .element({
                        let changelog_entity = changelog_entity.clone();
                        move |_window, _cx| changelog_entity.clone().into_any_element()
                    }),
            )
            .child(
                Route::new()
                    .path(routes::to_pattern(routes::SETTINGS_DONATE))
                    .element({
                        let donate_entity = donate_entity.clone();
                        move |_window, _cx| donate_entity.clone().into_any_element()
                    }),
            )
            .child(
                Route::new()
                    .path(routes::to_pattern(routes::SETTINGS_OPEN_SOURCE_LICENSES))
                    .element({
                        let open_source_licenses_entity = open_source_licenses_entity.clone();
                        move |_window, _cx| open_source_licenses_entity.clone().into_any_element()
                    }),
            )
            .child(
                Route::new()
                    .path(routes::to_pattern(routes::RECEIVE_INCOMING))
                    .element({
                        let receive_incoming_entity = receive_incoming_entity.clone();
                        move |_window, _cx| receive_incoming_entity.clone().into_any_element()
                    }),
            )
            .child(
                Route::new()
                    .path(routes::to_pattern(routes::TRANSFER_PROGRESS))
                    .element({
                        let progress_entity = progress_entity.clone();
                        move |_window, _cx| progress_entity.clone().into_any_element()
                    }),
            )
            .child(
                Route::new()
                    .path(routes::to_pattern(routes::SEND_FILES))
                    .element({
                        let selected_files_entity = selected_files_entity.clone();
                        move |_window, _cx| selected_files_entity.clone().into_any_element()
                    }),
            )
            .child(
                Route::new()
                    .path(routes::to_pattern(routes::SEND_LINK))
                    .element({
                        let web_send_entity = web_send_entity.clone();
                        move |_window, _cx| web_send_entity.clone().into_any_element()
                    }),
            )
            .child(Route::new().index().element({
                let home_entity = home_entity.clone();
                move |_window, _cx| home_entity.clone().into_any_element()
            }))
            .child(Route::new().path("{*not_found}").element({
                let home_entity = home_entity.clone();
                move |_window, _cx| home_entity.clone().into_any_element()
            }));

        let insets = crate::ui::safe_area::current(cx);

        v_flex()
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .child(
                div()
                    .size_full()
                    .pt(insets.top)
                    .pb(insets.bottom)
                    .pl(insets.left)
                    .pr(insets.right)
                    .child(div().size_full().child(routed_page)),
            )
            .children(sheet_layer)
            // gpui-component 9890a77 moved the dialog dim onto an
            // `absolute().size_full()` backdrop nested in an unsized wrapper,
            // so the mask laid out as 0x0. Paint a window-sized scrim here.
            .when(has_dialog, |this| {
                this.child(
                    div()
                        .id("dialog-scrim")
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .occlude()
                        .bg(cx.theme().overlay)
                        .on_mouse_down(MouseButton::Left, |_, window, cx| {
                            window.close_dialog(cx);
                        }),
                )
            })
            .children(dialog_layer)
            .children(notification_layer)
    }
}
