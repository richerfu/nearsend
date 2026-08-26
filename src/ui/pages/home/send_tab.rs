//! Send tab: select content type, files, and nearby devices (LocalSend-aligned layout).

use super::HomePage;
use crate::ui::components::chrome::{circle_icon_slot, muted_card, section_title};
use crate::ui::components::{
    device_card::DeviceCard, device_placeholder::DevicePlaceholder,
    opacity_slideshow::OpacitySlideshow,
};
use crate::ui::icons::{app_icon, paths};
use crate::ui::routes;
use crate::ui::theme::{radius, spacing};
use crate::ui::utils::format_file_size;
use gpui::{
    div, percentage, prelude::*, px, Anchor, Animation, AnimationExt as _, AnyElement, Context,
    Transformation, Window,
};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    h_flex,
    popover::Popover,
    v_flex, ActiveTheme as _, Size, StyledExt as _,
};
use std::time::Duration;

/// Render a content-type selector button (File / Media / Text / Folder).
/// Primary background + white text, no hover/active state change.
fn render_content_type_button(
    id: impl Into<gpui::ElementId>,
    icon_path: impl Into<gpui::SharedString>,
    label: &str,
    cx: &mut Context<HomePage>,
    on_click: impl Fn(&mut HomePage, &mut Window, &mut Context<HomePage>) + 'static,
) -> AnyElement {
    let icon_path = icon_path.into();
    let fg = cx.theme().foreground;
    let fill = cx.theme().foreground.opacity(0.08);
    let hover_fill = cx.theme().foreground.opacity(0.12);
    let active_fill = cx.theme().foreground.opacity(0.16);
    let border = cx.theme().border;

    Button::new(id)
        .flex_1()
        .custom(
            ButtonCustomVariant::new(cx)
                .color(fill)
                .foreground(fg)
                .hover(hover_fill)
                .active(active_fill),
        )
        .h(px(76.))
        .rounded(radius::LG)
        .border_1()
        .border_color(border)
        .on_click(cx.listener(move |this, _event, window, cx| {
            on_click(this, window, cx);
        }))
        .child(
            v_flex()
                .w_full()
                .h_full()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(
                    div()
                        .w(px(32.))
                        .h(px(32.))
                        .rounded(radius::MD)
                        .bg(cx.theme().background.opacity(0.72))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(app_icon(icon_path, Size::Small, fg)),
                )
                .child(
                    div()
                        .text_xs()
                        .font_medium()
                        .text_center()
                        .text_color(fg)
                        .child(label.to_string()),
                ),
        )
        .into_any_element()
}

/// Render a circular action button (scan / send / favorites / settings).
fn render_action_button(
    id: impl Into<gpui::ElementId>,
    icon_path: impl Into<gpui::SharedString>,
    spinning: bool,
    animations: bool,
    cx: &mut Context<HomePage>,
    on_click: impl Fn(&mut HomePage, &mut Window, &mut Context<HomePage>) + 'static,
) -> AnyElement {
    let icon_path = icon_path.into();
    let icon_el = app_icon(icon_path, Size::Small, cx.theme().foreground);

    let icon_element = if spinning && animations {
        icon_el
            .with_animation(
                "send-action-refresh-spin",
                Animation::new(Duration::from_millis(900)).repeat(),
                |this, delta| this.transform(Transformation::rotate(percentage(delta))),
            )
            .into_any_element()
    } else {
        icon_el.into_any_element()
    };

    div()
        .id(id)
        .cursor_pointer()
        .child(circle_icon_slot(icon_element, cx))
        .on_click(cx.listener(move |this, _event, window, cx| {
            on_click(this, window, cx);
        }))
        .into_any_element()
}

