//! History page: full-screen route showing transfer history.

use crate::state::{
    history_state::{HistoryEntry, HistoryEntryKind, HistoryState},
    receive_inbox_state::{ReceiveInboxState, ReceiveItem, ReceiveSession},
    transfer_state::{TransferDirection, TransferStatus},
};
use crate::ui::components::chrome::{
    back_icon_button, dialog_title, empty_state, header_icon_button, page_header, section_title,
};
use crate::ui::icons::{app_icon, paths};
use crate::ui::routes;
use crate::ui::theme::{radius, spacing};
use chrono::{Datelike, Local, TimeZone as _, Timelike};
use gpui::{div, prelude::*, px, AnyElement, App, Context, Entity, Hsla, SharedString, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex, v_flex, ActiveTheme as _, Size, StyledExt as _, WindowExt as _,
};
use gpui_router::RouterState;

/// History page: back bar + title + history list.
pub struct HistoryPage {
    pub root: Option<Entity<crate::app::AppRoot>>,
    history_state: Option<Entity<HistoryState>>,
    receive_inbox_state: Option<Entity<ReceiveInboxState>>,
}

impl HistoryPage {
    pub fn new() -> Self {
        Self {
            root: None,
            history_state: None,
            receive_inbox_state: None,
        }
    }

    pub fn with_root(mut self, root: Entity<crate::app::AppRoot>) -> Self {
        self.root = Some(root);
        self
    }

    pub fn with_history_state(mut self, state: Entity<HistoryState>) -> Self {
        self.history_state = Some(state);
        self
    }

    pub fn with_receive_inbox_state(mut self, state: Entity<ReceiveInboxState>) -> Self {
        self.receive_inbox_state = Some(state);
        self
    }
}

impl gpui::Render for HistoryPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let entries = if let Some(ref state) = self.history_state {
            state.read(cx).entries().to_vec()
        } else {
            vec![]
        };
        let has_entries = !entries.is_empty();
        let groups = group_entries_by_date(entries);
        let receive_inbox_state = self.receive_inbox_state.clone();
        let root = self.root.clone();

        let mut trailing = h_flex().items_center();
        trailing = trailing.child(header_icon_button(
            "history-open-folder",
            paths::FOLDER,
            cx,
            |this, window, cx| {
                this.open_notice_dialog("打开目录功能即将接入。", window, cx);
            },
        ));
        if has_entries {
            trailing = trailing.child(header_icon_button(
                "history-clear",
                paths::TRASH,
                cx,
                |this, window, cx| {
                    this.open_clear_history_dialog(window, cx);
                },
            ));
        }

        v_flex()
            .size_full()
            .child(page_header(
                "历史",
                back_icon_button("history-back", cx, |this, window, cx| {
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
                trailing,
                cx,
            ))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_y_scrollbar()
                    .child(if has_entries {
                        v_flex()
                            .w_full()
                            .max_w(px(960.))
                            .mx_auto()
                            .px(spacing::PAGE)
                            .pt(px(12.))
                            .pb(px(12.))
                            .gap(px(16.))
                            .children(groups.into_iter().map(|(label, items)| {
                                render_date_group(
                                    label,
                                    items,
                                    receive_inbox_state.clone(),
                                    root.clone(),
                                    cx,
                                )
                            }))
                            .into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .py(px(80.))
                            .child(empty_state(
                                paths::HISTORY,
                                "无历史记录",
                                "发送或接收文件后会出现在这里",
                                cx,
                            ))
                            .into_any_element()
                    }),
            )
    }
}

impl HistoryPage {
    fn open_notice_dialog(&self, message: &str, window: &mut Window, cx: &mut Context<Self>) {
        open_notice_dialog(message, window, cx);
    }

