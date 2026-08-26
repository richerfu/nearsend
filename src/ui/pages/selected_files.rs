//! Selected files page: review and manage files before sending.
//! Route: /send/files

use crate::state::app_state::AppState;
use crate::state::send_selection_state::SendSelectionState;
use crate::ui::components::chrome::{
    back_icon_button, dialog_title, empty_state, page_header,
};
use crate::ui::icons::{app_icon, paths};
use crate::ui::routes;
use crate::ui::theme::{radius, spacing};
use gpui::{div, hsla, prelude::*, px, Context, Entity, Window};
use gpui_component::input::{Input, InputState};
use gpui_component::notification::Notification;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex, v_flex, ActiveTheme as _, Size, WindowExt as _,
};
use gpui_router::RouterState;

enum ClipboardPickOutcome {
    Success(String),
    Empty,
    PermissionDenied,
    ReadFailed,
}

enum PathPickOutcome {
    Success(Vec<(String, std::path::PathBuf)>),
    Cancelled,
    Failed,
}

/// Selected files page state.
pub struct SelectedFilesPage {
    pub root: Option<Entity<crate::app::AppRoot>>,
    app_state: Entity<AppState>,
    send_selection_state: Entity<SendSelectionState>,
}

impl SelectedFilesPage {
    pub fn new(
        root: Entity<crate::app::AppRoot>,
        app_state: Entity<AppState>,
        send_selection_state: Entity<SendSelectionState>,
    ) -> Self {
        Self {
            root: Some(root),
            app_state,
            send_selection_state,
        }
    }