pub fn render_send_content(
    app: &mut HomePage,
    _window: &mut Window,
    cx: &mut Context<HomePage>,
) -> AnyElement {
    app.hydrate_nearby_devices_from_cache(cx);

    if !app.send_state.has_scanned_once
        && !app.send_state.scanning
        && app.send_state.nearby_devices.is_empty()
    {
        app.start_discovery_scan(false, cx);
    }

    let selected_files = app.send_state.selected_files.clone();
    let has_files = !selected_files.is_empty();
    let scanning = app.send_state.scanning;
    let total_size = app.send_state.selected_files_total_size;
    let animations = app.settings_state.animations;
    let home_entity = cx.entity();

    v_flex()
    .size_full()
    .bg(cx.theme().background)
    .relative()
    .child(
        div()
            .size_full()
            .w_full()
            .overflow_y_scrollbar()
            .child(
                v_flex()
                    .w_full()
                    .pt(px(20.))
                    .child(
                v_flex()
                    .w_full()
                    .gap(spacing::SM)
                    .child(
                        div()
                            .px(spacing::PAGE)
                            .child(section_title("选择", cx)),
                    )
                    .child(
                        div()
                            .px(spacing::PAGE)
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap(px(8.))
                                    .items_stretch()
                                    .child(render_content_type_button(
                                        "content-file",
                                        paths::FILE,
                                        "文件",
                                        cx,
                                        |this, window, cx| {
                                            this.handle_pick_content(
                                                super::SendContentType::File,
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .child(render_content_type_button(
                                        "content-folder",
                                        paths::FOLDER,
                                        "文件夹",
                                        cx,
                                        |this, window, cx| {
                                            this.handle_pick_content(
                                                super::SendContentType::Folder,
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .child(render_content_type_button(
                                        "content-text",
                                        paths::BOOK_OPEN,
                                        "文本",
                                        cx,
                                        |this, window, cx| {
                                            this.handle_pick_content(
                                                super::SendContentType::Text,
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .child(render_content_type_button(
                                        "content-clipboard",
                                        paths::COPY,
                                        "剪贴板",
                                        cx,
                                        |this, window, cx| {
                                            this.handle_pick_content(
                                                super::SendContentType::Media,
                                                window,
                                                cx,
                                            );
                                        },
                                    )),
                            ),
                    )
                    // -- Selected files card --
                    .when(has_files, |this| {
                        let file_count = selected_files.len();
                        this.child(
                            muted_card(cx)
                                .mx(spacing::PAGE)
                                .mb(px(10.))
                                .pl(px(15.))
                                .pt(px(8.))
                                .pb(px(15.))
                                .child(
                                    v_flex()
                                        .gap(spacing::MD)
                                        // Card title: file count
                                        .child(
                                            h_flex()
                                                .justify_between()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_base()
                                                        .font_weight(gpui::FontWeight::BLACK)
                                                        .text_color(cx.theme().foreground)
                                                        .child(format!("{} 个文件已选择", file_count)),
                                                )
                                                .child(
                                                    Button::new("clear")
                                                        .ghost()
                                                        .on_click(cx.listener(|this, _event, _window, _cx| {
                                                            this.send_selection_state.update(_cx, |state, _| {
                                                                state.clear();
                                                            });
                                                        }))
                                                        .child(app_icon(
                                                            paths::X,
                                                            Size::Small,
                                                            cx.theme().muted_foreground,
                                                        )),
                                                ),
                                        )
                                        // File count + total size
                                        .child(
                                            v_flex()
                                                .gap(spacing::XS)
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(cx.theme().foreground)
                                                        .child(format!("{} 个文件", file_count)),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(format_file_size(total_size)),
                                                ),
                                        )
                                        // File thumbnails
                                        .child(
                                            div()
                                                .child(
                                                    h_flex()
                                                        .gap(px(10.))
                                                        .children(selected_files.iter().map(|file| {
                                                            let icon_path = if file.text_content.is_some() {
                                                                paths::BOOK_OPEN
                                                            } else {
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
                                                            };
                                                            div()
                                                                .child(
                                                                    div()
                                                                        .w(px(48.))
                                                                        .h(px(48.))
                                                                        .bg(cx.theme().primary.opacity(0.12))
                                                                        .rounded(radius::MD)
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .child(app_icon(
                                                                            icon_path,
                                                                            Size::Small,
                                                                            cx.theme().foreground,
                                                                        )),
                                                                )
                                                        })),
                                                ),
                                        )
                                        // Edit / Add buttons
                                        .child(
                                            h_flex()
                                                .justify_end()
                                                .gap(px(15.))
                                                .child(
                                                    Button::new("edit")
                                                        .ghost()
                                                        .on_click(cx.listener(|_this, _event, window, cx| {
                                                            _this.navigate_to(routes::SEND_FILES, cx);
                                                            window.refresh();
                                                        }))
                                                        .child("编辑"),
                                                )
                                                .child(
                                                    Button::new("add")
                                                        .with_variant(gpui_component::button::ButtonVariant::Primary)
                                                        .on_click(cx.listener(|this, _event, window, cx| {
                                                            this.open_add_content_dialog(window, cx);
                                                        }))
                                                        .child(
                                                            h_flex()
                                                                .items_center()
                                                                .gap(px(6.))
                                                                .child(app_icon(
                                                                    paths::PLUS,
                                                                    Size::Small,
                                                                    cx.theme().primary_foreground,
                                                                ))
                                                                .child("添加"),
                                                        ),
                                                ),
                                        ),
                                ),
                        )
                    })
                    // -- Nearby devices section --
                    .child(
                        div()
                            .px(spacing::PAGE)
                            .pt(px(8.))
                            .child(
                                h_flex()
                                    .gap(px(10.))
                                    .items_center()
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(section_title("附近的设备", cx)),
                                    )
                                    .child(
                                        h_flex()
                                            .gap(px(6.))
                                            .items_center()
                                            .child(render_action_button(
                                                "send-scan",
                                                paths::REFRESH,
                                                scanning,
                                                animations,
                                                cx,
                                                |this, _window, cx| {
                                                    this.start_discovery_scan(true, cx);
                                                },
                                            ))
                                            .child(render_action_button(
                                                "send-manual",
                                                paths::TARGET,
                                                false,
                                                animations,
                                                cx,
                                                |this, window, cx| {
                                                    if !this.ensure_has_selected_files(window, cx) {
                                                        return;
                                                    }
                                                    this.open_send_target_dialog(window, cx);
                                                },
                                            ))
                                            .child(render_action_button(
                                                "send-favorites",
                                                paths::HEART,
                                                false,
                                                animations,
                                                cx,
                                                |this, window, cx| {
                                                    this.open_favorites_dialog(window, cx);
                                                },
                                            ))
                                            // Send mode button (dropdown)
                                            .child(
                                                Popover::new("send-mode-popover")
                                                    .anchor(Anchor::TopRight)
                                                    .overlay_closable(true)
                                                    .open(app.send_state.show_send_mode_menu)
                                                    .on_open_change({
                                                        let home_entity = home_entity.clone();
                                                        move |open, _window, cx| {
                                                            home_entity.update(cx, |this, _cx| {
                                                                this.send_state.show_send_mode_menu = *open;
                                                            });
                                                        }
                                                    })
                                                    .trigger(
                                                        Button::new("send-mode")
                                                            .ghost()
                                                            .custom(
                                                                ButtonCustomVariant::new(cx)
                                                                    .color(cx.theme().transparent)
                                                                    .foreground(cx.theme().foreground)
                                                                    .hover(cx.theme().transparent)
                                                                    .active(cx.theme().transparent),
                                                            )
                                                            .rounded_full()
                                                            .p(px(0.))
                                                            .child(circle_icon_slot(
                                                                app_icon(
                                                                    paths::SETTINGS,
                                                                    Size::Small,
                                                                    cx.theme().foreground,
                                                                ),
                                                                cx,
                                                            )),
                                                    )
                                                    .content({
                                                        let home_entity = home_entity.clone();
                                                        let current_mode = app.send_state.send_mode;
                                                        move |_state, _window, cx| {
                                                            let home_single = home_entity.clone();
                                                            let home_multiple = home_entity.clone();
                                                            let home_link = home_entity.clone();
                                                            let home_help = home_entity.clone();
                                                            v_flex()
                                                                .w(px(248.))
                                                                .py(px(4.))
                                                                .gap(px(2.))
                                                                .child(
                                                                    div()
                                                                        .id("send-mode-link-inline")
                                                                        .w_full()
                                                                        .h(px(40.))
                                                                        .px(px(10.))
                                                                        .rounded_md()
                                                                        .cursor_pointer()
                                                                        .on_click(move |_event, window, cx| {
                                                                            let _ = home_link.update(cx, |this, cx| {
                                                                                this.send_state.show_send_mode_menu = false;
                                                                                if this.send_state.selected_files.is_empty() {
                                                                                    this.open_simple_notice_dialog(
                                                                                        "请先选择要发送的文件或文本",
                                                                                        window,
                                                                                        cx,
                                                                                    );
                                                                                    return;
                                                                                }
                                                                                this.apply_send_mode_current(super::SendMode::Link);
                                                                                this.navigate_to(routes::SEND_LINK, cx);
                                                                                window.refresh();
                                                                            });
                                                                        })
                                                                        .when(matches!(current_mode, super::SendMode::Link), |this| {
                                                                            this.bg(cx.theme().primary.opacity(0.14))
                                                                        })
                                                                        .child(
                                                                            h_flex()
                                                                                .w_full()
                                                                                .h_full()
                                                                                .justify_between()
                                                                                .items_center()
                                                                                .child(
                                                                                    div()
                                                                                        .text_sm()
                                                                                        .when(matches!(current_mode, super::SendMode::Link), |this| this.font_semibold())
                                                                                        .child("通过分享链接发送"),
                                                                                )
                                                                                .child(if matches!(current_mode, super::SendMode::Link) {
                                                                                    app_icon(paths::CHECK, Size::Small, cx.theme().primary).into_any_element()
                                                                                } else {
                                                                                    div().w(px(16.)).into_any_element()
                                                                                }),
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .id("send-mode-single-inline")
                                                                        .w_full()
                                                                        .h(px(40.))
                                                                        .px(px(10.))
                                                                        .rounded_md()
                                                                        .cursor_pointer()
                                                                        .on_click(move |_event, _window, cx| {
                                                                            let _ = home_single.update(cx, |this, _| {
                                                                                this.apply_send_mode_current(super::SendMode::Single);
                                                                                this.send_state.show_send_mode_menu = false;
                                                                            });
                                                                        })
                                                                        .when(matches!(current_mode, super::SendMode::Single), |this| {
                                                                            this.bg(cx.theme().primary.opacity(0.14))
                                                                        })
                                                                        .child(
                                                                            h_flex()
                                                                                .w_full()
                                                                                .h_full()
                                                                                .justify_between()
                                                                                .items_center()
                                                                                .child(
                                                                                    div()
                                                                                        .text_sm()
                                                                                        .when(matches!(current_mode, super::SendMode::Single), |this| this.font_semibold())
                                                                                        .child("单接收者"),
                                                                                )
                                                                                .child(if matches!(current_mode, super::SendMode::Single) {
                                                                                    app_icon(paths::CHECK, Size::Small, cx.theme().primary).into_any_element()
                                                                                } else {
                                                                                    div().w(px(16.)).into_any_element()
                                                                                }),
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .id("send-mode-multiple-inline")
                                                                        .w_full()
                                                                        .h(px(40.))
                                                                        .px(px(10.))
                                                                        .rounded_md()
                                                                        .cursor_pointer()
                                                                        .on_click(move |_event, _window, cx| {
                                                                            let _ = home_multiple.update(cx, |this, _| {
                                                                                this.apply_send_mode_current(super::SendMode::Multiple);
                                                                                this.send_state.show_send_mode_menu = false;
                                                                            });
                                                                        })
                                                                        .when(matches!(current_mode, super::SendMode::Multiple), |this| {
                                                                            this.bg(cx.theme().primary.opacity(0.14))
                                                                        })
                                                                        .child(
                                                                            h_flex()
                                                                                .w_full()
                                                                                .h_full()
                                                                                .justify_between()
                                                                                .items_center()
                                                                                .child(
                                                                                    div()
                                                                                        .text_sm()
                                                                                        .when(matches!(current_mode, super::SendMode::Multiple), |this| this.font_semibold())
                                                                                        .child("多个接收者"),
                                                                                )
                                                                                .child(if matches!(current_mode, super::SendMode::Multiple) {
                                                                                    app_icon(paths::CHECK, Size::Small, cx.theme().primary).into_any_element()
                                                                                } else {
                                                                                    div().w(px(16.)).into_any_element()
                                                                                }),
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .h(px(1.))
                                                                        .my(px(4.))
                                                                        .bg(cx.theme().border.opacity(0.9)),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .id("send-mode-help-inline")
                                                                        .w_full()
                                                                        .h(px(38.))
                                                                        .px(px(10.))
                                                                        .bg(cx.theme().background.opacity(0.001))
                                                                        .cursor_pointer()
                                                                        .on_click(move |_event, window, cx| {
                                                                            let _ = home_help.update(cx, |this, cx| {
                                                                                this.send_state.show_send_mode_menu = false;
                                                                                this.open_send_mode_help_dialog(window, cx);
                                                                            });
                                                                        })
                                                                        .child(
                                                                            h_flex()
                                                                                .w_full()
                                                                                .h_full()
                                                                                .justify_between()
                                                                                .items_center()
                                                                                .child(
                                                                                    h_flex()
                                                                                        .items_center()
                                                                                        .gap(px(8.))
                                                                                        .child(
                                                                                            app_icon(
                                                                                                paths::INFO,
                                                                                                Size::Small,
                                                                                                cx.theme().muted_foreground,
                                                                                            ),
                                                                                        )
                                                                                        .child(
                                                                                            div()
                                                                                                .text_sm()
                                                                                                .text_color(cx.theme().foreground)
                                                                                                .child("发送模式说明"),
                                                                                        ),
                                                                                )
                                                                                .child(div().w(px(16.))),
                                                                        ),
                                                                )
                                                        }
                                                    }),
                                            ),
                                    )
                            ),
                    )
                    // -- Device list or placeholder --
                    .child(
                        if app.send_state.nearby_devices.is_empty() {
                            div()
                                .px(spacing::PAGE)
                                .pb(px(10.))
                                .child(DevicePlaceholder)
                        } else {
                            v_flex()
                                .gap(px(10.))
                                .w_full()
                                .children(app.send_state.nearby_devices.iter().map(|device| {
                                    let home_entity = home_entity.clone();
                                    let home_for_favorite = home_entity.clone();
                                    let device_for_select = device.clone();
                                    let token = device.token.clone();
                                    let is_favorite = app.send_state.favorite_tokens.contains(&token);
                                    let favorite_device =
                                        app.send_state.favorite_devices.get(&token).cloned();
                                    let endpoint = app.send_state.nearby_endpoints.get(&token);
                                    let protocol_badge = endpoint
                                        .map(|endpoint| {
                                            if endpoint.https {
                                                "LAN • HTTPS".to_string()
                                            } else {
                                                "LAN • HTTP".to_string()
                                            }
                                        })
                                        .unwrap_or_else(|| "WebRTC".to_string());
                                    let ip_suffix_badge = endpoint.and_then(|endpoint| {
                                        endpoint
                                            .ip
                                            .rsplit('.')
                                            .find(|segment| !segment.is_empty())
                                            .map(|segment| format!("#{}", segment))
                                    });
                                    div()
                                        .id(format!("device-row-{}", token))
                                        .px(spacing::PAGE)
                                        .pb(px(8.))
                                        .on_click(cx.listener(move |this, _event, window, cx| {
                                            if this.send_state.suppress_next_nearby_row_click {
                                                this.send_state.suppress_next_nearby_row_click = false;
                                                return;
                                            }
                                            let device = device_for_select.clone();
                                            if !this.ensure_has_selected_files(window, cx) {
                                                return;
                                            }
                                            this.send_state.target_device = Some(device);
                                            if let Some(endpoint) = this
                                                .send_state
                                                .target_device
                                                .as_ref()
                                                .and_then(|d| this.send_state.nearby_endpoints.get(&d.token))
                                                .cloned()
                                            {
                                                this.execute_send(endpoint.ip, endpoint.port, window, cx);
                                            } else {
                                                this.send_state.target_ip = None;
                                                this.open_send_to_address_dialog(window, cx);
                                            }
                                        }))
                                        .child(
                                            {
                                                let mut card = DeviceCard::new(device.clone())
                                                    .is_favorite(is_favorite)
                                                    .protocol_badge(protocol_badge);
                                                if let Some(favorite) = favorite_device.clone() {
                                                    if !favorite.alias.trim().is_empty() {
                                                        card = card.name_override(favorite.alias);
                                                    }
                                                }
                                                if let Some(tag) = ip_suffix_badge {
                                                    card = card.ip_suffix_badge(tag);
                                                }
                                                card
                                            }
                                                .on_favorite_tap({
                                                    let token = token.clone();
                                                    let device_for_favorite = device.clone();
                                                    let endpoint_for_favorite = endpoint.cloned();
                                                    move |_device, window, cx| {
                                                        home_for_favorite.update(cx, |this, cx| {
                                                            this.send_state.suppress_next_nearby_row_click = true;
                                                            if this.send_state.favorite_tokens.contains(&token) {
                                                                let alias_for_delete = favorite_device
                                                                    .as_ref()
                                                                    .map(|item| item.alias.clone())
                                                                    .unwrap_or_else(|| {
                                                                        device_for_favorite.alias
                                                                            .clone()
                                                                    });
                                                                this.open_confirm_remove_favorite_dialog(
                                                                    token.clone(),
                                                                    alias_for_delete,
                                                                    window,
                                                                    cx,
                                                                );
                                                                return;
                                                            }
                                                            let Some(endpoint) = endpoint_for_favorite.clone() else {
                                                                this.open_simple_notice_dialog(
                                                                    "当前设备地址不可用，暂时无法添加到收藏夹。",
                                                                    window,
                                                                    cx,
                                                                );
                                                                return;
                                                            };
                                                            this.open_edit_favorite_dialog(
                                                                Some(
                                                                    super::send_state::FavoriteDevice {
                                                                        token: token.clone(),
                                                                        alias: device_for_favorite.alias.clone(),
                                                                        ip: endpoint.ip,
                                                                        port: endpoint.port,
                                                                        https: endpoint.https,
                                                                        custom_alias: false,
                                                                    },
                                                                ),
                                                                window,
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                })
                                        )
                                }))
                        },
                    )
                    // -- Troubleshoot button --
                    .child(
                        div()
                            .w_full()
                            .py(px(10.))
                            .flex()
                            .justify_center()
                            .items_center()
                            .child(
                                Button::new("troubleshoot")
                                    .ghost()
                                    .on_click(cx.listener(|this, _event, window, cx| {
                                        this.open_simple_notice_dialog("请确认目标设备与本机在同一 Wi-Fi 网络。", window, cx);
                                    }))
                                    .child("故障排查"),
                            ),
                    )
                    .child(div().h(px(10.)))
                    // -- OpacitySlideshow hints --
                    .child(
                        div()
                            .px(spacing::PAGE)
                            .child(
                                OpacitySlideshow::new(vec![
                                    "选择文件并选择附近设备即可发送".to_string(),
                                    "请确保两台设备在同一网络中".to_string(),
                                ])
                                .duration_millis(6000)
                                .switch_duration_millis(300)
                                .running(animations),
                            ),
                    )
                    .child(div().h(px(20.))),
                ),
            ),
    )
    .into_any_element()
}