    fn open_clear_history_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let history_state = self.history_state.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let history_state = history_state.clone();
            dialog
                .title(dialog_title("删除历史"))
                .overlay(true)
                .w(px(340.))
                .child(
                    div()
                        .w_full()
                        .text_sm()
                        .text_color(_cx.theme().muted_foreground)
                        .child("确定删除全部历史记录吗？仅移除列表，不会删除已保存的文件。"),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("删除")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(build_confirm_dialog_footer("history-clear", "删除", "取消"))
                .on_ok(move |_event, _window, cx| {
                    if let Some(ref state) = history_state {
                        state.update(cx, |s, state_cx| {
                            s.clear();
                            state_cx.notify();
                        });
                    }
                    true
                })
        });
    }

    fn open_delete_entry_dialog(
        &self,
        entry_id: String,
        file_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let history_state = self.history_state.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let history_state = history_state.clone();
            let entry_id = entry_id.clone();
            dialog
                .title(dialog_title("删除记录"))
                .overlay(true)
                .w(px(340.))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(6.))
                        .child(
                            div()
                                .w_full()
                                .text_sm()
                                .child(format!("从历史记录中删除「{file_name}」？")),
                        )
                        .child(
                            div()
                                .w_full()
                                .text_sm()
                                .text_color(_cx.theme().muted_foreground)
                                .child("不会删除设备上已保存的文件。"),
                        ),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("删除")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(build_confirm_dialog_footer(
                    "history-entry-delete",
                    "删除",
                    "取消",
                ))
                .on_ok(move |_event, _window, cx| {
                    if let Some(ref state) = history_state {
                        state.update(cx, |s, state_cx| {
                            s.remove_entry(&entry_id);
                            state_cx.notify();
                        });
                    }
                    true
                })
        });
    }

    fn open_entry_actions_dialog(
        &self,
        entry: &HistoryEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = cx.entity();
        let entry = entry.clone();
        let receive_inbox_state = self.receive_inbox_state.clone();
        let root = self.root.clone();
        let openable = is_openable_entry(&entry);
        let title = entry.file_name.clone();

        window.open_dialog(cx, move |dialog, _window, cx| {
            let fg = cx.theme().foreground;
            let danger = cx.theme().danger;
            let border = cx.theme().border.opacity(0.7);

            let mut actions = v_flex().w_full();
            let mut added = false;

            if openable {
                let entry_open = entry.clone();
                let inbox_open = receive_inbox_state.clone();
                let root_open = root.clone();
                actions = actions.child(action_sheet_row(
                    format!("history-sheet-open-{}", entry.id),
                    paths::EXTERNAL_LINK,
                    "打开",
                    fg,
                    cx,
                    move |_event, window, cx| {
                        window.close_dialog(cx);
                        open_history_entry(
                            &entry_open,
                            inbox_open.as_ref(),
                            root_open.as_ref(),
                            window,
                            cx,
                        );
                    },
                ));
                added = true;
            }

            {
                let entry_info = entry.clone();
                if added {
                    actions = actions.child(sheet_divider(border));
                }
                actions = actions.child(action_sheet_row(
                    format!("history-sheet-info-{}", entry.id),
                    paths::INFO,
                    "查看详情",
                    fg,
                    cx,
                    move |_event, window, cx| {
                        window.close_dialog(cx);
                        open_entry_info_dialog(&entry_info, window, cx);
                    },
                ));
                added = true;
            }

            {
                let page_delete = page.clone();
                let entry_id = entry.id.clone();
                let file_name = entry.file_name.clone();
                if added {
                    actions = actions.child(sheet_divider(border));
                }
                actions = actions.child(action_sheet_row(
                    format!("history-sheet-delete-{}", entry.id),
                    paths::TRASH,
                    "从历史中删除",
                    danger,
                    cx,
                    move |_event, window, cx| {
                        window.close_dialog(cx);
                        page_delete.update(cx, |this, cx| {
                            this.open_delete_entry_dialog(
                                entry_id.clone(),
                                file_name.clone(),
                                window,
                                cx,
                            );
                        });
                    },
                ));
            }

            dialog
                .title(dialog_title(title.clone()))
                .overlay(true)
                .w(px(340.))
                .child(actions)
                .footer(build_alert_dialog_footer("history-sheet", "取消"))
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("取消"))
        });
    }
}

