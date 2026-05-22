//! History page: full-screen route showing transfer history.

use crate::state::{
    history_state::{HistoryEntry, HistoryEntryKind, HistoryState},
    receive_inbox_state::{ReceiveInboxState, ReceiveItem, ReceiveSession},
    transfer_state::TransferDirection,
};
use crate::ui::routes;
use crate::ui::theme::spacing;
use chrono::{Datelike, Local, TimeZone as _, Timelike};
use gpui::{div, prelude::*, px, Context, Entity, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex,
    popover::Popover,
    v_flex, ActiveTheme as _, Anchor, Icon, Sizable as _, Size, StyledExt as _, WindowExt as _,
};
use gpui_router::RouterState;

/// History page: back bar + title + history list.
pub struct HistoryPage {
    pub root: Option<Entity<crate::app::AppRoot>>,
    history_state: Option<Entity<HistoryState>>,
    receive_inbox_state: Option<Entity<ReceiveInboxState>>,
    open_menu_entry: Option<String>,
}

impl HistoryPage {
    pub fn new() -> Self {
        Self {
            root: None,
            history_state: None,
            receive_inbox_state: None,
            open_menu_entry: None,
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
        let page_entity = cx.entity();
        let history_state = self.history_state.clone();
        let receive_inbox_state = self.receive_inbox_state.clone();
        let root = self.root.clone();
        let back_button_variant = ButtonCustomVariant::new(cx)
            .hover(cx.theme().transparent)
            .active(cx.theme().transparent);

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            // App bar
            .child(
                h_flex()
                    .w_full()
                    .h(px(56.))
                    .px(px(16.))
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Button::new("history-back")
                                    .ghost()
                                    .custom(back_button_variant)
                                    .h(px(36.))
                                    .w(px(36.))
                                    .p(px(0.))
                                    .rounded_md()
                                    .child(
                                        Icon::default()
                                            .path("icons/arrow-left.svg")
                                            .with_size(Size::Small),
                                    )
                                    .on_click(cx.listener(|this, _event, window, cx| {
                                        if let Some(root) = &this.root {
                                            let _ = root.update(cx, |this, cx| {
                                                this.go_back_or_navigate(routes::HOME, cx);
                                            });
                                        } else {
                                            // Fallback if no root
                                            if let Some(entry) =
                                                crate::ui::router_history::RouterHistoryState::global_mut(cx)
                                                    .history
                                                    .go_back()
                                            {
                                                RouterState::global_mut(cx).location.pathname = entry.pathname;
                                            } else {
                                                RouterState::global_mut(cx).location.pathname =
                                                    routes::HOME.into();
                                            }
                                        }
                                        window.refresh();
                                    })),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child("历史"),
                            ),
                    ),
            )
            // Content
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
                            .px(px(15.))
                            .pt(px(8.))
                            .pb(px(10.))
                            .gap(px(8.))
                            .child(
                                h_flex()
                                    .gap(px(10.))
                                    .items_center()
                                    .child(
                                        Button::new("history-open-folder")
                                            .outline()
                                            .rounded_md()
                                            .h(px(36.))
                                            .px(px(12.))
                                            .on_click(cx.listener(|this, _event, window, cx| {
                                                this.open_notice_dialog(
                                                    "打开目录功能即将接入。",
                                                    window,
                                                    cx,
                                                );
                                            }))
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child(
                                                        Icon::default()
                                                            .path("icons/folder.svg")
                                                            .with_size(Size::Small),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_medium()
                                                            .child("打开目录"),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        Button::new("history-clear")
                                            .outline()
                                            .rounded_md()
                                            .h(px(36.))
                                            .px(px(12.))
                                            .on_click(cx.listener(|this, _event, window, cx| {
                                                this.open_clear_history_dialog(window, cx);
                                            }))
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child(
                                                        Icon::default()
                                                            .path("icons/trash.svg")
                                                            .with_size(Size::Small),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_medium()
                                                            .child("删除历史"),
                                                    ),
                                            ),
                                    ),
                            )
                            .children(entries.into_iter().map(|entry| {
                                let entry_id = entry.id.clone();
                                let menu_open = self.open_menu_entry.as_ref() == Some(&entry_id);
                                let entry_openable = is_openable_entry(&entry);
                                let file_name = entry.file_name.clone();
                                let subline = format!(
                                    "{} - {} - {}",
                                    format_timestamp(entry.timestamp),
                                    format_file_size(entry.file_size),
                                    entry.device_name
                                );

                                let history_state_for_delete = history_state.clone();
                                let page_for_delete = page_entity.clone();
                                let page_for_open_change = page_entity.clone();
                                let page_for_open_action = page_entity.clone();
                                let page_for_info = page_entity.clone();
                                let entry_for_info = entry.clone();
                                let entry_for_open = entry.clone();
                                let entry_for_row_open = entry.clone();
                                let receive_inbox_for_row_open = receive_inbox_state.clone();
                                let receive_inbox_for_menu_open = receive_inbox_state.clone();
                                let root_for_row_open = root.clone();
                                let root_for_menu_open = root.clone();
                                let entry_id_open_change = entry_id.clone();
                                let entry_id_for_open = entry_id.clone();
                                let entry_id_for_delete = entry_id.clone();
                                let row_id = format!("history-row-{}", entry.id);

                                h_flex()
                                    .id(row_id)
                                    .w_full()
                                    .items_center()
                                    .gap(px(12.))
                                    .when(entry_openable, |this| {
                                        this.cursor_pointer().on_click({
                                            let entry_for_row_open = entry_for_row_open.clone();
                                            let receive_inbox_for_row_open =
                                                receive_inbox_for_row_open.clone();
                                            move |_event, window, cx| {
                                                open_history_entry(
                                                    &entry_for_row_open,
                                                    receive_inbox_for_row_open.as_ref(),
                                                    root_for_row_open.as_ref(),
                                                    window,
                                                    cx,
                                                );
                                            }
                                        })
                                    })
                                    .child(
                                        div()
                                            .w(px(44.))
                                            .h(px(44.))
                                            .rounded_md()
                                            .bg(cx.theme().secondary)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                Icon::default()
                                                    .path(icon_for_direction(entry.direction))
                                                    .with_size(Size::Medium)
                                                    .text_color(cx.theme().primary),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .gap(px(4.))
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .w_full()
                                                    .text_lg()
                                                    .font_medium()
                                                    .text_color(cx.theme().foreground)
                                                    .truncate()
                                                    .child(file_name),
                                            )
                                            .child(
                                                div()
                                                    .w_full()
                                                    .text_base()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .truncate()
                                                    .child(subline),
                                            ),
                                    )
                                    .child(
                                        div().flex_none().child(
                                            Popover::new(format!("history-menu-{}", entry_id))
                                                .anchor(Anchor::BottomRight)
                                                .open(menu_open)
                                                .on_open_change(move |open, _window, cx| {
                                                    page_for_open_change.update(cx, |this, _cx| {
                                                        if *open {
                                                            this.open_menu_entry =
                                                                Some(entry_id_open_change.clone());
                                                        } else if this.open_menu_entry.as_ref()
                                                            == Some(&entry_id_open_change)
                                                        {
                                                            this.open_menu_entry = None;
                                                        }
                                                    });
                                                })
                                                .trigger(
                                                    Button::new(format!("history-more-{}", entry_id))
                                                        .ghost()
                                                        .rounded_full()
                                                        .p(px(8.))
                                                        .child(
                                                            Icon::default()
                                                                .path("icons/more-horizontal.svg")
                                                                .with_size(Size::Large),
                                                        ),
                                                )
                                                .content(move |_state, _window, _cx| {
                                                    v_flex()
                                                        .w(px(184.))
                                                        .p(px(4.))
                                                        .gap(px(1.))
                                                        .child(
                                                            Button::new(format!(
                                                                "history-entry-info-{}",
                                                                entry_id_for_open
                                                            ))
                                                            .ghost()
                                                            .w_full()
                                                            .h(px(30.))
                                                            .px(px(8.))
                                                            .justify_start()
                                                            .on_click({
                                                                let page_for_info =
                                                                    page_for_info.clone();
                                                                let entry_for_info =
                                                                    entry_for_info.clone();
                                                                move |_event, window, cx| {
                                                                    page_for_info.update(
                                                                        cx,
                                                                        |this, _cx| {
                                                                            this.open_menu_entry = None;
                                                                        },
                                                                    );
                                                                    open_entry_info_dialog(
                                                                        &entry_for_info,
                                                                        window,
                                                                        cx,
                                                                    );
                                                                }
                                                            })
                                                            .child(div().text_sm().child("信息")),
                                                        )
                                                        .child(
                                                            Button::new(format!(
                                                                "history-entry-delete-{}",
                                                                entry_id_for_delete
                                                            ))
                                                            .ghost()
                                                            .w_full()
                                                            .h(px(30.))
                                                            .px(px(8.))
                                                            .justify_start()
                                                            .on_click({
                                                                let history_state_for_delete =
                                                                    history_state_for_delete.clone();
                                                                let page_for_delete =
                                                                    page_for_delete.clone();
                                                                let entry_id_for_delete =
                                                                    entry_id_for_delete.clone();
                                                                move |_event, _window, cx| {
                                                                    if let Some(ref state) =
                                                                        history_state_for_delete
                                                                    {
                                                                        state.update(cx, |s, _cx| {
                                                                            s.remove_entry(
                                                                                &entry_id_for_delete,
                                                                            );
                                                                        });
                                                                    }
                                                                    page_for_delete.update(
                                                                        cx,
                                                                        |this, _cx| {
                                                                            this.open_menu_entry = None;
                                                                        },
                                                                    );
                                                                }
                                                            })
                                                            .child(
                                                                div().text_sm().child("从历史记录中删除"),
                                                            ),
                                                        )
                                                        .when(entry_openable, |this| {
                                                                let root_for_menu_open =
                                                                    root_for_menu_open.clone();
                                                                this.child(
                                                                    Button::new(format!(
                                                                        "history-entry-open-{}",
                                                                        entry_id_for_open
                                                                    ))
                                                                    .ghost()
                                                                    .w_full()
                                                                    .h(px(30.))
                                                                    .px(px(8.))
                                                                    .justify_start()
                                                                    .on_click({
                                                                        let page_for_open_action =
                                                                            page_for_open_action
                                                                                .clone();
                                                                        let entry_for_open =
                                                                            entry_for_open.clone();
                                                                        let receive_inbox_for_menu_open =
                                                                            receive_inbox_for_menu_open
                                                                                .clone();
                                                                        let root_for_menu_open =
                                                                            root_for_menu_open
                                                                                .clone();
                                                                        move |_event, window, cx| {
                                                                            page_for_open_action.update(
                                                                            cx,
                                                                            |this, _cx| {
                                                                                this.open_menu_entry =
                                                                                    None;
                                                                            },
                                                                        );
                                                                            open_history_entry(
                                                                                &entry_for_open,
                                                                                receive_inbox_for_menu_open.as_ref(),
                                                                                root_for_menu_open.as_ref(),
                                                                                window,
                                                                                cx,
                                                                            );
                                                                        }
                                                                    })
                                                                    .child(div().text_sm().child("打开")),
                                                                )
                                                            })
                                                }),
                                        ),
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
                            .child(
                                v_flex().items_center().gap(spacing::MD).child(
                                    div()
                                        .text_xl()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("无历史记录"),
                                ),
                            )
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
                .title("删除历史")
                .overlay(true)
                .w(px(340.))
                .child(div().w_full().text_sm().child("确定删除所有历史记录吗？"))
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("删除")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(build_confirm_dialog_footer("history-clear", "删除", "取消"))
                .on_ok(move |_event, _window, cx| {
                    if let Some(ref state) = history_state {
                        state.update(cx, |s, _cx| s.clear());
                    }
                    true
                })
        });
    }
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
        inbox.update(cx, move |state, _| {
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

    let open_result = if let Some(uri) = entry.file_uri.as_ref().filter(|u| !u.trim().is_empty()) {
        crate::platform::file_opener::open_saved_uri(uri)
    } else if entry.file_path.exists() {
        crate::platform::file_opener::open_saved_file(&entry.file_path)
    } else {
        open_notice_dialog("文件不存在或已被移动。", window, cx);
        return;
    };
    if let Err(err) = open_result {
        log::warn!("failed to open file from history: {}", err);
        open_notice_dialog("系统打开文件失败。", window, cx);
    }
}

fn icon_for_direction(direction: TransferDirection) -> &'static str {
    match direction {
        TransferDirection::Send => "icons/upload.svg",
        TransferDirection::Receive => "icons/download.svg",
    }
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

fn open_entry_info_dialog(
    entry: &crate::state::history_state::HistoryEntry,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let title = entry.file_name.clone();
    let file_path = entry.file_path.display().to_string();
    let file_uri = entry.file_uri.clone().unwrap_or_default();
    let file_size = format_file_size(entry.file_size);
    let timestamp = format_timestamp(entry.timestamp);
    let sender = entry.device_name.clone();

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title("信息")
            .overlay(true)
            .w(px(360.))
            .child(
                v_flex()
                    .w_full()
                    .gap(px(8.))
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .truncate()
                            .text_sm()
                            .child(format!("名称: {}", title)),
                    )
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .truncate()
                            .text_sm()
                            .child(format!("大小: {}", file_size)),
                    )
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .truncate()
                            .text_sm()
                            .child(format!("来源: {}", sender)),
                    )
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .truncate()
                            .text_sm()
                            .child(format!("时间: {}", timestamp)),
                    )
                    .when(!file_uri.is_empty(), |this| {
                        this.child(
                            div()
                                .w_full()
                                .overflow_hidden()
                                .truncate()
                                .text_sm()
                                .child(format!("URI: {}", file_uri)),
                        )
                    })
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .truncate()
                            .text_sm()
                            .child(format!("路径: {}", file_path)),
                    ),
            )
            .footer(build_alert_dialog_footer("history-info", "关闭"))
            .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("关闭"))
    });
}

fn open_notice_dialog(message: &str, window: &mut Window, cx: &mut gpui::App) {
    let msg = message.to_string();
    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title("提示")
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
