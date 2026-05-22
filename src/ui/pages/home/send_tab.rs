//! Send tab: select content type, files, and nearby devices (LocalSend-aligned layout).

use super::HomePage;
use crate::ui::components::{
    device_card::DeviceCard, device_placeholder::DevicePlaceholder,
    opacity_slideshow::OpacitySlideshow,
};
use crate::ui::routes;
use crate::ui::theme::spacing;
use crate::ui::utils::format_file_size;
use gpui::{
    div, percentage, prelude::*, px, Animation, AnimationExt as _, AnyElement, Context,
    ScrollHandle, Transformation, Window,
};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    h_flex,
    popover::Popover,
    v_flex, ActiveTheme as _, Anchor, Icon, Sizable as _, Size, StyledExt as _,
};
use std::time::Duration;

/// Button width/height for content type buttons (matches BigButton constants).
const CONTENT_BTN_WIDTH: f32 = 90.0;
const CONTENT_BTN_HEIGHT: f32 = 65.0;
const CONTENT_BTN_GAP: f32 = 10.0;
const CONTENT_BTN_COUNT: f32 = 4.0;
const CONTENT_ROW_MIN_WIDTH: f32 =
    CONTENT_BTN_WIDTH * CONTENT_BTN_COUNT + CONTENT_BTN_GAP * (CONTENT_BTN_COUNT - 1.0) + 2.0;

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
    let bg = cx.theme().foreground.opacity(0.08);
    let hover_bg = cx.theme().foreground.opacity(0.10);
    let active_bg = cx.theme().foreground.opacity(0.12);
    let fg = cx.theme().foreground;

    Button::new(id)
        .flex_none()
        .custom(
            ButtonCustomVariant::new(cx)
                .color(bg)
                .foreground(fg)
                .hover(hover_bg)
                .active(active_bg),
        )
        .w(px(CONTENT_BTN_WIDTH))
        .h(px(CONTENT_BTN_HEIGHT))
        .rounded_md()
        .on_click(cx.listener(move |this, _event, window, cx| {
            on_click(this, window, cx);
        }))
        .child(
            v_flex()
                .items_center()
                .justify_between()
                .gap(px(4.))
                .child(
                    Icon::default()
                        .path(icon_path.clone())
                        .with_size(gpui_component::Size::Medium)
                        .text_color(fg),
                )
                .child(
                    div()
                        .text_sm()
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
    let icon = Icon::default()
        .path(icon_path.clone())
        .with_size(Size::Small);

    let icon_element = if spinning && animations {
        icon.with_animation(
            "send-action-refresh-spin",
            Animation::new(Duration::from_millis(900)).repeat(),
            |this, delta| this.transform(Transformation::rotate(percentage(delta))),
        )
        .into_any_element()
    } else {
        icon.into_any_element()
    };

    div()
        .id(id)
        .cursor_default()
        .rounded_full()
        .p(px(4.))
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
                .w(px(38.))
                .h(px(38.))
                .flex()
                .items_center()
                .justify_center()
                .child(icon_element),
        )
        .on_click(cx.listener(move |this, _event, window, cx| {
            on_click(this, window, cx);
        }))
        .into_any_element()
}

pub fn render_send_content(
    app: &mut HomePage,
    window: &mut Window,
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
    let select_row_scroll = window
        .use_keyed_state("send-select-row-scroll", cx, |_, _| ScrollHandle::default())
        .read(cx)
        .clone();

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
                    // -- Section title: "选择" --
                    .child(
                        div()
                            .px(px(15.))
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::BLACK)
                                    .text_color(cx.theme().foreground)
                                    .child("选择"),
                            ),
                    )
                    // -- Content type buttons row --
                    .child(
                        div()
                            .px(px(15.))
                            .child(
                                div()
                                    .id("send-content-type-scroll")
                                    .w_full()
                                    .track_scroll(&select_row_scroll)
                                    .overflow_x_scroll()
                                    .child(
                                        h_flex()
                                            .min_w(px(CONTENT_ROW_MIN_WIDTH))
                                            .gap(px(CONTENT_BTN_GAP))
                                            .items_center()
                                            .child(render_content_type_button(
                                                "content-file",
                                                "icons/file.svg",
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
                                                "icons/folder.svg",
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
                                                "icons/book-open.svg",
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
                                                "icons/copy.svg",
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
                            ),
                    )
                    // -- Selected files card --
                    .when(has_files, |this| {
                        let file_count = selected_files.len();
                        this.child(
                            div()
                                .mx(px(15.))
                                .mb(px(10.))
                                .bg(cx.theme().secondary)
                                .border_1()
                                .border_color(cx.theme().border)
                                .rounded_lg()
                                .pl(px(15.))
                                .pt(px(5.))
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
                                                        .child(
                                                            Icon::default()
                                                                .path("icons/x.svg")
                                                                .with_size(gpui_component::Size::Small),
                                                        ),
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
                                                                "icons/book-open.svg"
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
                                                                    "icons/image.svg"
                                                                } else {
                                                                    "icons/file.svg"
                                                                }
                                                            };
                                                            div()
                                                                .child(
                                                                    div()
                                                                        .w(px(56.))
                                                                        .h(px(56.))
                                                                        .bg(cx.theme().primary.opacity(0.18))
                                                                        .rounded_md()
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .child(
                                                                            Icon::default()
                                                                                .path(icon_path)
                                                                                .with_size(gpui_component::Size::Medium),
                                                                        ),
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
                                                                .child(
                                                                    Icon::default()
                                                                        .path("icons/plus.svg")
                                                                        .with_size(gpui_component::Size::Small),
                                                                )
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
                            .px(px(15.))
                            .pt(px(5.))
                            .child(
                                h_flex()
                                    .gap(px(16.))
                                    .items_center()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(gpui::FontWeight::BLACK)
                                            .text_color(cx.theme().foreground)
                                            .child("附近的设备"),
                                    )
                                    .child(
                                        h_flex()
                                            .gap(spacing::SM)
                                            .items_center()
                                            // Scan button
                                            .child(render_action_button(
                                                "send-scan",
                                                "icons/refresh.svg",
                                                scanning,
                                                animations,
                                                cx,
                                                |this, _window, cx| {
                                                    this.start_discovery_scan(true, cx);
                                                },
                                            ))
                                            // Manual address button
                                            .child(render_action_button(
                                                "send-manual",
                                                "icons/target.svg",
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
                                            // Favorites button
                                            .child(render_action_button(
                                                "send-favorites",
                                                "icons/heart.svg",
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
                                                            .p(px(4.))
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
                                                                    .w(px(38.))
                                                                    .h(px(38.))
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .child(
                                                                        Icon::default()
                                                                            .path("icons/settings.svg")
                                                                            .with_size(Size::Small)
                                                                            .text_color(cx.theme().foreground),
                                                                    ),
                                                            ),
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
                                                                                    Icon::default().path("icons/check.svg").with_size(Size::Small).into_any_element()
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
                                                                                    Icon::default().path("icons/check.svg").with_size(Size::Small).into_any_element()
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
                                                                                    Icon::default().path("icons/check.svg").with_size(Size::Small).into_any_element()
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
                                                                                            Icon::default()
                                                                                                .path("icons/info.svg")
                                                                                                .with_size(Size::Small)
                                                                                                .text_color(cx.theme().muted_foreground),
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
                                .px(px(15.))
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
                                        .px(px(15.))
                                        .pb(px(10.))
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
                            .px(px(15.))
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
