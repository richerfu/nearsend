//! Progress page: shows real-time transfer progress with per-file progress bars.
//! Route: /transfer/progress

use crate::state::transfer_state::{
    TransferDirection, TransferInfo, TransferState, TransferStatus,
};
use crate::ui::components::chrome::{
    back_icon_button, empty_state, header_icon_button, page_header,
};
use crate::ui::components::transfer_item::TransferItem;
use crate::ui::icons::{app_icon, paths};
use crate::ui::routes;
use crate::ui::theme::spacing;
use gpui::{div, prelude::*, px, Context, Entity, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    progress::Progress,
    v_flex, ActiveTheme as _, Size, StyledExt as _,
};
use gpui_router::RouterState;

/// Progress page: full-screen view of ongoing transfer.
pub struct ProgressPage {
    pub root: Option<Entity<crate::app::AppRoot>>,
    transfer_state: Option<Entity<TransferState>>,
    direction: TransferDirection,
}

impl ProgressPage {
    pub fn new(
        root: Entity<crate::app::AppRoot>,
        transfer_state: Entity<TransferState>,
        direction: TransferDirection,
    ) -> Self {
        Self {
            root: Some(root),
            transfer_state: Some(transfer_state),
            direction,
        }
    }
}

impl gpui::Render for ProgressPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let direction_label = match self.direction {
            TransferDirection::Send => "发送中",
            TransferDirection::Receive => "接收中",
        };

        let direction_icon = match self.direction {
            TransferDirection::Send => paths::UPLOAD,
            TransferDirection::Receive => paths::DOWNLOAD,
        };

        let transfer = self.transfer_state.as_ref().and_then(|transfer_state| {
            transfer_state
                .read(cx)
                .snapshot_latest_by_direction(self.direction)
        });

        let (progress_val, status, speed_text, file_count_text, current_file_name) =
            if let Some(ref t) = transfer {
                let speed = if t.speed_bytes_per_sec > 0 {
                    format!("{}/s", format_bytes(t.speed_bytes_per_sec))
                } else {
                    String::new()
                };
                let file_count = format!(
                    "{}/{}",
                    t.files
                        .iter()
                        .filter(|f| f.status == TransferStatus::Completed)
                        .count(),
                    t.files.len()
                );
                let current = t.file_name.clone();
                (t.progress, t.status, speed, file_count, current)
            } else {
                (
                    0.0,
                    TransferStatus::Pending,
                    String::new(),
                    "0/0".to_string(),
                    String::new(),
                )
            };

        let is_done = matches!(
            status,
            TransferStatus::Completed | TransferStatus::Failed | TransferStatus::Cancelled
        );

        let trailing = if self.direction == TransferDirection::Send
            && status != TransferStatus::InProgress
        {
            header_icon_button("progress-retry", paths::REFRESH, cx, |_this, window, _cx| {
                crate::core::send_retry_events::request_send_retry();
                window.refresh();
            })
            .into_any_element()
        } else {
            div().w(px(40.)).h(px(40.)).into_any_element()
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(page_header(
                direction_label,
                back_icon_button("progress-back", cx, |this, window, cx| {
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
                h_flex()
                    .items_center()
                    .gap(px(4.))
                    .child(app_icon(direction_icon, Size::Small, cx.theme().foreground))
                    .child(trailing),
                cx,
            ))
            // Content
            .child(
                div().flex_1().w_full().overflow_y_scrollbar().child(
                    v_flex()
                        .w_full()
                        .px(spacing::PAGE)
                        .py(px(20.))
                        .gap(spacing::MD)
                        // Overall progress
                        .child(
                            v_flex()
                                .gap(spacing::SM)
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(file_count_text),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("{:.0}%", progress_val * 100.0)),
                                        ),
                                )
                                .child(
                                    Progress::new("overall-progress")
                                        .value((progress_val * 100.0) as f32)
                                        .w_full(),
                                )
                                .when(!current_file_name.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .text_base()
                                            .font_semibold()
                                            .text_color(cx.theme().foreground)
                                            .overflow_hidden()
                                            .truncate()
                                            .child(format!("当前文件：{}", current_file_name)),
                                    )
                                })
                                .when(!speed_text.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(speed_text),
                                    )
                                }),
                        )
                        // Per-file list
                        .when_some(transfer.clone(), |this, t| {
                            this.children(t.files.iter().map(|file| {
                                let file_transfer = TransferInfo {
                                    id: file.file_id.clone(),
                                    device_name: t.device_name.clone(),
                                    status: file.status,
                                    direction: t.direction,
                                    progress: if file.file_size > 0 {
                                        file.bytes_transferred as f64 / file.file_size as f64
                                    } else {
                                        0.0
                                    },
                                    bytes_sent: file.bytes_transferred,
                                    total_bytes: file.file_size,
                                    file_name: file.file_name.clone(),
                                    speed_bytes_per_sec: 0,
                                    eta_seconds: None,
                                    files: vec![],
                                };
                                div()
                                    .mb(spacing::SM)
                                    .child(TransferItem::new(file_transfer))
                            }))
                        })
                        // Empty state
                        .when(transfer.is_none(), |this| {
                            this.child(empty_state(
                                paths::UPLOAD,
                                "暂无传输",
                                "开始发送或接收后会在这里显示进度",
                                cx,
                            ))
                        }),
                ),
            )
            // Bottom action button
            .child(div().w_full().px(spacing::PAGE).py(px(15.)).child(if is_done {
                Button::new("progress-done")
                    .primary()
                    .w_full()
                    .on_click(cx.listener(|this, _event, window, cx| {
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
                    }))
                    .child("完成")
            } else {
                Button::new("progress-cancel")
                    .with_variant(gpui_component::button::ButtonVariant::Danger)
                    .outline()
                    .w_full()
                    .on_click(cx.listener(|_this, _event, window, _cx| {
                        crate::core::send_cancel_events::request_send_cancel();
                        window.refresh();
                    }))
                    .child("取消")
            }))
    }
}

impl Default for ProgressPage {
    fn default() -> Self {
        Self {
            root: None,
            transfer_state: None,
            direction: TransferDirection::Send,
        }
    }
}

fn format_bytes(bytes: u64) -> String {
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
