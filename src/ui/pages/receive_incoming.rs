use crate::state::{
    app_state::AppState, receive_inbox_state::ReceiveInboxState, transfer_state::TransferDirection,
};
use crate::ui::icons::{app_icon, paths};
use crate::ui::routes;
use crate::ui::theme::{radius, spacing};
use gpui::{div, hsla, prelude::*, px, Context, Entity, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    notification::Notification,
    progress::Progress,
    v_flex, ActiveTheme as _, Size, StyledExt as _, WindowExt as _,
};
use gpui_router::RouterState;
use std::collections::HashSet;
use std::time::Duration;

pub struct ReceiveIncomingPage {
    pub root: Option<Entity<crate::app::AppRoot>>,
    app_state: Entity<AppState>,
    inbox_state: Entity<ReceiveInboxState>,
}

impl ReceiveIncomingPage {
    pub fn new(
        root: Entity<crate::app::AppRoot>,
        app_state: Entity<AppState>,
        inbox_state: Entity<ReceiveInboxState>,
    ) -> Self {
        Self {
            root: Some(root),
            app_state,
            inbox_state,
        }
    }

    fn show_copy_success_toast(&self, window: &mut Window, cx: &mut Context<Self>) {
        struct CopySuccessToast;
        window.push_notification(
            Notification::new()
                .id::<CopySuccessToast>()
                .autohide(false)
                .content(|_, _, _| {
                    div()
                        .w_full()
                        .text_xs()
                        .text_center()
                        .child("复制成功")
                        .into_any_element()
                })
                .w(px(92.))
                .py(px(4.))
                .px(px(10.))
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
            tokio::time::sleep(Duration::from_millis(1500)).await;
        });
        cx.spawn(async move |_this, cx| {
            let _ = dismiss.await;
            let _ = window_handle.update(cx, |_, window, cx| {
                window.remove_notification::<CopySuccessToast>(cx);
            });
        })
        .detach();
    }
}