fn render_date_group(
    label: String,
    items: Vec<HistoryEntry>,
    receive_inbox_state: Option<Entity<ReceiveInboxState>>,
    root: Option<Entity<crate::app::AppRoot>>,
    cx: &mut Context<HistoryPage>,
) -> AnyElement {
    let mut group = v_flex()
        .w_full()
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border.opacity(0.75))
        .rounded(radius::LG)
        .px(px(10.));

    for (index, entry) in items.into_iter().enumerate() {
        if index > 0 {
            group = group.child(
                div()
                    .h(px(1.))
                    .ml(px(52.))
                    .bg(cx.theme().border.opacity(0.7)),
            );
        }
        group = group.child(render_history_entry(
            entry,
            receive_inbox_state.clone(),
            root.clone(),
            cx,
        ));
    }

    v_flex()
        .w_full()
        .gap(px(8.))
        .child(section_title(label, cx))
        .child(group)
        .into_any_element()
}

fn render_history_entry(
    entry: HistoryEntry,
    receive_inbox_state: Option<Entity<ReceiveInboxState>>,
    root: Option<Entity<crate::app::AppRoot>>,
    cx: &mut Context<HistoryPage>,
) -> AnyElement {
    let entry_openable = is_openable_entry(&entry);
    let file_name = entry.file_name.clone();
    let time_label = format_time_of_day(entry.timestamp);
    let subline = format_entry_subline(&entry);
    let icon_color = status_icon_color(entry.status, cx);
    let row_id = format!("history-row-{}", entry.id);
    let more_id = format!("history-more-{}", entry.id);
    let entry_for_row = entry.clone();
    let inbox_for_row = receive_inbox_state.clone();
    let root_for_row = root.clone();
    let entry_for_more = entry.clone();

    h_flex()
        .w_full()
        .items_center()
        .gap(px(2.))
        .child(
            h_flex()
                .id(row_id)
                .flex_1()
                .min_w(px(0.))
                .items_center()
                .gap(px(12.))
                .min_h(px(56.))
                .py(px(8.))
                .cursor_pointer()
                .on_click(move |_event, window, cx| {
                    if is_openable_entry(&entry_for_row) {
                        open_history_entry(
                            &entry_for_row,
                            inbox_for_row.as_ref(),
                            root_for_row.as_ref(),
                            window,
                            cx,
                        );
                    } else {
                        open_entry_info_dialog(&entry_for_row, window, cx);
                    }
                })
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
                        .child(app_icon(icon_for_entry(&entry), Size::Small, icon_color)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.))
                        .gap(px(2.))
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(cx.theme().foreground)
                                        .truncate()
                                        .child(file_name),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(time_label),
                                ),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .truncate()
                                        .child(subline),
                                )
                                .children(status_badge(entry.status, cx)),
                        )
                        .when(
                            entry.kind == HistoryEntryKind::File && !entry_openable,
                            |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground.opacity(0.9))
                                        .child("文件已不可用"),
                                )
                            },
                        ),
                ),
        )
        .child(
            div()
                .id(more_id)
                .w(px(36.))
                .h(px(36.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .flex_none()
                .cursor_pointer()
                .on_click(cx.listener(move |this, _event, window, cx| {
                    this.open_entry_actions_dialog(&entry_for_more, window, cx);
                }))
                .child(app_icon(
                    paths::MORE,
                    Size::Small,
                    cx.theme().muted_foreground,
                )),
        )
        .into_any_element()
}

fn action_sheet_row(
    id: impl Into<SharedString>,
    icon: &'static str,
    label: &'static str,
    color: Hsla,
    _cx: &App,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    h_flex()
        .id(id.into())
        .w_full()
        .min_h(px(48.))
        .px(px(4.))
        .gap(px(12.))
        .items_center()
        .cursor_pointer()
        .on_click(on_click)
        .child(app_icon(icon, Size::Small, color))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_sm()
                .font_medium()
                .text_color(color)
                .child(label),
        )
        .into_any_element()
}