    fn open_notice_dialog(&self, msg: &str, window: &mut Window, cx: &mut Context<Self>) {
        let msg = msg.to_string();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(dialog_title("提示"))
                .overlay(true)
                .w(px(320.))
                .child(div().w_full().text_sm().child(msg.clone()))
                .footer(build_alert_dialog_footer("selected-files-notice", "确定"))
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("确定"))
        });
    }

    fn show_clipboard_empty_toast(&self, window: &mut Window, cx: &mut Context<Self>) {
        struct ClipboardEmptyToast;
        window.push_notification(
            Notification::new()
                .id::<ClipboardEmptyToast>()
                .autohide(false)
                .content(|_, _, _| {
                    div()
                        .w_full()
                        .text_xs()
                        .text_center()
                        .child("当前剪贴板无内容")
                        .into_any_element()
                })
                .w(px(158.))
                .py(px(4.))
                .px(px(10.))
                .mb(px(22.))
                .rounded_full()
                .shadow_none()
                .border_color(hsla(0.0, 0.0, 0.0, 0.0))
                .bg(hsla(0.0, 0.0, 0.12, 0.92))
                .text_color(hsla(0.0, 0.0, 1.0, 0.96)),
            cx,
        );
        let window_handle = window.window_handle();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();
        let dismiss = tokio_handle.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        });
        cx.spawn(async move |_this, cx| {
            let _ = dismiss.await;
            let _ = window_handle.update(cx, |_, window, cx| {
                window.remove_notification::<ClipboardEmptyToast>(cx);
            });
        })
        .detach();
    }

    fn open_text_edit_dialog(
        &self,
        index: usize,
        initial: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(3, 5)
                .placeholder("输入文本内容")
                .default_value(initial)
                .soft_wrap(true)
        });
        let send_state = self.send_selection_state.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_ok = input_state.clone();
            let send_state_for_ok = send_state.clone();
            dialog
                .title(dialog_title("编辑文本"))
                .overlay(true)
                .w(px(360.))
                .child(
                    div()
                        .w_full()
                        .child(Input::new(&input_state).appearance(true)),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("确认")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(build_confirm_dialog_footer(
                    "selected-files-text-edit",
                    "确认",
                    "取消",
                ))
                .on_ok(move |_event, _window, cx| {
                    let text = input_for_ok.read(cx).value().to_string();
                    if text.is_empty() {
                        return false;
                    }
                    send_state_for_ok.update(cx, |state, _| {
                        if index == usize::MAX {
                            state.add_text(text.clone());
                        } else {
                            state.update_text(index, text.clone());
                        }
                    });
                    true
                })
        });
    }

    fn open_add_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let page_text = page.clone();
            let page_file = page.clone();
            let page_folder = page.clone();
            let page_clipboard = page.clone();
            let variant = ButtonCustomVariant::new(_cx)
                .color(_cx.theme().foreground.opacity(0.08))
                .foreground(_cx.theme().foreground)
                .hover(_cx.theme().foreground.opacity(0.12))
                .active(_cx.theme().foreground.opacity(0.16));
            dialog
                .title(dialog_title("你想加入什么文件？"))
                .overlay(true)
                .w(px(340.))
                .child(
                    h_flex()
                        .w_full()
                        .gap(px(10.))
                        .flex_wrap()
                        .justify_start()
                        .child(
                            Button::new("selected-add-file")
                                .custom(variant.clone())
                                .w(px(90.))
                                .h(px(65.))
                                .rounded_md()
                                .border_1()
                                .border_color(_cx.theme().border)
                                .on_click(move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    page_file.update(cx, |this, cx| {
                                        this.add_from_system_picker(false, window, cx);
                                    });
                                })
                                .child(
                                    v_flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(4.))
                                        .child(
                                            app_icon(
                                                paths::FILE,
                                                Size::Medium,
                                                _cx.theme().foreground,
                                            ),
                                        )
                                        .child(div().text_sm().text_center().child("文件")),
                                ),
                        )
                        .child(
                            Button::new("selected-add-folder")
                                .custom(variant.clone())
                                .w(px(90.))
                                .h(px(65.))
                                .rounded_md()
                                .border_1()
                                .border_color(_cx.theme().border)
                                .on_click(move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    page_folder.update(cx, |this, cx| {
                                        this.add_from_system_picker(true, window, cx);
                                    });
                                })
                                .child(
                                    v_flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(4.))
                                        .child(
                                            app_icon(
                                                paths::FOLDER,
                                                Size::Medium,
                                                _cx.theme().foreground,
                                            ),
                                        )
                                        .child(div().text_sm().text_center().child("文件夹")),
                                ),
                        )
                        .child(
                            Button::new("selected-add-text")
                                .custom(variant.clone())
                                .w(px(90.))
                                .h(px(65.))
                                .rounded_md()
                                .border_1()
                                .border_color(_cx.theme().border)
                                .on_click(move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    page_text.update(cx, |this, cx| {
                                        this.open_text_edit_dialog(
                                            usize::MAX,
                                            String::new(),
                                            window,
                                            cx,
                                        );
                                    });
                                })
                                .child(
                                    v_flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(4.))
                                        .child(
                                            app_icon(
                                                paths::BOOK_OPEN,
                                                Size::Medium,
                                                _cx.theme().foreground,
                                            ),
                                        )
                                        .child(div().text_sm().text_center().child("文本")),
                                ),
                        )
                        .child(
                            Button::new("selected-add-clipboard")
                                .custom(variant)
                                .w(px(90.))
                                .h(px(65.))
                                .rounded_md()
                                .border_1()
                                .border_color(_cx.theme().border)
                                .on_click(move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    page_clipboard.update(cx, |this, cx| {
                                        this.add_from_clipboard(window, cx);
                                    });
                                })
                                .child(
                                    v_flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(4.))
                                        .child(
                                            app_icon(
                                                paths::COPY,
                                                Size::Medium,
                                                _cx.theme().foreground,
                                            ),
                                        )
                                        .child(div().text_sm().text_center().child("剪贴板")),
                                ),
                        ),
                )
                .footer(build_alert_dialog_footer("selected-files-add", "关闭"))
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("关闭"))
        });
    }

    fn add_from_clipboard(&self, window: &mut Window, cx: &mut Context<Self>) {
        let window_handle = window.window_handle();
        let page_entity = cx.entity();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();

        let join = tokio_handle.spawn(async move {
            let permission_granted =
                match crate::platform::clipboard::ensure_read_clipboard_permission().await {
                    Ok(granted) => granted,
                    Err(err) => {
                        log::error!("ensure read clipboard permission failed: {}", err);
                        false
                    }
                };
            if !permission_granted {
                return ClipboardPickOutcome::PermissionDenied;
            }

            let text = match crate::platform::clipboard::read_clipboard_text().await {
                Ok(text) => text,
                Err(err) => {
                    log::error!("read clipboard text failed: {}", err);
                    return ClipboardPickOutcome::ReadFailed;
                }
            };
            if text.is_empty() {
                return ClipboardPickOutcome::Empty;
            }

            ClipboardPickOutcome::Success(text)
        });

        cx.spawn(async move |_this, cx| {
            let outcome = match join.await {
                Ok(outcome) => outcome,
                Err(err) => {
                    log::error!("clipboard task failed: {}", err);
                    ClipboardPickOutcome::ReadFailed
                }
            };

            match outcome {
                ClipboardPickOutcome::Success(text) => {
                    if text.is_empty() {
                        return;
                    }
                    let _ = page_entity.update(cx, |this, cx| {
                        this.send_selection_state.update(cx, |state, _| {
                            state.add_text(text.clone());
                        });
                    });
                }
                ClipboardPickOutcome::Empty => {
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = page_entity.update(cx, |this, cx| {
                            this.show_clipboard_empty_toast(window, cx);
                        });
                    });
                }
                ClipboardPickOutcome::PermissionDenied => {
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = page_entity.update(cx, |this, cx| {
                            this.open_notice_dialog("无权限。请开启权限。", window, cx);
                        });
                    });
                }
                ClipboardPickOutcome::ReadFailed => {
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = page_entity.update(cx, |this, cx| {
                            this.open_notice_dialog("读取剪贴板失败。", window, cx);
                        });
                    });
                }
            }
        })
        .detach();
    }

    fn add_from_system_picker(
        &self,
        pick_folder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let window_handle = window.window_handle();
        let page_entity = cx.entity();
        let send_selection_state = self.send_selection_state.clone();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();

        let join = tokio_handle.spawn(async move {
            let uris = if pick_folder {
                crate::platform::file_picker::pick_folders().await
            } else {
                crate::platform::file_picker::pick_files().await
            };
            match uris {
                Ok(uris) => {
                    if uris.is_empty() {
                        PathPickOutcome::Cancelled
                    } else {
                        let picked = uris
                            .into_iter()
                            .filter_map(|uri| {
                                crate::platform::file_picker::picker_uri_to_path_with_uri(&uri)
                            })
                            .collect::<Vec<_>>();
                        if picked.is_empty() {
                            PathPickOutcome::Failed
                        } else {
                            PathPickOutcome::Success(picked)
                        }
                    }
                }
                Err(err) => {
                    log::error!("pick from system failed: {}", err);
                    PathPickOutcome::Failed
                }
            }
        });

        cx.spawn(async move |_this, cx| {
            let outcome = match join.await {
                Ok(outcome) => outcome,
                Err(err) => {
                    log::error!("picker task failed: {}", err);
                    PathPickOutcome::Failed
                }
            };

            match outcome {
                PathPickOutcome::Success(picked) => {
                    let mut added = 0usize;
                    let _ = send_selection_state.update(cx, |state, _| {
                        added = state.add_picker_paths_recursive(picked.clone());
                    });
                    if added > 0 {
                        return;
                    }
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = page_entity.update(cx, |this, cx| {
                            this.open_notice_dialog(
                                "未添加到可发送文件，请确认已授权并且文件可读。",
                                window,
                                cx,
                            );
                        });
                    });
                }
                PathPickOutcome::Cancelled => {}
                PathPickOutcome::Failed => {
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = page_entity.update(cx, |this, cx| {
                            this.open_notice_dialog("选择文件失败。", window, cx);
                        });
                    });
                }
            }
        })
        .detach();
    }
}

