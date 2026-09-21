//! Receive tab content for home page (top bar, middle logo, bottom Quick Save bar).

use super::HomePage;
use super::QuickSaveMode;
use crate::ui::components::chrome::{circle_icon_button, dialog_title, CircleIconTrigger};
use crate::ui::components::logo::Logo;
use crate::ui::icons::paths;
use crate::ui::responsive::ResponsiveLayout;
use crate::ui::routes;
use crate::ui::theme::{radius, spacing};
use gpui::{div, prelude::*, px, Anchor, AnyElement, Context, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonVariant, ButtonVariants as _},
    dialog::{DialogAction, DialogButtonProps, DialogFooter},
    h_flex,
    popover::Popover,
    v_flex, ActiveTheme as _, StyledExt as _, WindowExt as _,
};

pub fn render_receive_content(
    home: &mut HomePage,
    window: &mut Window,
    cx: &mut Context<HomePage>,
) -> AnyElement {
    let layout = ResponsiveLayout::current(window, cx);
    let desktop = layout.is_desktop();
    let show_advanced = home.receive_state.show_advanced;
    let quick_save_mode = home.receive_state.quick_save_mode;
    let server_alias = home.receive_state.server_alias.clone();
    let server_ips = home.receive_state.server_ips.clone();
    let server_port = home.receive_state.server_port;
    let server_running = home.receive_state.server_running;
    let animations = home.settings_state.animations;
    let home_entity = cx.entity();
    let quick_save_selected_index = match quick_save_mode {
        QuickSaveMode::Off => 0,
        QuickSaveMode::Favorites => 1,
        QuickSaveMode::On => 2,
    };

    let actions = render_receive_actions(
        home_entity.clone(),
        show_advanced,
        server_alias.clone(),
        server_ips.clone(),
        server_port.to_string(),
        cx,
    );
    let quick_save_control = render_quick_save_control(home_entity, quick_save_selected_index, cx);
    let visual_id = if server_running && !server_ips.is_empty() {
        format_visual_ip_ids(&server_ips)
    } else {
        "离线".to_string()
    };

    if desktop {
        let status_color = if server_running {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground
        };
        let quick_save_description = match quick_save_mode {
            QuickSaveMode::Off => "收到文件时先询问，适合公共网络。",
            QuickSaveMode::Favorites => "自动接收收藏夹设备发送的内容。",
            QuickSaveMode::On => "自动接收同一网络内的所有请求。",
        };

        div()
            .size_full()
            .overflow_y_scrollbar()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .w_full()
                    .h_full()
                    .min_h(px(560.))
                    .max_w(px(760.))
                    .mx_auto()
                    .p(layout.page_padding)
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div().w(px(8.)).h(px(8.)).rounded_full().bg(status_color),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_medium()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(if server_running {
                                                "正在接收"
                                            } else {
                                                "接收服务已暂停"
                                            }),
                                    ),
                            )
                            .child(actions),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h(px(280.))
                            .items_center()
                            .justify_center()
                            .gap(px(12.))
                            .child(
                                div()
                                    .w(px(148.))
                                    .h(px(148.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Logo::default()
                                            .size(148.)
                                            .spinning(server_running && animations)
                                            .duration(15),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .overflow_hidden()
                                    .truncate()
                                    .text_3xl()
                                    .font_bold()
                                    .text_color(cx.theme().foreground)
                                    .text_center()
                                    .child(server_alias),
                            )
                            .child(
                                div()
                                    .rounded_full()
                                    .bg(cx.theme().muted)
                                    .px(px(12.))
                                    .py(px(5.))
                                    .text_sm()
                                    .font_medium()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(visual_id),
                            ),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(560.))
                            .mx_auto()
                            .flex_none()
                            .items_center()
                            .gap(px(10.))
                            .border_t_1()
                            .border_color(cx.theme().border.opacity(0.8))
                            .pt(px(24.))
                            .pb(px(8.))
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child("自动保存"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .text_color(cx.theme().muted_foreground)
                                    .text_center()
                                    .child("选择收到传输请求时的处理方式。"),
                            )
                            .child(quick_save_control)
                            .child(
                                div()
                                    .text_xs()
                                    .line_height(px(18.))
                                    .text_color(cx.theme().muted_foreground)
                                    .text_center()
                                    .child(quick_save_description),
                            ),
                    ),
            )
            .into_any_element()
    } else {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .w_full()
                    .h(px(52.))
                    .px(layout.page_padding)
                    .items_center()
                    .justify_end()
                    .child(actions),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .w_full()
                            .h_full()
                            .min_h(px(440.))
                            .max_w(px(600.))
                            .mx_auto()
                            .items_center()
                            .justify_center()
                            .px(layout.page_padding)
                            .py(px(24.))
                            .child(
                                v_flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .w(px(168.))
                                            .h(px(168.))
                                            .flex_none()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                Logo::default()
                                                    .size(168.)
                                                    .spinning(server_running && animations)
                                                    .duration(15),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .max_w(px(520.))
                                            .overflow_hidden()
                                            .truncate()
                                            .text_2xl()
                                            .font_bold()
                                            .text_color(cx.theme().foreground)
                                            .text_center()
                                            .child(server_alias),
                                    )
                                    .child(
                                        div()
                                            .max_w(px(520.))
                                            .text_base()
                                            .text_color(cx.theme().muted_foreground)
                                            .text_center()
                                            .child(visual_id),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .w_full()
                                    .max_w(px(420.))
                                    .items_center()
                                    .gap(spacing::MD)
                                    .mt(px(24.))
                                    .border_t_1()
                                    .border_color(cx.theme().border.opacity(0.8))
                                    .pt(px(20.))
                                    .child(
                                        div()
                                            .text_base()
                                            .font_medium()
                                            .text_color(cx.theme().foreground)
                                            .child("自动保存"),
                                    )
                                    .child(quick_save_control),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn render_receive_actions(
    home_entity: gpui::Entity<HomePage>,
    show_advanced: bool,
    info_alias: String,
    info_ips: Vec<String>,
    server_port: String,
    cx: &mut Context<HomePage>,
) -> AnyElement {
    h_flex()
        .items_center()
        .gap(spacing::SM)
        .child(circle_icon_button(
            "receive-history",
            paths::HISTORY,
            cx,
            |this, _event, window, cx| {
                this.navigate_to(routes::RECEIVE_HISTORY, cx);
                window.refresh();
            },
        ))
        .child(
            div().flex_none().child(
                Popover::new("receive-info")
                    .anchor(Anchor::TopRight)
                    .overlay_closable(false)
                    .open(show_advanced)
                    .on_open_change({
                        let home_entity = home_entity.clone();
                        move |open, _window, cx| {
                            home_entity.update(cx, |this, _cx| {
                                this.receive_state.show_advanced = *open;
                            });
                        }
                    })
                    .trigger(
                        CircleIconTrigger::from_path("receive-info-trigger", paths::INFO, cx)
                            .touch(),
                    )
                    .content(move |_state, _window, cx| {
                        v_flex()
                            .gap(spacing::SM)
                            .child(render_info_row("Alias:", &info_alias, cx))
                            .child(
                                h_flex()
                                    .items_start()
                                    .child(
                                        div()
                                            .w(px(60.))
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("IP:"),
                                    )
                                    .child(if info_ips.is_empty() {
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child("Unknown")
                                    } else {
                                        v_flex().gap(px(2.)).items_start().children(
                                            info_ips.iter().map(|ip| {
                                                div()
                                                    .w_full()
                                                    .overflow_hidden()
                                                    .truncate()
                                                    .text_sm()
                                                    .text_color(cx.theme().foreground)
                                                    .child(ip.clone())
                                            }),
                                        )
                                    }),
                            )
                            .child(render_info_row("Port:", &server_port, cx))
                    }),
            ),
        )
        .into_any_element()
}

fn render_quick_save_control(
    home_entity: gpui::Entity<HomePage>,
    selected_index: usize,
    cx: &mut Context<HomePage>,
) -> AnyElement {
    let selected_bg = cx.theme().background;
    let track_bg = cx.theme().muted;
    let selected_fg = cx.theme().foreground;
    let idle_fg = cx.theme().muted_foreground;

    h_flex()
        .id("receive-quick-save")
        .w_full()
        .max_w(px(360.))
        .h(px(42.))
        .rounded(radius::FULL)
        .p(px(3.))
        .bg(track_bg)
        .child(quick_save_chip(
            "quick-save-off",
            "关",
            selected_index == 0,
            selected_bg,
            selected_fg,
            idle_fg,
            {
                let home_entity = home_entity.clone();
                move |_event, _window, cx| {
                    home_entity.update(cx, |this, _cx| {
                        set_quick_save_mode(this, QuickSaveMode::Off);
                    });
                }
            },
        ))
        .child(quick_save_chip(
            "quick-save-favorites",
            "收藏夹",
            selected_index == 1,
            selected_bg,
            selected_fg,
            idle_fg,
            {
                let home_entity = home_entity.clone();
                move |_event, window, cx| {
                    home_entity.update(cx, |this, _cx| {
                        set_quick_save_mode(this, QuickSaveMode::Favorites);
                    });
                    open_quick_save_notice_dialog(QuickSaveMode::Favorites, window, cx);
                }
            },
        ))
        .child(quick_save_chip(
            "quick-save-on",
            "开",
            selected_index == 2,
            selected_bg,
            selected_fg,
            idle_fg,
            {
                let home_entity = home_entity.clone();
                move |_event, window, cx| {
                    home_entity.update(cx, |this, _cx| {
                        set_quick_save_mode(this, QuickSaveMode::On);
                    });
                    open_quick_save_notice_dialog(QuickSaveMode::On, window, cx);
                }
            },
        ))
        .into_any_element()
}

fn format_visual_ip_ids(server_ips: &[String]) -> String {
    let mut ids = Vec::new();
    for ip in server_ips {
        if let Some(id) = ip.split('.').next_back() {
            if !ids.iter().any(|existing| existing == id) {
                ids.push(id.to_string());
            }
        }
    }
    ids.into_iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn set_quick_save_mode(home: &mut HomePage, mode: QuickSaveMode) {
    home.receive_state.quick_save_mode = mode;
    match mode {
        QuickSaveMode::Off => {
            home.settings_state.quick_save = false;
            home.settings_state.quick_save_favorites = false;
        }
        QuickSaveMode::Favorites => {
            home.settings_state.quick_save = false;
            home.settings_state.quick_save_favorites = true;
        }
        QuickSaveMode::On => {
            home.settings_state.quick_save = true;
            home.settings_state.quick_save_favorites = false;
        }
    }
    home.persist_settings();
}

fn open_quick_save_notice_dialog(mode: QuickSaveMode, window: &mut Window, cx: &mut gpui::App) {
    let (title, lines) = match mode {
        QuickSaveMode::Favorites => (
            "自动保存来自“收藏夹(白名单)”设备的文件",
            vec![
                "当前会自动接受收藏夹中设备的文件请求。",
                "警告：这目前并非绝对安全，若您收藏夹列表中的设备指纹被黑客窃取，其仍可以向您发送文件。",
                "但是，此选项比“允许任何设备”更安全。",
            ],
        ),
        QuickSaveMode::On => (
            "自动保存",
            vec![
                "自动接受所有文件传输请求。请注意，这会让此网络中的所有人都可以向你发送文件。",
            ],
        ),
        QuickSaveMode::Off => return,
    };

    let title = title.to_string();
    let lines = lines
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(dialog_title(title.clone()))
            .overlay(true)
            .w(px(360.))
            .child(
                v_flex()
                    .w_full()
                    .gap(px(8.))
                    .children(lines.iter().map(|line| {
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(_cx.theme().foreground)
                            .child(line.clone())
                    })),
            )
            .button_props(
                DialogButtonProps::default()
                    .ok_text("确定")
                    .ok_variant(ButtonVariant::Danger),
            )
            .footer(build_alert_dialog_footer("quick-save-notice", "确定"))
    });
}

fn build_alert_dialog_footer(id_prefix: &str, ok_text: &str) -> DialogFooter {
    DialogFooter::new().child(
        DialogAction::new().child(
            Button::new(format!("{id_prefix}-ok"))
                .label(ok_text.to_string())
                .with_variant(ButtonVariant::Danger),
        ),
    )
}

/// Render a single info row with fixed-width label.
fn render_info_row(label: &str, value: &str, cx: &gpui::App) -> impl IntoElement {
    h_flex()
        .items_start()
        .w_full()
        .child(
            div()
                .w(px(60.))
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .truncate()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(value.to_string()),
        )
}

fn quick_save_chip(
    id: &'static str,
    label: &'static str,
    selected: bool,
    selected_bg: gpui::Hsla,
    selected_fg: gpui::Hsla,
    idle_fg: gpui::Hsla,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex_1()
        .h_full()
        .rounded(radius::FULL)
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .when(selected, |this| {
            this.bg(selected_bg).shadow(vec![gpui_component::box_shadow(
                px(0.),
                px(1.),
                px(3.),
                px(0.),
                gpui::hsla(0.0, 0.0, 0.0, 0.08),
            )])
        })
        .on_click(on_click)
        .child(
            div()
                .text_sm()
                .when(selected, |this| this.font_semibold())
                .text_color(if selected { selected_fg } else { idle_fg })
                .child(label),
        )
}