fn sheet_divider(color: Hsla) -> AnyElement {
    div().h(px(1.)).bg(color).into_any_element()
}

fn info_row(label: &str, value: impl Into<SharedString>, cx: &App) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .gap(px(12.))
        .child(
            div()
                .w(px(44.))
                .flex_none()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_sm()
                .text_color(cx.theme().foreground)
                .whitespace_normal()
                .child(value.into()),
        )
        .into_any_element()
}

fn is_openable_entry(entry: &HistoryEntry) -> bool {
    match entry.kind {
        HistoryEntryKind::Text => entry
            .text_content
            .as_ref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or_else(|| !entry.file_name.trim().is_empty()),
        HistoryEntryKind::File => {
            entry
                .file_uri
                .as_ref()
                .map(|u| !u.trim().is_empty())
                .unwrap_or(false)
                || entry.file_path.exists()
        }
    }
}

fn resolve_text_content(entry: &HistoryEntry) -> Option<String> {
    if let Some(text) = entry.text_content.as_ref() {
        if !text.trim().is_empty() {
            return Some(text.clone());
        }
    }
    if !entry.file_name.trim().is_empty() {
        return Some(entry.file_name.clone());
    }
    None
}

fn open_history_entry(
    entry: &HistoryEntry,
    receive_inbox_state: Option<&Entity<ReceiveInboxState>>,
    root: Option<&Entity<crate::app::AppRoot>>,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    if entry.kind == HistoryEntryKind::Text {
        let Some(content) = resolve_text_content(entry) else {
            open_notice_dialog("该文本历史缺少内容，无法打开。", window, cx);
            return;
        };
        let Some(inbox) = receive_inbox_state else {
            open_notice_dialog("文本查看页状态未初始化。", window, cx);
            return;
        };

        let sender_alias = entry.device_name.clone();
        let session_id = format!("history-text-{}", entry.id);
        let item = ReceiveItem {
            file_id: format!("history-item-{}", entry.id),
            file_name: entry.file_name.clone(),
            file_type: "text/plain".to_string(),
            size: entry.file_size,
            bytes_received: entry.file_size,
            speed_bytes_per_sec: 0,
            saved_path: None,
            saved_uri: None,
            text_content: Some(content),
        };
        inbox.update(cx, move |state, state_cx| {
            state.active = Some(ReceiveSession {
                session_id,
                sender_alias,
                sender_device_model: Some("NearSend".to_string()),
                sender_fingerprint: "history".to_string(),
                direction: entry.direction,
                items: vec![item],
                completed: true,
                cancelled: false,
                decision_submitted: true,
                is_message_only: true,
                selected_file_ids: Vec::new(),
            });
            state_cx.notify();
        });
        if let Some(root) = root {
            let _ = root.update(cx, |root, cx| {
                root.navigate_to(routes::RECEIVE_INCOMING, cx);
            });
        } else {
            RouterState::global_mut(cx).location.pathname = routes::RECEIVE_INCOMING.into();
        }
        window.refresh();
        return;
    }

    #[cfg(target_env = "ohos")]
    {
        enum OpenTarget {
            Uri(String),
            Path(std::path::PathBuf),
        }

        let target = if let Some(uri) = entry.file_uri.as_ref().filter(|u| !u.trim().is_empty()) {
            OpenTarget::Uri(uri.clone())
        } else if entry.file_path.exists() {
            OpenTarget::Path(entry.file_path.clone())
        } else {
            open_notice_dialog("文件不存在或已被移动。", window, cx);
            return;
        };
        let window_handle = window.window_handle();
        cx.spawn(async move |cx| {
            let open_result = match target {
                OpenTarget::Uri(uri) => crate::platform::file_opener::open_saved_uri(&uri).await,
                OpenTarget::Path(path) => {
                    crate::platform::file_opener::open_saved_file(&path).await
                }
            };
            if let Err(error) = open_result {
                log::warn!("failed to open file from history: {error}");
                let _ = window_handle.update(cx, |_, window, cx| {
                    open_notice_dialog("系统打开文件失败。", window, cx);
                });
            }
        })
        .detach();
    }

    #[cfg(not(target_env = "ohos"))]
    {
        let open_result =
            if let Some(uri) = entry.file_uri.as_ref().filter(|u| !u.trim().is_empty()) {
                crate::platform::file_opener::open_saved_uri(uri)
            } else if entry.file_path.exists() {
                crate::platform::file_opener::open_saved_file(&entry.file_path)
            } else {
                open_notice_dialog("文件不存在或已被移动。", window, cx);
                return;
            };
        if let Err(err) = open_result {
            log::warn!("failed to open file from history: {err}");
            open_notice_dialog("系统打开文件失败。", window, cx);
        }
    }
}