impl gpui::Render for SelectedFilesPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let files = self.send_selection_state.read(cx).items().to_vec();
        let total_size = self.send_selection_state.read(cx).total_size();
        let file_count = files.len();
        let send_state = self.send_selection_state.clone();
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(page_header(
                "选择",
                back_icon_button("files-back", cx, |this, window, cx| {
                    if let Some(root) = &this.root {
                        let _ = root.update(cx, |this, cx| {
                            this.go_back_or_navigate(routes::HOME, cx);
                        });
                    } else if let Some(entry) =
                        crate::ui::router_history::RouterHistoryState::global_mut(cx)
                            .history
                            .go_back()
                    {
                        RouterState::global_mut(cx).location.pathname = entry.pathname;
                    } else {
                        RouterState::global_mut(cx).location.pathname = routes::HOME.into();
                    }
                    window.refresh();
                }),
                div(),
                cx,
            ))
            .child(
                div().flex_1().w_full().overflow_y_scrollbar().child(
                    v_flex()
                        .w_full()
                        .px(spacing::PAGE)
                        .gap(spacing::SM)
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .items_center()
                                .child(
                                    v_flex()
                                        .gap(px(2.))
                                        .child(
                                            div()
                                                .text_lg()
                                                .text_color(cx.theme().foreground)
                                                .child(format!("文件： {}", file_count)),
                                        )
                                        .child(
                                            div()
                                                .text_lg()
                                                .text_color(cx.theme().foreground)
                                                .child(format!(
                                                    "大小： {}",
                                                    format_file_size(total_size)
                                                )),
                                        ),
                                )
                                .child(
                                    Button::new("files-clear-all")
                                        .primary()
                                        .on_click(cx.listener(
                                            move |_this, _event, _window, _cx| {
                                                send_state.update(_cx, |state, _| {
                                                    state.clear();
                                                });
                                            },
                                        ))
                                        .child("全部删除"),
                                ),
                        )
                        .children(files.iter().enumerate().map(|(i, file)| {
                            let file_name = file.name.clone();
                            let file_size = format_file_size(file.size);
                            let is_text = file.text_content.is_some();
                            let text = file.text_content.clone().unwrap_or_default();
                            let send_state_for_delete = self.send_selection_state.clone();
                            div()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border.opacity(0.8))
                                .rounded(radius::LG)
                                .p(px(12.))
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap(spacing::SM)
                                        .w_full()
                                        .child(
                                            div()
                                                .w(px(56.))
                                                .h(px(56.))
                                                .rounded(radius::MD)
                                                .bg(cx.theme().muted)
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(app_icon(
                                                    paths::BOOK_OPEN,
                                                    Size::Small,
                                                    cx.theme().foreground,
                                                )),
                                        )
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .gap(px(2.))
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .overflow_hidden()
                                                        .truncate()
                                                        .text_base()
                                                        .text_color(cx.theme().foreground)
                                                        .child(format!("\"{}\"", file_name)),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(file_size),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .gap(px(8.))
                                                .when(is_text, |this| {
                                                    this.child(
                                                        Button::new(format!("edit-file-{}", i))
                                                            .ghost()
                                                            .on_click(cx.listener(
                                                                move |this, _event, window, cx| {
                                                                    this.open_text_edit_dialog(
                                                                        i,
                                                                        text.clone(),
                                                                        window,
                                                                        cx,
                                                                    );
                                                                },
                                                            ))
                                                            .child("编辑"),
                                                    )
                                                })
                                                .child(
                                                    Button::new(format!("delete-file-{}", i))
                                                        .ghost()
                                                        .on_click(cx.listener(
                                                            move |_this, _event, _window, _cx| {
                                                                send_state_for_delete.update(
                                                                    _cx,
                                                                    |state, _| {
                                                                        state.remove(i);
                                                                    },
                                                                );
                                                            },
                                                        ))
                                                        .child(
                                                            app_icon(
                                                                paths::TRASH,
                                                                Size::Small,
                                                                cx.theme().danger,
                                                            ),
                                                        ),
                                                ),
                                        ),
                                )
                        }))
                        .when(files.is_empty(), |this| {
                            this.child(empty_state(
                                paths::FILE,
                                "暂无文件",
                                "点击下方添加按钮选择要发送的内容",
                                cx,
                            ))
                        }),
                ),
            )
            .child(
                div().w_full().px(spacing::PAGE).py(px(15.)).child(
                    h_flex().justify_end().items_center().child(
                        Button::new("add-more-files")
                            .primary()
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.open_add_dialog(window, cx);
                            }))
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        app_icon(
                                            paths::PLUS,
                                            Size::Small,
                                            cx.theme().primary_foreground,
                                        ),
                                    )
                                    .child("添加"),
                            ),
                    ),
                ),
            )
    }
}

fn build_confirm_dialog_footer(id_prefix: &str, ok_text: &str, cancel_text: &str) -> DialogFooter {
    DialogFooter::new()
        .child(
            DialogClose::new()
                .child(Button::new(format!("{id_prefix}-cancel")).label(cancel_text.to_string())),
        )
        .child(
            DialogAction::new().child(
                Button::new(format!("{id_prefix}-ok"))
                    .label(ok_text.to_string())
                    .primary(),
            ),
        )
}

fn build_alert_dialog_footer(id_prefix: &str, ok_text: &str) -> DialogFooter {
    DialogFooter::new().child(
        DialogAction::new().child(
            Button::new(format!("{id_prefix}-ok"))
                .label(ok_text.to_string())
                .primary(),
        ),
    )
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
