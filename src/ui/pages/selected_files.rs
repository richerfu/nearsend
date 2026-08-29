//! Selected files page: review and manage files before sending.
//! Route: /send/files

use crate::state::app_state::AppState;
use crate::state::send_selection_state::{SendSelectionItem, SendSelectionState};
use crate::ui::components::chrome::{
    back_icon_button, content_picker_grid, content_picker_tile, dialog_title, empty_state,
    page_header,
};
use crate::ui::icons::{app_icon, paths};
use crate::ui::routes;
use crate::ui::theme::{radius, sizing, spacing};
use crate::ui::utils::format_file_size;
use gpui::{div, hsla, prelude::*, px, AnyElement, Context, Entity, Hsla, Window};
use gpui_component::input::{Textarea, TextareaState};
use gpui_component::notification::Notification;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex, v_flex, ActiveTheme as _, Size, StyledExt as _, WindowExt as _,
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
            TextareaState::new(window, cx)
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
                        .child(Textarea::new(&input_state).appearance(true)),
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
                    send_state_for_ok.update(cx, |state, state_cx| {
                        if index == usize::MAX {
                            state.add_text(text.clone());
                        } else {
                            state.update_text(index, text.clone());
                        }
                        state_cx.notify();
                    });
                    true
                })
        });
    }

    fn open_add_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity();
        window.open_dialog(cx, move |dialog, _window, cx| {
            let page_text = page.clone();
            let page_file = page.clone();
            let page_folder = page.clone();
            let page_clipboard = page.clone();
            dialog
                .title(dialog_title("添加内容"))
                .overlay(true)
                .w(px(340.))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(12.))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("选择要加入的内容类型"),
                        )
                        .child(content_picker_grid([
                            content_picker_tile(
                                "selected-add-file",
                                paths::FILE,
                                "文件",
                                cx,
                                move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    page_file.update(cx, |this, cx| {
                                        this.add_from_system_picker(false, window, cx);
                                    });
                                },
                            ),
                            content_picker_tile(
                                "selected-add-folder",
                                paths::FOLDER,
                                "文件夹",
                                cx,
                                move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    page_folder.update(cx, |this, cx| {
                                        this.add_from_system_picker(true, window, cx);
                                    });
                                },
                            ),
                            content_picker_tile(
                                "selected-add-text",
                                paths::BOOK_OPEN,
                                "文本",
                                cx,
                                move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    page_text.update(cx, |this, cx| {
                                        this.open_text_edit_dialog(
                                            usize::MAX,
                                            String::new(),
                                            window,
                                            cx,
                                        );
                                    });
                                },
                            ),
                            content_picker_tile(
                                "selected-add-clipboard",
                                paths::COPY,
                                "剪贴板",
                                cx,
                                move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    page_clipboard.update(cx, |this, cx| {
                                        this.add_from_clipboard(window, cx);
                                    });
                                },
                            ),
                        ])),
                )
                .footer(build_close_footer("selected-files-add", "关闭"))
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
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = page_entity.update(cx, |this, cx| {
                            this.send_selection_state.update(cx, |state, state_cx| {
                                state.add_text(text.clone());
                                state_cx.notify();
                            });
                            cx.notify();
                        });
                        window.refresh();
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
                    let _ = send_selection_state.update(cx, |state, state_cx| {
                        added = state.add_picker_paths_recursive(picked.clone());
                        if added > 0 {
                            state_cx.notify();
                        }
                    });
                    if added > 0 {
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = page_entity.update(cx, |_this, cx| cx.notify());
                            window.refresh();
                        });
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

    fn go_back(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(root) = &self.root {
            let _ = root.update(cx, |this, cx| {
                this.go_back_or_navigate(routes::HOME, cx);
            });
        } else if let Some(entry) = crate::ui::router_history::RouterHistoryState::global_mut(cx)
            .history
            .go_back()
        {
            RouterState::global_mut(cx).location.pathname = entry.pathname;
        } else {
            RouterState::global_mut(cx).location.pathname = routes::HOME.into();
        }
        window.refresh();
    }
}

fn file_kind_icon(file: &SendSelectionItem) -> &'static str {
    if file.text_content.is_some() {
        return paths::BOOK_OPEN;
    }
    let lower = file.name.to_lowercase();
    if lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".bmp")
        || lower.ends_with(".svg")
    {
        paths::IMAGE
    } else {
        paths::FILE
    }
}

fn item_title(file: &SendSelectionItem) -> String {
    if let Some(text) = file.text_content.as_ref() {
        let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            "文本".to_string()
        } else {
            format!("\"{collapsed}\"")
        }
    } else if file.name.trim().is_empty() {
        "未命名文件".to_string()
    } else {
        file.name.clone()
    }
}