impl gpui::Render for ReceiveIncomingPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.inbox_state.read(cx).active.clone();
        let sender_alias = session
            .as_ref()
            .map(|s| s.sender_alias.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "NearSend".to_string());
        let sender_model = session
            .as_ref()
            .and_then(|s| s.sender_device_model.clone())
            .unwrap_or_else(|| "OpenHarmony".to_string());
        let sender_tag = session
            .as_ref()
            .map(|s| format!("#{}", visual_tag(&s.sender_fingerprint)))
            .unwrap_or_else(|| "#--".to_string());

        let message_content = session.as_ref().and_then(|s| {
            if s.is_message_only {
                s.items.first().and_then(|item| item.text_content.clone())
            } else {
                None
            }
        });
        let file_count = session.as_ref().map(|s| s.items.len()).unwrap_or(0);
        let direction = session
            .as_ref()
            .map(|s| s.direction)
            .unwrap_or(TransferDirection::Receive);
        let subtitle = if message_content.is_some() {
            if direction == TransferDirection::Send {
                format!("你发送给 {} 的消息：", sender_alias)
            } else {
                "发送给你了一条消息：".to_string()
            }
        } else if file_count > 0 {
            if direction == TransferDirection::Send {
                format!("你发送给 {} {} 个文件", sender_alias, file_count)
            } else {
                format!("发送给你 {} 个文件", file_count)
            }
        } else {
            "等待接收内容".to_string()
        };
        let show_cancelled = session.as_ref().map(|s| s.cancelled).unwrap_or(false);
        let decision_submitted = session
            .as_ref()
            .map(|s| s.decision_submitted)
            .unwrap_or(false);
        let show_waiting_actions = session
            .as_ref()
            .map(|s| !s.decision_submitted && !s.completed && !s.cancelled && !s.is_message_only)
            .unwrap_or(false);
        let show_receiving_actions = session
            .as_ref()
            .map(|s| s.decision_submitted && !s.completed && !s.cancelled && !s.is_message_only)
            .unwrap_or(false);
        let selected_file_ids: HashSet<String> = session
            .as_ref()
            .map(|s| s.selected_file_ids.iter().cloned().collect())
            .unwrap_or_default();
        let active_session_id = session.as_ref().map(|s| s.session_id.clone());
        let is_completed = session.as_ref().map(|s| s.completed).unwrap_or(false);
        let total_size: u64 = session
            .as_ref()
            .map(|s| s.items.iter().map(|item| item.size).sum())
            .unwrap_or(0);
        let received_size: u64 = session
            .as_ref()
            .map(|s| s.items.iter().map(|item| item.bytes_received).sum())
            .unwrap_or(0);
        let selected_size: u64 = session
            .as_ref()
            .map(|s| {
                s.items
                    .iter()
                    .filter(|item| selected_file_ids.contains(&item.file_id))
                    .map(|item| item.size)
                    .sum()
            })
            .unwrap_or(0);
        let selected_count = selected_file_ids.len();
        let overall_progress = if total_size > 0 {
            (received_size as f64 / total_size as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let active_speed = session
            .as_ref()
            .and_then(|s| {
                s.items
                    .iter()
                    .filter(|item| item.bytes_received > 0 && item.bytes_received < item.size)
                    .map(|item| item.speed_bytes_per_sec)
                    .max()
            })
            .unwrap_or(0);
        let is_receiving = !is_completed
            && !show_cancelled
            && message_content.is_none()
            && received_size > 0
            && received_size < total_size.max(1);
        let status_title = if show_cancelled {
            "传输已取消".to_string()
        } else if is_receiving {
            "正在接收".to_string()
        } else if decision_submitted && !is_completed && message_content.is_none() {
            "等待对方发送".to_string()
        } else if is_completed {
            if direction == TransferDirection::Send {
                "发送完成".to_string()
            } else {
                "接收完成".to_string()
            }
        } else if message_content.is_some() {
            "收到一条消息".to_string()
        } else if file_count > 0 {
            "准备接收文件".to_string()
        } else {
            "等待接收内容".to_string()
        };
        let status_caption = if show_cancelled {
            "对方已取消本次传输".to_string()
        } else if is_receiving {
            if active_speed > 0 {
                format!(
                    "{} / {} · {:.0}% · {}/s",
                    format_receive_size(received_size),
                    format_receive_size(total_size),
                    overall_progress * 100.0,
                    format_receive_size(active_speed)
                )
            } else {
                format!(
                    "{} / {} · {:.0}%",
                    format_receive_size(received_size),
                    format_receive_size(total_size),
                    overall_progress * 100.0
                )
            }
        } else if decision_submitted && !is_completed && message_content.is_none() {
            format!("已接受 {} 个文件，正在建立传输", selected_count)
        } else if is_completed {
            if file_count > 0 {
                format!(
                    "已处理 {} 个文件 · {}",
                    file_count,
                    format_receive_size(total_size)
                )
            } else {
                "内容已处理完成".to_string()
            }
        } else if message_content.is_some() {
            format!("来自 {}", sender_alias)
        } else if file_count > 0 {
            let selection_hint = if selected_count == file_count {
                "全部已选".to_string()
            } else {
                format!("已选 {}/{}", selected_count, file_count)
            };
            format!(
                "{} · {}",
                selection_hint,
                format_receive_size(selected_size)
            )
        } else {
            "保持此页面打开以继续接收".to_string()
        };
        let status_icon = if show_cancelled {
            paths::X
        } else if is_completed {
            paths::CHECK
        } else if direction == TransferDirection::Send {
            paths::UPLOAD
        } else {
            paths::DOWNLOAD
        };
        let status_color = if show_cancelled {
            cx.theme().danger
        } else if is_completed {
            cx.theme().success
        } else {
            cx.theme().primary
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(760.))
                            .mx_auto()
                            .px(spacing::PAGE)
                            .pt(px(20.))
                            .pb(px(20.))
                            .items_center()
                            .gap(px(14.))
                            .child(
                                div()
                                    .w(px(64.))
                                    .h(px(64.))
                                    .rounded_full()
                                    .bg(status_color.opacity(0.14))
                                    .border_1()
                                    .border_color(status_color.opacity(0.30))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(app_icon(status_icon, Size::Large, status_color)),
                            )
                            .child(
                                div()
                                    .max_w(px(660.))
                                    .w_full()
                                    .overflow_hidden()
                                    .truncate()
                                    .text_2xl()
                                    .font_semibold()
                                    .text_center()
                                    .text_color(cx.theme().foreground)
                                    .child(sender_alias),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .px(px(9.))
                                            .py(px(3.))
                                            .rounded_full()
                                            .bg(cx.theme().foreground.opacity(0.08))
                                            .child(
                                                div()
                                                    .max_w(px(200.))
                                                    .overflow_hidden()
                                                    .truncate()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .text_color(cx.theme().foreground)
                                                    .child(sender_tag),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px(px(9.))
                                            .py(px(3.))
                                            .rounded_full()
                                            .bg(cx.theme().foreground.opacity(0.08))
                                            .child(
                                                div()
                                                    .max_w(px(260.))
                                                    .overflow_hidden()
                                                    .truncate()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .text_color(cx.theme().foreground)
                                                    .child(sender_model),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .mt(px(8.))
                                    .rounded(radius::LG)
                                    .border_1()
                                    .border_color(cx.theme().border.opacity(0.78))
                                    .bg(cx.theme().background)
                                    .shadow_sm()
                                    .p(px(16.))
                                    .child(
                                        v_flex()
                                            .gap(px(14.))
                                            .child(
                                                h_flex()
                                                    .items_start()
                                                    .gap(px(12.))
                                                    .child(
                                                        div()
                                                            .w(px(42.))
                                                            .h(px(42.))
                                                            .rounded_md()
                                                            .bg(status_color.opacity(0.12))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(
                                                                app_icon(
                                                                    status_icon,
                                                                    Size::Small,
                                                                    status_color,
                                                                ),
                                                            ),
                                                    )
                                                    .child(
                                                        v_flex()
                                                            .flex_1()
                                                            .min_w(px(0.))
                                                            .gap(px(4.))
                                                            .child(
                                                                div()
                                                                    .text_xl()
                                                                    .font_semibold()
                                                                    .text_color(cx.theme().foreground)
                                                                    .child(status_title),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_sm()
                                                                    .line_height(px(20.))
                                                                    .text_color(cx.theme().muted_foreground)
                                                                    .child(status_caption),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_base()
                                                    .line_height(px(22.))
                                                    .text_color(cx.theme().foreground)
                                                    .child(subtitle),
                                            )
                                            .when(is_receiving || is_completed, |this| {
                                                this.child(
                                                    v_flex()
                                                        .gap(px(8.))
                                                        .child(
                                                            h_flex()
                                                                .justify_between()
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(cx.theme().muted_foreground)
                                                                        .child("总进度"),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .font_semibold()
                                                                        .text_color(status_color)
                                                                        .child(format!("{:.0}%", overall_progress * 100.0)),
                                                                ),
                                                        )
                                                        .child(
                                                            Progress::new("receive-overall-progress")
                                                                .value((overall_progress * 100.0) as f32)
                                                                .w_full(),
                                                        ),
                                                )
                                            })
                                            .when_some(message_content.clone(), |this, content| {
                                                this.child(
                                                    div()
                                                        .w_full()
                                                        .min_h(px(118.))
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(cx.theme().border.opacity(0.72))
                                                        .bg(cx.theme().background)
                                                        .p(px(14.))
                                                        .child(
                                                            div()
                                                                .text_base()
                                                                .line_height(px(22.))
                                                                .text_color(cx.theme().foreground)
                                                                .whitespace_normal()
                                                                .child(content),
                                                        ),
                                                )
                                            })
                                            .when(message_content.is_none() && file_count > 0, |this| {
                                                this.child(
                                                    v_flex()
                                                        .gap(px(8.))
                                                        .children(session.clone().into_iter().flat_map(|s| {
                                                            s.items.into_iter().map(|item| {
                                                                let file_id = item.file_id.clone();
                                                                let icon = if item.file_type.starts_with("text/") {
                                                                    paths::BOOK_OPEN
                                                                } else {
                                                                    paths::FILE
                                                                };
                                                                let selected = selected_file_ids.contains(&item.file_id);
                                                                let row_tone = if show_cancelled {
                                                                    cx.theme().danger
                                                                } else if is_completed {
                                                                    cx.theme().success
                                                                } else if selected {
                                                                    cx.theme().primary
                                                                } else {
                                                                    cx.theme().muted_foreground
                                                                };
                                                                let row_active = !show_cancelled && (selected || is_completed);
                                                                let row_progress = if item.size > 0 {
                                                                    (item.bytes_received as f64
                                                                        / item.size as f64)
                                                                        .clamp(0.0, 1.0)
                                                                } else {
                                                                    0.0
                                                                };
                                                                let row_receiving = !show_cancelled
                                                                    && !is_completed
                                                                    && item.bytes_received > 0
                                                                    && item.bytes_received < item.size;
                                                                let row_status = if show_cancelled {
                                                                    "已取消".to_string()
                                                                } else if is_completed || item.saved_path.is_some() || item.saved_uri.is_some() {
                                                                    "已保存".to_string()
                                                                } else if row_receiving {
                                                                    if item.speed_bytes_per_sec > 0 {
                                                                        format!(
                                                                            "{} / {} · {}/s",
                                                                            format_receive_size(item.bytes_received),
                                                                            format_receive_size(item.size),
                                                                            format_receive_size(item.speed_bytes_per_sec)
                                                                        )
                                                                    } else {
                                                                        format!(
                                                                            "{} / {}",
                                                                            format_receive_size(item.bytes_received),
                                                                            format_receive_size(item.size)
                                                                        )
                                                                    }
                                                                } else if selected {
                                                                    format_receive_size(item.size)
                                                                } else {
                                                                    "未选择".to_string()
                                                                };
                                                                div()
                                                                    .id(format!("receive-file-item-{}", item.file_id))
                                                                    .w_full()
                                                                    .rounded_md()
                                                                    .border_1()
                                                                    .border_color(if row_active || show_cancelled {
                                                                        row_tone.opacity(0.26)
                                                                    } else {
                                                                        cx.theme().border.opacity(0.70)
                                                                    })
                                                                    .bg(if row_active || show_cancelled {
                                                                        row_tone.opacity(0.08)
                                                                    } else {
                                                                        cx.theme().background
                                                                    })
                                                                    .p(px(10.))
                                                                    .when(show_waiting_actions, |this| {
                                                                        this.cursor_pointer().on_click(cx.listener(
                                                                            move |this, _e, _window, cx| {
                                                                                this.inbox_state.update(cx, |state, state_cx| {
                                                                                    state.toggle_file_selected(&file_id);
                                                                                    state_cx.notify();
                                                                                });
                                                                            },
                                                                        ))
                                                                    })
                                                                    .child(
                                                                        h_flex()
                                                                            .items_center()
                                                                            .gap(px(10.))
                                                                            .child(
                                                                                div()
                                                                                    .w(px(24.))
                                                                                    .h(px(24.))
                                                                                    .rounded_full()
                                                                                    .border_1()
                                                                                    .border_color(row_tone.opacity(0.45))
                                                                                    .bg(if row_active || show_cancelled {
                                                                                        row_tone.opacity(0.16)
                                                                                    } else {
                                                                                        cx.theme().background
                                                                                    })
                                                                                    .flex()
                                                                                    .items_center()
                                                                                    .justify_center()
                                                                                    .child(
                                                                                        app_icon(
                                                                                            if row_active {
                                                                                                paths::CHECK
                                                                                            } else {
                                                                                                paths::X
                                                                                            },
                                                                                            Size::XSmall,
                                                                                            row_tone,
                                                                                        ),
                                                                                    ),
                                                                            )
                                                                            .child(
                                                                                app_icon(
                                                                                    icon,
                                                                                    Size::Small,
                                                                                    cx.theme().muted_foreground,
                                                                                ),
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
                                                                                            .font_semibold()
                                                                                            .text_color(cx.theme().foreground)
                                                                                            .child(item.file_name),
                                                                                    )
                                                                                    .child(
                                                                                        h_flex()
                                                                                            .items_center()
                                                                                            .gap(px(6.))
                                                                                            .child(
                                                                                                div()
                                                                                                    .text_xs()
                                                                                                    .text_color(if row_receiving || is_completed {
                                                                                                        row_tone
                                                                                                    } else {
                                                                                                        cx.theme().muted_foreground
                                                                                                    })
                                                                                                    .child(row_status),
                                                                                            )
                                                                                    ),
                                                                            ),
                                                                    )
                                                                    .when(row_receiving || is_completed, |this| {
                                                                        this.child(
                                                                            div()
                                                                                .mt(px(8.))
                                                                                .child(
                                                                                    Progress::new(format!(
                                                                                        "receive-file-progress-{}",
                                                                                        item.file_id
                                                                                    ))
                                                                                    .value((row_progress * 100.0) as f32)
                                                                                    .w_full(),
                                                                                ),
                                                                        )
                                                                    })
                                                            })
                                                        })),
                                                )
                                            })
                                            .when_some(message_content.clone(), |this, content| {
                                                this.child(
                                                    Button::new("receive-incoming-copy")
                                                        .primary()
                                                        .h(px(42.))
                                                        .px(px(20.))
                                                        .rounded_md()
                                                        .child("复制消息")
                                                        .on_click(cx.listener(
                                                            move |this, _event, window, cx| {
                                                                if !content.is_empty() {
                                                                    let page = cx.entity();
                                                                    let tokio_handle =
                                                                        this.app_state.read(cx).tokio_handle.clone();
                                                                    let content_to_write = content.clone();
                                                                    let window_handle = window.window_handle();
                                                                    let join = tokio_handle.spawn(async move {
                                                                        crate::platform::clipboard::write_clipboard_text(
                                                                            content_to_write,
                                                                        )
                                                                        .await
                                                                        .unwrap_or(false)
                                                                    });
                                                                    cx.spawn(async move |_this, cx| {
                                                                        let copied = join.await.unwrap_or(false);
                                                                        let _ = window_handle.update(cx, |_, window, cx| {
                                                                            if copied {
                                                                                let _ = page.update(cx, |this, cx| {
                                                                                    this.show_copy_success_toast(window, cx);
                                                                                });
                                                                            }
                                                                        });
                                                                    })
                                                                    .detach();
                                                                }
                                                            },
                                                        )),
                                                )
                                            }),
                                    ),
                            ),
                    ),
            )
            .when(show_waiting_actions, |this| {
                let session_id = active_session_id.clone().unwrap_or_default();
                let session_id_for_decline = session_id.clone();
                let session_id_for_accept = session_id.clone();
                let selected_ids_for_accept = session
                    .as_ref()
                    .map(|s| s.selected_file_ids.clone())
                    .unwrap_or_default();
                this.child(
                    h_flex()
                        .w_full()
                        .px(px(20.))
                        .pt(px(14.))
                        .pb(px(26.))
                        .gap(px(12.))
                        .border_t_1()
                        .border_color(cx.theme().border.opacity(0.72))
                        .bg(cx.theme().background)
                        .child(
                            Button::new("receive-incoming-decline")
                                .outline()
                                .flex_1()
                                .h(px(48.))
                                .rounded_md()
                                .child("拒绝")
                                .on_click(cx.listener(move |this, _e, window, cx| {
                                    crate::core::receive_events::submit_incoming_decision(
                                        session_id_for_decline.clone(),
                                        crate::core::receive_events::IncomingTransferDecision::Decline,
                                    );
                                    this.inbox_state.update(cx, |s, state_cx| {
                                        s.clear();
                                        state_cx.notify();
                                    });
                                    if let Some(root) = &this.root {
                                        let _ = root.update(cx, |this, cx| {
                                            this.go_back_or_navigate(routes::HOME, cx);
                                        });
                                    } else {
                                        if let Some(entry) =
                                            crate::ui::router_history::RouterHistoryState::global_mut(cx)
                                                .history
                                                .go_back()
                                        {
                                            RouterState::global_mut(cx).location.pathname =
                                                entry.pathname;
                                        } else {
                                            RouterState::global_mut(cx).location.pathname =
                                                routes::HOME.into();
                                        }
                                    }
                                    window.refresh();
                                })),
                        )
                        .child(
                            Button::new("receive-incoming-accept")
                                .primary()
                                .flex_1()
                                .h(px(48.))
                                .rounded_md()
                                .child("接受")
                                .on_click(cx.listener(move |this, _e, window, cx| {
                                    crate::core::receive_events::submit_incoming_decision(
                                        session_id_for_accept.clone(),
                                        crate::core::receive_events::IncomingTransferDecision::AcceptSelected(
                                            selected_ids_for_accept.clone(),
                                        ),
                                    );
                                    this.inbox_state
                                        .update(cx, |state, state_cx| {
                                            state.mark_decision_submitted();
                                            state_cx.notify();
                                        });
                                    window.refresh();
                                })),
                        ),
                )
            })
            .when(show_receiving_actions, |this| {
                let session_id_for_cancel = active_session_id.clone().unwrap_or_default();
                this.child(
                    h_flex()
                        .w_full()
                        .justify_center()
                        .px(px(20.))
                        .pt(px(14.))
                        .pb(px(26.))
                        .border_t_1()
                        .border_color(cx.theme().border.opacity(0.72))
                        .bg(cx.theme().background)
                        .child(
                            Button::new("receive-incoming-cancel")
                                .outline()
                                .h(px(48.))
                                .px(px(42.))
                                .rounded_md()
                                .child("取消接收")
                                .on_click(cx.listener(move |this, _e, window, cx| {
                                    crate::core::receive_events::request_incoming_cancel(
                                        session_id_for_cancel.clone(),
                                    );
                                    this.inbox_state.update(cx, |state, state_cx| {
                                        state.clear();
                                        state_cx.notify();
                                    });
                                    if let Some(root) = &this.root {
                                        let _ = root.update(cx, |this, cx| {
                                            this.go_back_or_navigate(routes::HOME, cx);
                                        });
                                    } else if let Some(entry) =
                                        crate::ui::router_history::RouterHistoryState::global_mut(cx)
                                            .history
                                            .go_back()
                                    {
                                        RouterState::global_mut(cx).location.pathname =
                                            entry.pathname;
                                    } else {
                                        RouterState::global_mut(cx).location.pathname =
                                            routes::HOME.into();
                                    }
                                    window.refresh();
                                })),
                        ),
                )
            })
            .when(!show_waiting_actions && !show_receiving_actions, |this| {
                this.child(
                    h_flex()
                        .w_full()
                        .justify_center()
                        .px(px(20.))
                        .pt(px(14.))
                        .pb(px(26.))
                        .border_t_1()
                        .border_color(cx.theme().border.opacity(0.72))
                        .bg(cx.theme().background)
                        .child(
                            Button::new("receive-incoming-close")
                                .primary()
                                .h(px(48.))
                                .px(px(42.))
                                .rounded_md()
                                .child(if is_completed { "完成" } else { "关闭" })
                                .on_click(cx.listener(move |this, _e, window, cx| {
                                    if let Some(active) = this.inbox_state.read(cx).active.as_ref() {
                                        if !active.completed && !active.cancelled && !active.is_message_only {
                                            crate::core::receive_events::request_incoming_cancel(
                                                active.session_id.clone(),
                                            );
                                        }
                                    }
                                    this.inbox_state.update(cx, |s, state_cx| {
                                        s.clear();
                                        state_cx.notify();
                                    });
                                    if let Some(root) = &this.root {
                                        let _ = root.update(cx, |this, cx| {
                                            this.go_back_or_navigate(routes::HOME, cx);
                                        });
                                    } else {
                                        if let Some(entry) =
                                            crate::ui::router_history::RouterHistoryState::global_mut(cx)
                                                .history
                                                .go_back()
                                        {
                                            RouterState::global_mut(cx).location.pathname = entry.pathname;
                                        } else {
                                            RouterState::global_mut(cx).location.pathname = routes::HOME.into();
                                        }
                                    }
                                    window.refresh();
                                })),
                        ),
                )
            })
    }
}

fn format_receive_size(bytes: u64) -> String {
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

fn visual_tag(fingerprint: &str) -> String {
    if fingerprint.is_empty() {
        return "--".to_string();
    }
    let mut sum: u32 = 0;
    for b in fingerprint.as_bytes() {
        sum = sum.wrapping_add(*b as u32);
    }
    format!("{:02}", (sum % 100))
}
