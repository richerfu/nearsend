//! Settings tab: general, receive, send, network, other (uses ui/pages state types).

use super::HomePage;
use crate::ui::components::{logo::Logo, switch::Switch};
use crate::ui::icons::{app_icon, paths};
use crate::ui::routes;
use crate::ui::theme::{radius, spacing};
use gpui::{
    div, percentage, prelude::*, px, Animation, AnimationExt as _, AnyElement, Context, Entity,
    Transformation, Window,
};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    select::{Select, SelectState},
    v_flex, ActiveTheme as _, Sizable as _, Size, StyledExt as _,
};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Reusable helpers
// ---------------------------------------------------------------------------

/// Renders a settings section card with a title and a list of child entries.
fn render_settings_section(
    title: &str,
    cx: &mut Context<HomePage>,
    children: Vec<AnyElement>,
) -> AnyElement {
    let mut inner = v_flex().w_full();
    for (index, child) in children.into_iter().enumerate() {
        if index > 0 {
            inner = inner.child(
                div()
                    .h(px(1.))
                    .ml(px(4.))
                    .bg(cx.theme().border.opacity(0.7)),
            );
        }
        inner = inner.child(child);
    }
    v_flex()
        .w_full()
        .gap(px(8.))
        .child(
            div()
                .px(px(4.))
                .text_sm()
                .font_semibold()
                .text_color(cx.theme().muted_foreground)
                .child(title.to_string()),
        )
        .child(
            div()
                .w_full()
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border.opacity(0.75))
                .rounded(radius::LG)
                .px(px(14.))
                .py(px(4.))
                .child(inner),
        )
        .into_any_element()
}

/// Renders a select entry (label + in-place dropdown in a 150px container).
fn render_select_entry(
    label: &str,
    select_state: &Entity<SelectState<Vec<&'static str>>>,
    id: &str,
    cx: &mut Context<HomePage>,
) -> AnyElement {
    div()
        .py(px(10.))
        .child(
            h_flex()
                .items_center()
                .min_h(px(36.))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .flex_1()
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .id(id.to_string())
                        .w(px(150.))
                        .child(Select::new(select_state).w_full().with_size(Size::Medium)),
                ),
        )
        .into_any_element()
}

/// Renders a boolean toggle entry (label + switch).
fn render_boolean_entry(
    label: &str,
    value: bool,
    id: &str,
    cx: &mut Context<HomePage>,
    on_toggle: impl Fn(&mut HomePage, &mut Context<HomePage>) + 'static,
) -> AnyElement {
    div()
        .py(px(10.))
        .child(
            h_flex()
                .items_center()
                .min_h(px(36.))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .flex_1()
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .id(id.to_string())
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _ev, _win, cx| {
                            on_toggle(this, cx);
                        }))
                        .child(Switch::new(value)),
                ),
        )
        .into_any_element()
}

/// Renders a button entry whose click handler needs `window` and `cx`.
fn render_clickable_entry(
    label: &str,
    button_text: &str,
    id: &str,
    cx: &mut Context<HomePage>,
    on_click: impl Fn(&mut HomePage, &mut Window, &mut Context<HomePage>) + 'static,
) -> AnyElement {
    div()
        .py(px(10.))
        .child(
            h_flex()
                .items_center()
                .min_h(px(36.))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .flex_1()
                        .child(label.to_string()),
                )
                .child(div().w(px(10.)))
                .child(
                    div().w(px(150.)).child(
                        Button::new(id.to_string())
                            .with_variant(gpui_component::button::ButtonVariant::Secondary)
                            .outline()
                            .w_full()
                            .on_click(cx.listener(move |this, _ev, window, cx| {
                                on_click(this, window, cx);
                            }))
                            .child(
                                div()
                                    .w_full()
                                    .overflow_hidden()
                                    .truncate()
                                    .child(button_text.to_string()),
                            ),
                    ),
                ),
        )
        .into_any_element()
}

fn render_refresh_icon(spinning: bool, animations: bool, cx: &mut Context<HomePage>) -> AnyElement {
    let refresh_icon = app_icon(paths::REFRESH, Size::Small, cx.theme().foreground);

    if spinning && animations {
        refresh_icon.with_animation(
            "settings-server-refresh-spin",
            Animation::new(Duration::from_millis(900)).repeat(),
            |this, delta| this.transform(Transformation::rotate(percentage(delta))),
        )
        .into_any_element()
    } else {
        refresh_icon.into_any_element()
    }
}