fn icon_for_entry(entry: &HistoryEntry) -> &'static str {
    match entry.kind {
        HistoryEntryKind::Text => paths::BOOK_OPEN,
        HistoryEntryKind::File => match entry.direction {
            TransferDirection::Send => paths::UPLOAD,
            TransferDirection::Receive => paths::DOWNLOAD,
        },
    }
}

fn status_icon_color(status: TransferStatus, cx: &App) -> Hsla {
    match status {
        TransferStatus::Failed => cx.theme().danger,
        TransferStatus::Cancelled | TransferStatus::Skipped => cx.theme().muted_foreground,
        _ => cx.theme().primary,
    }
}

fn status_badge(status: TransferStatus, cx: &App) -> Option<AnyElement> {
    if status == TransferStatus::Completed {
        return None;
    }
    let (label, color) = match status {
        TransferStatus::Pending => ("等待中", cx.theme().muted_foreground),
        TransferStatus::InProgress => ("传输中", cx.theme().primary),
        TransferStatus::Completed => return None,
        TransferStatus::Failed => ("失败", cx.theme().danger),
        TransferStatus::Cancelled => ("已取消", cx.theme().muted_foreground),
        TransferStatus::Skipped => ("已跳过", cx.theme().muted_foreground),
    };
    Some(
        div()
            .flex_none()
            .px(px(6.))
            .py(px(1.))
            .rounded(radius::FULL)
            .bg(color.opacity(0.14))
            .child(div().text_xs().font_medium().text_color(color).child(label))
            .into_any_element(),
    )
}

fn format_direction(direction: TransferDirection) -> &'static str {
    match direction {
        TransferDirection::Send => "发送",
        TransferDirection::Receive => "接收",
    }
}

fn format_kind(kind: HistoryEntryKind) -> &'static str {
    match kind {
        HistoryEntryKind::File => "文件",
        HistoryEntryKind::Text => "文本",
    }
}

fn format_status(status: TransferStatus) -> &'static str {
    match status {
        TransferStatus::Pending => "等待中",
        TransferStatus::InProgress => "传输中",
        TransferStatus::Completed => "已完成",
        TransferStatus::Failed => "失败",
        TransferStatus::Cancelled => "已取消",
        TransferStatus::Skipped => "已跳过",
    }
}

