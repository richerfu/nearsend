//! Receive tab content for home page (top bar, middle logo, bottom Quick Save bar).

use super::HomePage;
use super::QuickSaveMode;
use crate::ui::components::logo::Logo;
use crate::ui::routes;
use crate::ui::theme::spacing;
use gpui::{div, prelude::*, px, AnyElement, Context, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariant, ButtonVariants as _},
    dialog::{DialogAction, DialogButtonProps, DialogFooter},
    h_flex,
    popover::Popover,
    v_flex, ActiveTheme as _, Anchor, Icon, Sizable as _, Size, StyledExt as _, WindowExt as _,
};

pub fn render_receive_content(
    home: &mut HomePage,
    _window: &mut Window,
    cx: &mut Context<HomePage>,
) -> AnyElement {
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

    let info_alias = server_alias.clone();
    let info_ips = server_ips.clone();

    v_flex()
        .size_full()
        .bg(cx.theme().background)
        // Top bar: history + info buttons
        .child(
            h_flex()
                .w_full()
                .h(px(56.))
                .px(px(20.))
                .items_center()
                .justify_end()
                .gap(spacing::SM)
                // History button — always visible
                .child(render_circle_button(
                    "receive-history",
                    "icons/history.svg",
                    cx,
                    |this, _event, window, cx| {
                        this.navigate_to(routes::RECEIVE_HISTORY, cx);
                        window.refresh();
                    },
                ))
                // Info popover
                .child(
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
                            Button::new("receive-info")
                                .custom(
                                    ButtonCustomVariant::new(cx)
                                        .hover(cx.theme().transparent)
                                        .active(cx.theme().transparent),
                                )
                                .rounded_full()
                                .p(px(8.))
                                .child(
                                    div()
                                        .shadow(vec![gpui_component::box_shadow(
                                            px(0.),
                                            px(0.),
                                            px(0.),
                                            px(1.),
                                            cx.theme().foreground.opacity(0.10),
                                        )])
                                        .bg(cx.theme().foreground.opacity(0.04))
                                        .rounded_full()
                                        .w(px(44.))
                                        .h(px(44.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            Icon::default()
                                                .path("icons/info.svg")
                                                .with_size(Size::Medium),
                                        ),
                                ),
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
                                .child(render_info_row("Port:", &server_port.to_string(), cx))
                        }),
                ),
        )
        // Middle: logo + alias + IP display
        .child(
            div()
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .overflow_y_scrollbar()
                .child(
                    div()
                        .w_full()
                        .max_w(px(600.))
                        .mx_auto()
                        .p(px(30.))
                        .h_full()
                        .child(
                            v_flex()
                                .flex_1()
                                .items_center()
                                .justify_center()
                                .gap(spacing::MD)
                                .child(
                                    div()
                                        .w(px(200.))
                                        .h(px(200.))
                                        .flex_none()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            Logo::default()
                                                .size(200.)
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
                                        .text_3xl()
                                        .font_bold()
                                        .text_color(cx.theme().foreground)
                                        .text_center()
                                        .child(server_alias.clone()),
                                )
                                .child(
                                    div()
                                        .max_w(px(520.))
                                        .text_xl()
                                        .text_color(cx.theme().muted_foreground)
                                        .text_center()
                                        .child(if server_running && !server_ips.is_empty() {
                                            format_visual_ip_ids(&server_ips)
                                        } else {
                                            "Offline".to_string()
                                        }),
                                ),
                        ),
                ),
        )
        // Bottom: Quick Save selector
        .child(
            div().w_full().pt(px(20.)).pb(px(30.)).px(px(30.)).child(
                v_flex()
                    .gap(spacing::MD)
                    .items_center()
                    .child(
                        div()
                            .text_base()
                            .font_medium()
                            .text_color(cx.theme().foreground)
                            .child("自动保存"),
                    )
                    .child({
                        let quick_save_track_bg = cx.theme().secondary;
                        let quick_save_selected_bg = cx.theme().primary.opacity(0.16);
                        let quick_save_border = cx.theme().border.opacity(0.95);
                        let quick_save_divider = cx.theme().border.opacity(0.92);
                        let quick_save_text_color = cx.theme().foreground.opacity(0.95);

                        h_flex()
                            .id("receive-quick-save")
                            .w_full()
                            .max_w(px(350.))
                            .h(px(46.))
                            .rounded_full()
                            .overflow_hidden()
                            .p(px(1.))
                            .border_1()
                            .border_color(quick_save_border)
                            .bg(quick_save_track_bg)
                            .child(
                                div()
                                    .id("quick-save-off")
                                    .flex_1()
                                    .h_full()
                                    .cursor_pointer()
                                    .bg(if quick_save_selected_index == 0 {
                                        quick_save_selected_bg
                                    } else {
                                        cx.theme().transparent
                                    })
                                    .when(quick_save_selected_index == 0, |this| {
                                        this.rounded_l(px(999.))
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_click({
                                        let home_entity = home_entity.clone();
                                        move |_event, _window, cx| {
                                            home_entity.update(cx, |this, _cx| {
                                                set_quick_save_mode(this, QuickSaveMode::Off);
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_medium()
                                            .text_color(quick_save_text_color)
                                            .child("关"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .child(div().w(px(1.)).h(px(24.)).bg(quick_save_divider))
                                    .child(
                                        div()
                                            .id("quick-save-favorites")
                                            .flex_1()
                                            .h_full()
                                            .cursor_pointer()
                                            .bg(if quick_save_selected_index == 1 {
                                                quick_save_selected_bg
                                            } else {
                                                cx.theme().transparent
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .on_click({
                                                let home_entity = home_entity.clone();
                                                move |_event, window, cx| {
                                                    home_entity.update(cx, |this, _cx| {
                                                        set_quick_save_mode(
                                                            this,
                                                            QuickSaveMode::Favorites,
                                                        );
                                                    });
                                                    open_quick_save_notice_dialog(
                                                        QuickSaveMode::Favorites,
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            })
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_medium()
                                                    .text_color(quick_save_text_color)
                                                    .child("收藏夹"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .child(div().w(px(1.)).h(px(24.)).bg(quick_save_divider))
                                    .child(
                                        div()
                                            .id("quick-save-on")
                                            .flex_1()
                                            .h_full()
                                            .cursor_pointer()
                                            .bg(if quick_save_selected_index == 2 {
                                                quick_save_selected_bg
                                            } else {
                                                cx.theme().transparent
                                            })
                                            .when(quick_save_selected_index == 2, |this| {
                                                this.rounded_r(px(999.))
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .on_click(move |_event, window, cx| {
                                                home_entity.update(cx, |this, _cx| {
                                                    set_quick_save_mode(this, QuickSaveMode::On);
                                                });
                                                open_quick_save_notice_dialog(
                                                    QuickSaveMode::On,
                                                    window,
                                                    cx,
                                                );
                                            })
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_medium()
                                                    .text_color(quick_save_text_color)
                                                    .child("开"),
                                            ),
                                    ),
                            )
                    }),
            ),
        )
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
            .title(
                div()
                    .text_lg()
                    .font_semibold()
                    .text_color(_cx.theme().foreground)
                    .child(title.clone()),
            )
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

/// Render a circular icon button (used for history, info, etc.).
fn render_circle_button(
    id: &str,
    icon_path: &str,
    cx: &Context<HomePage>,
    on_click: impl Fn(&mut HomePage, &gpui::ClickEvent, &mut Window, &mut Context<HomePage>) + 'static,
) -> impl IntoElement {
    let icon_path = icon_path.to_string();
    div()
        .id(id.to_string())
        .cursor_default()
        .rounded_full()
        .p(px(8.))
        .child(
            div()
                .shadow(vec![gpui_component::box_shadow(
                    px(0.),
                    px(0.),
                    px(0.),
                    px(1.),
                    cx.theme().foreground.opacity(0.10),
                )])
                .bg(cx.theme().foreground.opacity(0.04))
                .rounded_full()
                .w(px(44.))
                .h(px(44.))
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::default().path(icon_path).with_size(Size::Medium)),
        )
        .on_click(cx.listener(on_click))
}
