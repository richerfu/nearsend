use crate::state::transfer_state::{TransferInfo, TransferStatus};
use crate::ui::icons::{app_icon, paths};
use crate::ui::theme::{radius, sizing, spacing};
use gpui::{div, prelude::*, px, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    progress::Progress,
    v_flex, ActiveTheme as _, Size, StyledExt as _,
};

/// Transfer item component for mobile design — shows per-file transfer progress.
#[derive(IntoElement)]
pub struct TransferItem {
    transfer: TransferInfo,
    on_cancel: Option<std::rc::Rc<dyn Fn(&str, &mut Window, &mut gpui::App) + 'static>>,
    on_open: Option<std::rc::Rc<dyn Fn(&str, &mut Window, &mut gpui::App) + 'static>>,
}

impl TransferItem {
    pub fn new(transfer: TransferInfo) -> Self {
        Self {
            transfer,
            on_cancel: None,
            on_open: None,
        }
    }

    #[allow(dead_code)]
    pub fn on_cancel<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut gpui::App) + 'static,
    {
        self.on_cancel = Some(std::rc::Rc::new(handler));
        self
    }

    #[allow(dead_code)]
    pub fn on_open<F>(mut self, handler: F) -> Self
    where
        F: Fn(&str, &mut Window, &mut gpui::App) + 'static,
    {
        self.on_open = Some(std::rc::Rc::new(handler));
        self
    }
}

impl gpui::RenderOnce for TransferItem {
    fn render(self, _window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let (status_text, status_icon_path) = match self.transfer.status {
            TransferStatus::Pending => ("等待中", paths::LOADER),
            TransferStatus::InProgress => ("传输中", paths::UPLOAD),
            TransferStatus::Completed => ("已完成", paths::CHECK),
            TransferStatus::Failed => ("失败", paths::X),
            TransferStatus::Cancelled => ("已取消", paths::X),
            TransferStatus::Skipped => ("已跳过", paths::X),
        };

        let status_color = match self.transfer.status {
            TransferStatus::Completed => cx.theme().success,
            TransferStatus::Failed => cx.theme().danger,
            TransferStatus::Cancelled | TransferStatus::Skipped => cx.theme().muted_foreground,
            _ => cx.theme().foreground,
        };

        let transfer_id = self.transfer.id.clone();
        let on_cancel = self.on_cancel.clone();
        let on_open = self.on_open.clone();
        let transfer_id_open = self.transfer.id.clone();

        div()
            .bg(cx.theme().background)
            .rounded(radius::LG)
            .p(sizing::CARD_PADDING)
            .border_1()
            .border_color(cx.theme().border.opacity(0.8))
            .child(
                v_flex()
                    .gap(spacing::SM)
                    .w_full()
                    .child(
                        h_flex()
                            .items_center()
                            .gap(spacing::SM)
                            .w_full()
                            .child(app_icon(paths::FILE, Size::Small, cx.theme().muted_foreground))
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
                                            .text_sm()
                                            .font_semibold()
                                            .text_color(cx.theme().foreground)
                                            .child(self.transfer.file_name.clone()),
                                    )
                                    .child(
                                        h_flex()
                                            .gap(px(6.))
                                            .items_center()
                                            .child(app_icon(
                                                status_icon_path,
                                                Size::XSmall,
                                                status_color,
                                            ))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(status_color)
                                                    .child(status_text),
                                            ),
                                    ),
                            )
                            .when(
                                self.transfer.status == TransferStatus::InProgress
                                    && on_cancel.is_some(),
                                |this| {
                                    this.child(
                                        Button::new("cancel-transfer")
                                            .ghost()
                                            .on_click(move |_event, window, cx| {
                                                if let Some(ref handler) = on_cancel {
                                                    handler(&transfer_id, window, cx);
                                                }
                                            })
                                            .child(app_icon(paths::X, Size::Small, cx.theme().danger)),
                                    )
                                },
                            )
                            .when(
                                self.transfer.status == TransferStatus::Completed
                                    && on_open.is_some(),
                                |this| {
                                    this.child(
                                        Button::new("open-transfer")
                                            .ghost()
                                            .on_click(move |_event, window, cx| {
                                                if let Some(ref handler) = on_open {
                                                    handler(&transfer_id_open, window, cx);
                                                }
                                            })
                                            .child(app_icon(
                                                paths::EXTERNAL_LINK,
                                                Size::Small,
                                                cx.theme().foreground,
                                            )),
                                    )
                                },
                            ),
                    )
                    .when(self.transfer.status == TransferStatus::InProgress, |this| {
                        this.child(
                            Progress::new("transfer-progress")
                                .value((self.transfer.progress * 100.0) as f32)
                                .w_full(),
                        )
                    })
                    .when(self.transfer.status == TransferStatus::InProgress, |this| {
                        this.child(
                            h_flex()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{:.1}%", self.transfer.progress * 100.0,)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "{}/{}",
                                            format_bytes(self.transfer.bytes_sent),
                                            format_bytes(self.transfer.total_bytes),
                                        )),
                                )
                                .when(self.transfer.speed_bytes_per_sec > 0, |this| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "{}/s",
                                                format_bytes(self.transfer.speed_bytes_per_sec),
                                            )),
                                    )
                                }),
                        )
                    }),
            )
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