fn format_entry_subline(entry: &HistoryEntry) -> String {
    let mut parts = vec![format_direction(entry.direction).to_string()];
    match entry.kind {
        HistoryEntryKind::Text => parts.push("文本".to_string()),
        HistoryEntryKind::File => parts.push(format_file_size(entry.file_size)),
    }
    if !entry.device_name.trim().is_empty() {
        parts.push(entry.device_name.clone());
    }
    parts.join(" · ")
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

fn format_timestamp(timestamp: u64) -> String {
    if let Some(dt) = Local.timestamp_opt(timestamp as i64, 0).single() {
        format!(
            "{}/{}/{} {:02}:{:02}",
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute()
        )
    } else {
        "-".to_string()
    }
}

fn format_time_of_day(timestamp: u64) -> String {
    if let Some(dt) = Local.timestamp_opt(timestamp as i64, 0).single() {
        format!("{:02}:{:02}", dt.hour(), dt.minute())
    } else {
        "-".to_string()
    }
}

fn date_group_label(timestamp: u64) -> String {
    let Some(dt) = Local.timestamp_opt(timestamp as i64, 0).single() else {
        return "更早".to_string();
    };
    let today = Local::now().date_naive();
    let date = dt.date_naive();
    if date == today {
        "今天".to_string()
    } else if Some(date) == today.pred_opt() {
        "昨天".to_string()
    } else if date.year() == today.year() {
        format!("{}月{}日", date.month(), date.day())
    } else {
        format!("{}年{}月{}日", date.year(), date.month(), date.day())
    }
}

fn group_entries_by_date(entries: Vec<HistoryEntry>) -> Vec<(String, Vec<HistoryEntry>)> {
    let mut groups: Vec<(String, Vec<HistoryEntry>)> = Vec::new();
    for entry in entries {
        let label = date_group_label(entry.timestamp);
        match groups.last_mut() {
            Some((last, items)) if *last == label => items.push(entry),
            _ => groups.push((label, vec![entry])),
        }
    }
    groups
}

fn open_entry_info_dialog(entry: &HistoryEntry, window: &mut Window, cx: &mut gpui::App) {
    let title = entry.file_name.clone();
    let kind = format_kind(entry.kind).to_string();
    let direction = format_direction(entry.direction).to_string();
    let status = format_status(entry.status).to_string();
    let device = entry.device_name.clone();
    let file_size = format_file_size(entry.file_size);
    let timestamp = format_timestamp(entry.timestamp);
    let file_path = entry.file_path.display().to_string();
    let file_uri = entry.file_uri.clone().unwrap_or_default();
    let text_preview = entry.text_content.clone().and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else if trimmed.chars().count() > 160 {
            Some(trimmed.chars().take(160).collect::<String>() + "…")
        } else {
            Some(trimmed.to_string())
        }
    });
    let show_path = entry.kind == HistoryEntryKind::File && !file_path.trim().is_empty();

    window.open_dialog(cx, move |dialog, _window, cx| {
        let mut rows = v_flex().w_full().gap(px(10.));
        rows = rows
            .child(info_row("名称", title.clone(), cx))
            .child(info_row("类型", kind.clone(), cx))
            .child(info_row("方向", direction.clone(), cx))
            .child(info_row("状态", status.clone(), cx));
        if !device.trim().is_empty() {
            rows = rows.child(info_row("设备", device.clone(), cx));
        }
        rows = rows
            .child(info_row("大小", file_size.clone(), cx))
            .child(info_row("时间", timestamp.clone(), cx));
        if let Some(preview) = text_preview.clone() {
            rows = rows.child(info_row("内容", preview, cx));
        }
        if !file_uri.is_empty() {
            rows = rows.child(info_row("URI", file_uri.clone(), cx));
        }
        if show_path {
            rows = rows.child(info_row("路径", file_path.clone(), cx));
        }

        dialog
            .title(dialog_title("详情"))
            .overlay(true)
            .w(px(360.))
            .child(rows)
            .footer(build_alert_dialog_footer("history-info", "关闭"))
            .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("关闭"))
    });
}

fn open_notice_dialog(message: &str, window: &mut Window, cx: &mut gpui::App) {
    let msg = message.to_string();
    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(dialog_title("提示"))
            .overlay(true)
            .w(px(320.))
            .child(div().w_full().text_sm().child(msg.clone()))
            .footer(build_alert_dialog_footer("history-notice", "确定"))
            .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("确定"))
    });
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

impl Default for HistoryPage {
    fn default() -> Self {
        Self::new()
    }
}