fn row_icon_action(
    id: String,
    icon: &'static str,
    color: Hsla,
    cx: &mut Context<SelectedFilesPage>,
    on_click: impl Fn(&mut SelectedFilesPage, &mut Window, &mut Context<SelectedFilesPage>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .w(px(36.))
        .h(px(36.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _event, window, cx| {
            on_click(this, window, cx);
        }))
        .child(app_icon(icon, Size::Small, color))
        .into_any_element()
}

fn render_file_row(
    index: usize,
    file: &SendSelectionItem,
    cx: &mut Context<SelectedFilesPage>,
) -> AnyElement {
    let is_text = file.text_content.is_some();
    let text = file.text_content.clone().unwrap_or_default();
    let title = item_title(file);
    let size = format_file_size(file.size);

    h_flex()
        .w_full()
        .min_w(px(0.))
        .items_center()
        .min_h(px(56.))
        .py(px(8.))
        .gap(px(12.))
        .child(
            div()
                .w(px(40.))
                .h(px(40.))
                .rounded(radius::MD)
                .bg(cx.theme().muted)
                .flex()
                .items_center()
                .justify_center()
                .flex_none()
                .child(app_icon(
                    file_kind_icon(file),
                    Size::Small,
                    cx.theme().primary,
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
                        .min_w(px(0.))
                        .overflow_hidden()
                        .truncate()
                        .text_sm()
                        .font_semibold()
                        .text_color(cx.theme().foreground)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(size),
                ),
        )
        .when(is_text, |this| {
            this.child(row_icon_action(
                format!("edit-file-{index}"),
                paths::PENCIL,
                cx.theme().muted_foreground,
                cx,
                move |this, window, cx| {
                    this.open_text_edit_dialog(index, text.clone(), window, cx);
                },
            ))
        })
        .child(row_icon_action(
            format!("delete-file-{index}"),
            paths::TRASH,
            cx.theme().danger,
            cx,
            move |this, window, cx| {
                let remaining = this.send_selection_state.update(cx, |state, state_cx| {
                    state.remove(index);
                    state_cx.notify();
                    state.items().len()
                });
                if remaining == 0 {
                    this.go_back(window, cx);
                }
            },
        ))
        .into_any_element()
}

impl gpui::Render for SelectedFilesPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let files = self.send_selection_state.read(cx).items().to_vec();
        let total_size = self.send_selection_state.read(cx).total_size();
        let file_count = files.len();
        let has_files = file_count > 0;

        v_flex()
            .size_full()
            .child(page_header(
                "选择",
                back_icon_button("files-back", cx, |this, window, cx| {
                    this.go_back(window, cx);
                }),
                div(),
                cx,
            ))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .min_w(px(0.))
                    .overflow_x_hidden()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .w_full()
                            .min_w(px(0.))
                            .px(spacing::PAGE)
                            .pt(px(12.))
                            .pb(px(12.))
                            .gap(px(12.))
                            .when(has_files, |this| {
                                this.child(
                                    h_flex()
                                        .w_full()
                                        .min_w(px(0.))
                                        .items_center()
                                        .gap(px(12.))
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .gap(px(2.))
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_semibold()
                                                        .text_color(cx.theme().foreground)
                                                        .child(format!("{file_count} 个文件")),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(format_file_size(total_size)),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id("files-clear-all")
                                                .flex_none()
                                                .px(px(10.))
                                                .py(px(6.))
                                                .rounded(radius::FULL)
                                                .cursor_pointer()
                                                .on_click(cx.listener(
                                                    |this, _event, window, cx| {
                                                        this.send_selection_state.update(
                                                            cx,
                                                            |state, state_cx| {
                                                                state.clear();
                                                                state_cx.notify();
                                                            },
                                                        );
                                                        this.go_back(window, cx);
                                                    },
                                                ))
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_medium()
                                                        .text_color(cx.theme().danger)
                                                        .child("全部删除"),
                                                ),
                                        ),
                                )
                            })
                            .when(has_files, |this| {
                                let mut group = v_flex()
                                    .w_full()
                                    .min_w(px(0.))
                                    .bg(cx.theme().background)
                                    .border_1()
                                    .border_color(cx.theme().border.opacity(0.75))
                                    .rounded(radius::LG)
                                    .px(px(14.));
                                for (index, file) in files.iter().enumerate() {
                                    if index > 0 {
                                        group = group.child(
                                            div()
                                                .h(px(1.))
                                                .ml(px(52.))
                                                .bg(cx.theme().border.opacity(0.7)),
                                        );
                                    }
                                    group = group.child(render_file_row(index, file, cx));
                                }
                                this.child(group)
                            })
                            .when(!has_files, |this| {
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
                div()
                    .w_full()
                    .flex_none()
                    .px(spacing::PAGE)
                    .pt(px(10.))
                    .pb(px(12.))
                    .border_t_1()
                    .border_color(cx.theme().border.opacity(0.65))
                    .bg(cx.theme().background)
                    .child(
                        Button::new("add-more-files")
                            .primary()
                            .w_full()
                            .h(sizing::TOUCH)
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.open_add_dialog(window, cx);
                            }))
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(6.))
                                    .child(app_icon(
                                        paths::PLUS,
                                        Size::Small,
                                        cx.theme().primary_foreground,
                                    ))
                                    .child("添加"),
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

fn build_close_footer(id_prefix: &str, text: &str) -> DialogFooter {
    DialogFooter::new().child(
        DialogClose::new().child(Button::new(format!("{id_prefix}-close")).label(text.to_string())),
    )
}