// ---------------------------------------------------------------------------
// Main render
// ---------------------------------------------------------------------------

pub fn render_settings_content(
    app: &mut HomePage,
    _window: &mut Window,
    cx: &mut Context<HomePage>,
) -> AnyElement {
    let Some(send_mode_default_select) = app.send_mode_default_select.clone() else {
        return div()
            .size_full()
            .bg(cx.theme().background)
            .into_any_element();
    };
    let Some(device_type_select) = app.device_type_select.clone() else {
        return div()
            .size_full()
            .bg(cx.theme().background)
            .into_any_element();
    };
    let Some(device_model_select) = app.device_model_select.clone() else {
        return div()
            .size_full()
            .bg(cx.theme().background)
            .into_any_element();
    };
    let Some(network_filter_mode_select) = app.network_filter_mode_select.clone() else {
        return div()
            .size_full()
            .bg(cx.theme().background)
            .into_any_element();
    };

    let advanced = app.settings_state.advanced;
    let animations = app.settings_state.animations;
    let server_running = app.settings_state.server_running;
    let server_paused = app.settings_state.server_paused;
    let server_refreshing = app.server_refreshing;
    let server_alias = app.settings_state.server_alias.clone();
    let server_port = app.settings_state.server_port;
    let share_link_auto_accept = app.settings_state.share_via_link_auto_accept;
    let quick_save = app.settings_state.quick_save;
    let quick_save_favorites = app.settings_state.quick_save_favorites;
    let auto_finish = app.settings_state.auto_finish;
    let save_to_history = app.settings_state.save_to_history;

    // -- Receive section --
    let require_pin = app.settings_state.require_pin;
    let masked_pin = if app.settings_state.receive_pin.is_empty() {
        "未设置".to_string()
    } else {
        "*".repeat(app.settings_state.receive_pin.chars().count().min(12))
    };
    let r1 = render_boolean_entry(
        "接收需要 PIN",
        require_pin,
        "toggle-require-pin",
        cx,
        |this, cx| {
            this.settings_state.require_pin = !this.settings_state.require_pin;
            this.sync_server_config_to_runtime(cx);
            this.persist_settings();
        },
    );
    let r2 = render_clickable_entry(
        "接收 PIN",
        &masked_pin,
        "receive-pin-input",
        cx,
        |this, window, cx| {
            this.open_receive_pin_dialog(window, cx);
        },
    );
    let quick_save_entry = render_boolean_entry(
        "自动接受(全部)",
        quick_save,
        "toggle-quick-save",
        cx,
        |this, _cx| {
            this.settings_state.quick_save = !this.settings_state.quick_save;
            if this.settings_state.quick_save {
                this.settings_state.quick_save_favorites = false;
                this.receive_state.quick_save_mode = super::QuickSaveMode::On;
            } else if !this.settings_state.quick_save_favorites {
                this.receive_state.quick_save_mode = super::QuickSaveMode::Off;
            }
            this.persist_settings();
        },
    );
    let quick_save_fav_entry = render_boolean_entry(
        "自动接受(仅收藏夹)",
        quick_save_favorites,
        "toggle-quick-save-favorites",
        cx,
        |this, _cx| {
            this.settings_state.quick_save_favorites = !this.settings_state.quick_save_favorites;
            if this.settings_state.quick_save_favorites {
                this.settings_state.quick_save = false;
                this.receive_state.quick_save_mode = super::QuickSaveMode::Favorites;
            } else if !this.settings_state.quick_save {
                this.receive_state.quick_save_mode = super::QuickSaveMode::Off;
            }
            this.persist_settings();
        },
    );
    let auto_finish_entry = render_boolean_entry(
        "自动完成",
        auto_finish,
        "toggle-auto-finish",
        cx,
        |this, _cx| {
            this.settings_state.auto_finish = !this.settings_state.auto_finish;
            this.persist_settings();
        },
    );
    let save_to_history_entry = render_boolean_entry(
        "保存到历史",
        save_to_history,
        "toggle-save-to-history",
        cx,
        |this, _cx| {
            this.settings_state.save_to_history = !this.settings_state.save_to_history;
            this.persist_settings();
        },
    );
    let mut receive_children: Vec<AnyElement> = vec![
        r1,
        quick_save_entry,
        quick_save_fav_entry,
        auto_finish_entry,
        save_to_history_entry,
    ];
    if require_pin {
        receive_children.push(r2);
    }
    let receive = render_settings_section("接收", cx, receive_children);

    // -- Send section (align with LocalSend advanced settings) --
    let send_mode = render_select_entry(
        "默认发送模式",
        &send_mode_default_select,
        "select-send-mode-default",
        cx,
    );
    let share_link = render_boolean_entry(
        "分享链接自动接受",
        share_link_auto_accept,
        "toggle-share-link-auto-accept",
        cx,
        |this, _cx| {
            this.settings_state.share_via_link_auto_accept =
                !this.settings_state.share_via_link_auto_accept;
            this.persist_settings();
        },
    );
    let send = render_settings_section("发送", cx, vec![send_mode, share_link]);

    // -- Network section --
    let server_label_text = format!("服务器{}", if server_running { "" } else { " (离线)" });
    let can_pause = server_running && !server_paused;
    let server_controls = div()
        .py(px(10.))
        .child(
            h_flex()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .flex_1()
                        .child(server_label_text.clone()),
                )
                .child(div().w(px(10.)))
                .child(
                    div().w(px(150.)).child(
                        div().bg(cx.theme().muted).rounded_md().child(
                            h_flex()
                                .justify_center()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .id("server-start")
                                        .cursor_pointer()
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded_md()
                                        .on_click(cx.listener(|this, _ev, _win, cx| {
                                            this.trigger_server_refresh_feedback(cx);
                                            if this.settings_state.server_paused {
                                                this.resume_local_server(cx);
                                            } else if this.settings_state.server_running {
                                                this.restart_local_server_with_current_config(cx);
                                            } else {
                                                this.start_local_server(cx);
                                            }
                                        }))
                                        .when(server_paused, |this| {
                                            this.child(app_icon(
                                                paths::PLAY,
                                                Size::Small,
                                                cx.theme().foreground,
                                            ))
                                        })
                                        .when(!server_paused, |this| {
                                            this.child(render_refresh_icon(
                                                server_refreshing,
                                                animations,
                                                cx,
                                            ))
                                        }),
                                )
                                .child(
                                    div()
                                        .id("server-stop")
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded_md()
                                        .when(can_pause, |this| {
                                            this.cursor_pointer().on_click(cx.listener(
                                                |this, _ev, _win, cx| {
                                                    this.pause_local_server(cx);
                                                },
                                            ))
                                        })
                                        .child(app_icon(
                                            paths::PAUSE,
                                            Size::Small,
                                            if can_pause {
                                                cx.theme().foreground
                                            } else {
                                                cx.theme().muted_foreground.opacity(0.35)
                                            },
                                        )),
                                ),
                        ),
                    ),
                ),
        )
        .into_any_element();

    let n1 = render_clickable_entry(
        "别名",
        &server_alias,
        "alias-input",
        cx,
        |this, window, cx| {
            this.open_server_alias_dialog(window, cx);
        },
    );
    let n2 = render_clickable_entry(
        "端口",
        &server_port.to_string(),
        "port-input",
        cx,
        |this, window, cx| {
            this.open_server_port_dialog(window, cx);
        },
    );
    let mut network_children: Vec<AnyElement> = vec![server_controls, n1, n2];
    if advanced {
        let device_type_entry =
            render_select_entry("设备类型", &device_type_select, "select-device-type", cx);
        let device_model_entry =
            render_select_entry("设备型号", &device_model_select, "select-device-model", cx);
        let n3 = render_boolean_entry(
            "加密",
            app.settings_state.encryption,
            "toggle-encryption",
            cx,
            |this, cx| {
                this.settings_state.encryption = !this.settings_state.encryption;
                this.sync_server_config_to_runtime(cx);
                this.restart_local_server_with_current_config(cx);
                this.persist_settings();
            },
        );
        let discovery_timeout_entry = render_clickable_entry(
            "发现超时(ms)",
            &app.settings_state.discovery_timeout.to_string(),
            "discovery-timeout",
            cx,
            |this, window, cx| {
                this.open_discovery_timeout_dialog(window, cx);
            },
        );
        let discovery_target_subnets_label =
            if app.settings_state.discovery_target_subnets.is_empty() {
                "未设置".to_string()
            } else {
                format!("{} 条", app.settings_state.discovery_target_subnets.len())
            };
        let discovery_target_subnets_entry = render_clickable_entry(
            "发现目标网段",
            &discovery_target_subnets_label,
            "discovery-target-subnets",
            cx,
            |this, window, cx| {
                this.open_discovery_target_subnets_dialog(window, cx);
            },
        );
        let multicast_entry = render_clickable_entry(
            "组播地址",
            &app.settings_state.multicast_group,
            "multicast-group",
            cx,
            |this, window, cx| {
                this.open_multicast_group_dialog(window, cx);
            },
        );
        let n4 = render_select_entry(
            "网络接口模式",
            &network_filter_mode_select,
            "select-network-mode",
            cx,
        );
        let n5 = render_clickable_entry(
            "网络接口规则",
            "编辑",
            "network-rules",
            cx,
            |this, window, cx| {
                this.open_network_filters_dialog(window, cx);
            },
        );
        network_children.push(device_type_entry);
        network_children.push(device_model_entry);
        network_children.push(n3);
        network_children.push(discovery_timeout_entry);
        network_children.push(discovery_target_subnets_entry);
        network_children.push(multicast_entry);
        network_children.push(n4);
        network_children.push(n5);
    }
    let network = render_settings_section("网络", cx, network_children);

    // -- Other section children --
    let o1 = render_clickable_entry("关于", "打开", "about", cx, |this, _window, cx| {
        this.navigate_to(routes::SETTINGS_ABOUT, cx);
    });
    let o2 = render_clickable_entry("支持", "捐赠", "donate", cx, |this, _window, cx| {
        this.navigate_to(routes::SETTINGS_DONATE, cx);
    });
    let o3 = render_clickable_entry(
        "开源协议",
        "打开",
        "open-source-licenses",
        cx,
        |this, _window, cx| {
            this.navigate_to(routes::SETTINGS_OPEN_SOURCE_LICENSES, cx);
        },
    );
    let other = render_settings_section("其他", cx, vec![o1, o2, o3]);

    // -- Advanced Settings toggle --
    let advanced_toggle = h_flex()
        .justify_end()
        .w_full()
        .child(
            h_flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child("高级设置"),
                )
                .child(
                    div()
                        .id("toggle-advanced-settings")
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _ev, _win, _cx| {
                            this.settings_state.advanced = !this.settings_state.advanced;
                            this.persist_settings();
                        }))
                        .child(
                            div()
                                .w(px(18.))
                                .h(px(18.))
                                .rounded(px(4.))
                                .border_1()
                                .border_color(if advanced {
                                    cx.theme().primary
                                } else {
                                    cx.theme().border
                                })
                                .bg(if advanced {
                                    cx.theme().primary
                                } else {
                                    cx.theme().background
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(advanced, |this| {
                                    this.child(
                                        app_icon(
                                            paths::CHECK,
                                            Size::XSmall,
                                            cx.theme().primary_foreground,
                                        ),
                                    )
                                }),
                        ),
                ),
        )
        .into_any_element();

    // -- About section --
    let about = v_flex()
        .gap(px(5.))
        .items_center()
        .child(Logo::new().size(80.).with_text(true))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .text_center()
                .child("Version 0.1.0"),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .text_center()
                .child("\u{00a9} 2025 NearSend"),
        )
        .child(
            Button::new("changelog")
                .ghost()
                .on_click(cx.listener(|this, _ev, _win, cx| {
                    this.navigate_to(routes::SETTINGS_CHANGELOG, cx);
                }))
                .child("更新日志"),
        )
        .into_any_element();

    // -- Assemble page --
    let mut content = v_flex()
        .w_full()
        .px(spacing::PAGE)
        .pt(px(12.))
        .pb(px(40.))
        .gap(spacing::LG);

    content = content
        .child(receive)
        .when(advanced, |this| this.child(send))
        .child(network)
        .child(other)
        .child(advanced_toggle)
        .child(about)
        .child(div().h(px(80.)));

    div()
        .size_full()
        .w_full()
        .bg(cx.theme().muted.opacity(0.45))
        .overflow_y_scrollbar()
        .child(content)
        .into_any_element()
}
