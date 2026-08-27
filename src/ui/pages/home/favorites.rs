//! Favorite devices management dialogs and actions.

use super::*;
use crate::ui::theme::{radius, spacing};

impl HomePage {
    pub(super) fn open_favorites_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let favorites = self.send_state.favorite_list_sorted();
        let home_entity = cx.entity();
        let dialog_nonce = uuid::Uuid::new_v4().to_string();
        window.open_dialog(cx, move |dialog, _window, cx| {
            let home_for_add = home_entity.clone();
            let mut list = v_flex()
                .w_full()
                .min_w(px(0.))
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border.opacity(0.75))
                .rounded(radius::LG)
                .px(px(10.));

            for (index, favorite) in favorites.iter().enumerate() {
                if index > 0 {
                    list = list.child(
                        div()
                            .h(px(1.))
                            .ml(px(52.))
                            .bg(cx.theme().border.opacity(0.7)),
                    );
                }
                let favorite = favorite.clone();
                let home_for_pick = home_entity.clone();
                let home_for_edit = home_entity.clone();
                let home_for_delete = home_entity.clone();
                let favorite_for_edit = favorite.clone();
                let favorite_for_delete = favorite.clone();
                let row_alias = favorite.alias.clone();
                let row_ip = favorite.ip.clone();
                let row_port = favorite.port;
                let send_ip = row_ip.clone();
                let send_alias = row_alias.clone();
                let send_token = favorite.token.clone();
                let send_https = favorite.https;
                let custom_alias = favorite.custom_alias;
                let id_prefix = format!("favorite-device-{}-{}", favorite.token, dialog_nonce);

                list = list.child(
                    h_flex()
                        .w_full()
                        .min_w(px(0.))
                        .items_center()
                        .gap(px(4.))
                        .child(
                            h_flex()
                                .id(format!("{id_prefix}-send"))
                                .flex_1()
                                .min_w(px(0.))
                                .items_center()
                                .gap(px(12.))
                                .min_h(px(52.))
                                .py(px(8.))
                                .cursor_pointer()
                                .on_click(move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    home_for_pick.update(cx, |this, cx| {
                                        this.send_state.target_device = None;
                                        this.send_to_favorite_device(
                                            send_state::FavoriteDevice {
                                                token: send_token.clone(),
                                                alias: send_alias.clone(),
                                                ip: send_ip.clone(),
                                                port: row_port,
                                                https: send_https,
                                                custom_alias,
                                            },
                                            window,
                                            cx,
                                        );
                                    });
                                })
                                .child(
                                    div()
                                        .w(px(40.))
                                        .h(px(40.))
                                        .rounded(radius::MD)
                                        .bg(cx.theme().muted)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .flex_none()
                                        .child(app_icon(
                                            paths::SMARTPHONE,
                                            Size::Small,
                                            cx.theme().primary,
                                        )),
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
                                                .text_sm()
                                                .font_semibold()
                                                .text_color(cx.theme().foreground)
                                                .child(row_alias.clone()),
                                        )
                                        .child(
                                            div()
                                                .w_full()
                                                .overflow_hidden()
                                                .truncate()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("{row_ip}:{row_port}")),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .id(format!("{id_prefix}-edit"))
                                .w(px(36.))
                                .h(px(36.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .flex_none()
                                .cursor_pointer()
                                .on_click(move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    let preset = favorite_for_edit.clone();
                                    home_for_edit.update(cx, |this, cx| {
                                        this.open_edit_favorite_dialog(Some(preset), window, cx);
                                    });
                                })
                                .child(app_icon(
                                    paths::PENCIL,
                                    Size::Small,
                                    cx.theme().muted_foreground,
                                )),
                        )
                        .child(
                            div()
                                .id(format!("{id_prefix}-delete"))
                                .w(px(36.))
                                .h(px(36.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .flex_none()
                                .cursor_pointer()
                                .on_click(move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    let token = favorite_for_delete.token.clone();
                                    let alias = favorite_for_delete.alias.clone();
                                    home_for_delete.update(cx, |this, cx| {
                                        this.open_confirm_remove_favorite_dialog(
                                            token, alias, window, cx,
                                        );
                                    });
                                })
                                .child(app_icon(paths::TRASH, Size::Small, cx.theme().danger)),
                        ),
                );
            }

            dialog
                .title(dialog_title("收藏夹"))
                .overlay(true)
                .w(px(360.))
                .child(
                    v_flex()
                        .w_full()
                        .min_w(px(0.))
                        .gap(px(12.))
                        .child(if favorites.is_empty() {
                            v_flex()
                                .w_full()
                                .items_center()
                                .gap(spacing::SM)
                                .py(px(20.))
                                .child(
                                    div()
                                        .w(px(48.))
                                        .h(px(48.))
                                        .rounded_full()
                                        .bg(cx.theme().muted)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(app_icon(
                                            paths::HEART,
                                            Size::Small,
                                            cx.theme().muted_foreground,
                                        )),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("暂无收藏设备"),
                                )
                                .into_any_element()
                        } else {
                            list.into_any_element()
                        })
                        .child(
                            Button::new("favorites-add-manual")
                                .primary()
                                .w_full()
                                .h(px(44.))
                                .on_click(move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    home_for_add.update(cx, |this, cx| {
                                        this.open_edit_favorite_dialog(None, window, cx);
                                    });
                                })
                                .child("添加设备"),
                        ),
                )
        });
    }

    pub(super) fn open_edit_favorite_dialog(
        &mut self,
        preset: Option<send_state::FavoriteDevice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let alias_input = cx.new(|cx| InputState::new(window, cx).placeholder("设备名称（可选）"));
        let ip_input = cx.new(|cx| InputState::new(window, cx).placeholder("IP 地址"));
        let port_input = cx.new(|cx| InputState::new(window, cx).placeholder("端口（默认 53317）"));
        if let Some(ref item) = preset {
            alias_input.update(cx, |state, cx| {
                state.set_value(item.alias.clone(), window, cx);
            });
            ip_input.update(cx, |state, cx| {
                state.set_value(item.ip.clone(), window, cx);
            });
            port_input.update(cx, |state, cx| {
                state.set_value(item.port.to_string(), window, cx);
            });
        } else {
            port_input.update(cx, |state, cx| {
                state.set_value("53317", window, cx);
            });
        }
        let home_entity = cx.entity();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let alias_for_ok = alias_input.clone();
            let ip_for_ok = ip_input.clone();
            let port_for_ok = port_input.clone();
            let home_for_ok = home_entity.clone();
            let preset_for_ok = preset.clone();
            let original_alias = preset
                .as_ref()
                .map(|item| item.alias.clone())
                .unwrap_or_default();
            let original_custom_alias = preset
                .as_ref()
                .map(|item| item.custom_alias)
                .unwrap_or(false);
            dialog
                .title(dialog_title(if preset.is_some() {
                    "编辑收藏设备"
                } else {
                    "添加收藏设备"
                }))
                .overlay(true)
                .w(px(360.))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(10.))
                        .child(Input::new(&alias_input).appearance(true))
                        .child(Input::new(&ip_input).appearance(true))
                        .child(Input::new(&port_input).appearance(true)),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("下一步")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "edit-favorite",
                    "下一步",
                    "取消",
                ))
                .on_ok(move |_event, window, cx| {
                    let alias = alias_for_ok.read(cx).value().trim().to_string();
                    let ip = ip_for_ok.read(cx).value().trim().to_string();
                    let raw_port = port_for_ok.read(cx).value().trim().to_string();
                    if ip.is_empty() {
                        home_for_ok.update(cx, |this, cx| {
                            this.open_simple_notice_dialog("IP 地址不能为空", window, cx);
                        });
                        return false;
                    }
                    let Ok(port) = raw_port.parse::<u16>() else {
                        home_for_ok.update(cx, |this, cx| {
                            this.open_simple_notice_dialog("端口必须是 1-65535 的数字", window, cx);
                        });
                        return false;
                    };
                    if port == 0 {
                        home_for_ok.update(cx, |this, cx| {
                            this.open_simple_notice_dialog("端口必须是 1-65535 的数字", window, cx);
                        });
                        return false;
                    }
                    let token = if let Some(item) = &preset_for_ok {
                        item.token.clone()
                    } else {
                        format!("manual:{}:{}", ip, port)
                    };
                    let display_alias = if alias.is_empty() {
                        ip.clone()
                    } else {
                        alias.clone()
                    };
                    let https = preset_for_ok
                        .as_ref()
                        .map(|item| item.https)
                        .unwrap_or(false);
                    let custom_alias = if alias.is_empty() {
                        false
                    } else if original_alias.is_empty() {
                        true
                    } else {
                        original_custom_alias || alias != original_alias
                    };
                    window.close_dialog(cx);
                    home_for_ok.update(cx, |this, cx| {
                        this.open_confirm_add_favorite_dialog(
                            token,
                            display_alias,
                            ip,
                            port,
                            https,
                            custom_alias,
                            window,
                            cx,
                        );
                    });
                    false
                })
        });
    }

    fn send_to_favorite_device(
        &mut self,
        favorite: send_state::FavoriteDevice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let home_entity = cx.entity();
        let window_handle = window.window_handle();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();
        let self_fingerprint = self
            .app_state
            .read(cx)
            .client_info
            .as_ref()
            .map(|v| v.token.clone());
        let favorite_for_probe = favorite.clone();
        let join = tokio_handle.spawn(async move {
            crate::core::discovery::probe_device(
                &favorite_for_probe.ip,
                favorite_for_probe.port,
                favorite_for_probe.https,
                self_fingerprint,
            )
            .await
        });
        cx.spawn(async move |_this, cx| {
            let probe_result = match join.await {
                Ok(item) => item,
                Err(err) => {
                    log::warn!("favorite probe task failed: {}", err);
                    None
                }
            };
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = home_entity.update(cx, |this, cx| {
                    if probe_result.is_none() {
                        this.open_simple_notice_dialog(
                            "收藏设备暂不可达，请确认设备在线并在同一网络。",
                            window,
                            cx,
                        );
                        return;
                    }
                    this.execute_send(favorite.ip.clone(), favorite.port, window, cx);
                });
            });
        })
        .detach();
    }

    pub(super) fn open_confirm_remove_favorite_dialog(
        &mut self,
        token: String,
        alias: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let home_entity = cx.entity();
        let display_name = if alias.trim().is_empty() {
            "该设备".to_string()
        } else {
            alias
        };
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let home_for_ok = home_entity.clone();
            let token_for_ok = token.clone();
            dialog
                .title(dialog_title("删除收藏"))
                .overlay(true)
                .w(px(340.))
                .child(
                    div()
                        .w_full()
                        .overflow_hidden()
                        .truncate()
                        .text_sm()
                        .child(format!("确认将 \"{}\" 从收藏夹移除吗？", display_name)),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("删除")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "remove-favorite",
                    "删除",
                    "取消",
                ))
                .on_ok(move |_event, _window, cx| {
                    home_for_ok.update(cx, |this, _cx| {
                        this.send_state.remove_favorite_device(&token_for_ok);
                    });
                    true
                })
        });
    }

    pub(super) fn open_confirm_add_favorite_dialog(
        &mut self,
        token: String,
        alias: String,
        ip: String,
        port: u16,
        https: bool,
        custom_alias: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let home_entity = cx.entity();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let home_for_ok = home_entity.clone();
            let alias_text = if alias.trim().is_empty() {
                ip.clone()
            } else {
                alias.clone()
            };
            let token_for_ok = token.clone();
            let alias_for_ok = alias_text.clone();
            let ip_for_ok = ip.clone();
            dialog
                .title(dialog_title("确认添加收藏"))
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
                                .child(format!("设备名：{}", alias_text)),
                        )
                        .child(
                            div()
                                .w_full()
                                .overflow_hidden()
                                .truncate()
                                .text_sm()
                                .child(format!("地址：{}:{}", ip, port)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(_cx.theme().muted_foreground)
                                .child("请确认设备信息后再添加。"),
                        ),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("添加")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "add-favorite",
                    "添加",
                    "取消",
                ))
                .on_ok(move |_event, _window, cx| {
                    home_for_ok.update(cx, |this, _cx| {
                        this.send_state.add_or_update_favorite_device(
                            token_for_ok.clone(),
                            alias_for_ok.clone(),
                            ip_for_ok.clone(),
                            port,
                            https,
                            custom_alias,
                        );
                    });
                    true
                })
        });
    }
}
