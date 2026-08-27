//! Receive dialog: modal shown when another device sends a transfer request.

use crate::ui::icons::{app_icon, paths};
use crate::ui::theme::spacing;
use gpui::{div, prelude::*, px, Context, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Size, StyledExt as _,
};
use localsend::http::state::ClientInfo;
use localsend::model::transfer::FileDto;

/// Receive dialog state.
#[allow(dead_code)]
pub struct ReceiveDialog {
    pub sender: ClientInfo,
    pub files: Vec<FileDto>,
    pub selected_files: Vec<bool>,
    pub session_id: String,
}

#[allow(dead_code)]
impl ReceiveDialog {
    pub fn new(sender: ClientInfo, files: Vec<FileDto>, session_id: String) -> Self {
        let selected = vec![true; files.len()];
        Self {
            sender,
            files,
            selected_files: selected,
            session_id,
        }
    }
}

impl gpui::Render for ReceiveDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let sender_alias = self.sender.alias.clone();
        let sender_model = self.sender.device_model.clone().unwrap_or_default();
        let files = self.files.clone();
        let selected = self.selected_files.clone();
        let total_size: u64 = files
            .iter()
            .enumerate()
            .filter(|(i, _)| selected.get(*i).copied().unwrap_or(false))
            .map(|(_, f)| f.size)
            .sum();
        let selected_count = selected.iter().filter(|&&s| s).count();

        let device_icon = match self.sender.device_type {
            Some(localsend::model::discovery::DeviceType::Mobile) => paths::SMARTPHONE,
            Some(localsend::model::discovery::DeviceType::Desktop) => paths::MONITOR,
            Some(localsend::model::discovery::DeviceType::Web) => paths::GLOBE,
            Some(localsend::model::discovery::DeviceType::Server)
            | Some(localsend::model::discovery::DeviceType::Headless) => paths::SERVER,
            None => paths::SMARTPHONE,
        };

        v_flex()
            .w_full()
            .max_h(px(500.))
            .bg(cx.theme().background)
            .rounded_lg()
            .p(px(20.))
            .gap(spacing::MD)
            // Sender info header
            .child(
                h_flex()
                    .items_center()
                    .gap(spacing::MD)
                    .child(
                        div()
                            .w(px(48.))
                            .h(px(48.))
                            .rounded_full()
                            .bg(cx.theme().muted)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(app_icon(device_icon, Size::Large, cx.theme().foreground)),
                    )
                    .child(
                        v_flex()
                            .min_w(px(0.))
                            .gap(px(2.))
                            .child(
                                div()
                                    .w_full()
                                    .overflow_hidden()
                                    .truncate()
                                    .text_lg()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child(sender_alias),
                            )
                            .when(!sender_model.is_empty(), |this| {
                                this.child(
                                    div()
                                        .w_full()
                                        .overflow_hidden()
                                        .truncate()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(sender_model),
                                )
                            }),
                    ),
            )
            // File list
            .child(
                div()
                    .flex_1()
                    .min_h(px(100.))
                    .max_h(px(300.))
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .gap(spacing::SM)
                            .children(files.iter().enumerate().map(|(i, file)| {
                                let is_selected = selected.get(i).copied().unwrap_or(true);
                                let file_name = file.file_name.clone();
                                let file_size = format_file_size(file.size);
                                div()
                                    .id(format!("receive-file-{}", i))
                                    .bg(if is_selected {
                                        cx.theme().secondary
                                    } else {
                                        cx.theme().muted
                                    })
                                    .rounded_md()
                                    .p(px(10.))
                                    .cursor_default()
                                    .on_click(cx.listener(move |this, _event, _window, _cx| {
                                        if let Some(sel) = this.selected_files.get_mut(i) {
                                            *sel = !*sel;
                                        }
                                    }))
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap(spacing::SM)
                                            .child(
                                                div()
                                                    .w(px(20.))
                                                    .h(px(20.))
                                                    .rounded(px(4.))
                                                    .border_1()
                                                    .border_color(if is_selected {
                                                        cx.theme().primary
                                                    } else {
                                                        cx.theme().border
                                                    })
                                                    .bg(if is_selected {
                                                        cx.theme().primary
                                                    } else {
                                                        cx.theme().background
                                                    })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .when(is_selected, |this| {
                                                        this.child(
                                                            app_icon(
                                                                paths::CHECK,
                                                                Size::XSmall,
                                                                cx.theme().primary_foreground,
                                                            ),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                app_icon(
                                                    paths::FILE,
                                                    Size::Small,
                                                    cx.theme().muted_foreground,
                                                ),
                                            )
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w(px(0.))
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .overflow_hidden()
                                                            .truncate()
                                                            .text_sm()
                                                            .text_color(cx.theme().foreground)
                                                            .child(file_name),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(file_size),
                                                    ),
                                            ),
                                    )
                            })),
                    ),
            )
            // Summary
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} 个文件", selected_count)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format_file_size(total_size)),
                    ),
            )
            // Action buttons
            .child(
                h_flex()
                    .gap(spacing::MD)
                    .child(
                        Button::new("decline-transfer")
                            .outline()
                            .flex_1()
                            .on_click(cx.listener(|_this, _event, _window, _cx| {
                                log::info!("Decline transfer");
                            }))
                            .child("拒绝"),
                    )
                    .child(
                        Button::new("accept-transfer")
                            .primary()
                            .flex_1()
                            .on_click(cx.listener(|_this, _event, _window, _cx| {
                                log::info!("Accept transfer");
                            }))
                            .child("接受"),
                    ),
            )
    }
}

#[allow(dead_code)]
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
