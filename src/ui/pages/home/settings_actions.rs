//! Settings interactions and address-target dialogs.

use super::*;
use crate::ui::theme::radius;
use gpui_component::radio::{Radio, RadioGroup};
use gpui_component::scroll::ScrollableElement as _;
use std::rc::Rc;

impl HomePage {
    /// Opens a dialog with a multiline text input for sending text messages.
    /// Matches LocalSend's MessageInputDialog behavior.
    pub(super) fn open_text_input_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input_state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 5)
                .placeholder("输入文本内容")
                .soft_wrap(true)
        });
        self.text_input_state = Some(input_state.clone());

        let home_entity = cx.entity();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_ok = input_state.clone();
            let home_for_ok = home_entity.clone();

            dialog
                .title(dialog_title("发送文本"))
                .overlay(true)
                .w(px(340.))
                .child(
                    div()
                        .w_full()
                        .child(Textarea::new(&input_state).appearance(true)),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("确认")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "text-input",
                    "确认",
                    "取消",
                ))
                .on_ok(move |_event, _window, cx| {
                    let text = input_for_ok.read(cx).value().to_string();
                    if !text.is_empty() {
                        home_for_ok.update(cx, |this, cx| {
                            this.send_selection_state.update(cx, |state, state_cx| {
                                state.add_text(text.clone());
                                state_cx.notify();
                            });
                            this.sync_selected_files_from_shared(cx);
                            cx.notify();
                        });
                    }
                    true
                })
        });
    }

    pub(super) fn open_send_pin_dialog(
        &mut self,
        show_invalid_pin: bool,
        responder: oneshot::Sender<Option<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("输入接收端 PIN"));
        let responder = Arc::new(Mutex::new(Some(responder)));
        let home_entity = cx.entity();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_ok = input_state.clone();
            let responder_for_ok = responder.clone();
            let responder_for_cancel = responder.clone();
            let home_for_ok = home_entity.clone();
            let variant = ButtonCustomVariant::new(_cx)
                .color(_cx.theme().secondary)
                .foreground(_cx.theme().foreground)
                .hover(_cx.theme().secondary)
                .active(_cx.theme().secondary);
            dialog
                .title(dialog_title(if show_invalid_pin {
                    "PIN 错误，请重试"
                } else {
                    "请输入接收端 PIN"
                }))
                .overlay(true)
                .w(px(320.))
                .child(
                    div()
                        .w_full()
                        .child(Input::new(&input_state).appearance(true).large()),
                )
                .child(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            Button::new("send-pin-cancel")
                                .custom(variant.clone())
                                .on_click(move |_event, window, cx| {
                                    if let Ok(mut guard) = responder_for_cancel.lock() {
                                        if let Some(tx) = guard.take() {
                                            let _ = tx.send(None);
                                        }
                                    }
                                    window.close_dialog(cx);
                                })
                                .child("取消"),
                        )
                        .child(
                            Button::new("send-pin-confirm")
                                .custom(variant)
                                .on_click(move |_event, window, cx| {
                                    let pin = input_for_ok.read(cx).value().trim().to_string();
                                    if pin.is_empty() {
                                        home_for_ok.update(cx, |this, cx| {
                                            this.open_simple_notice_dialog(
                                                "PIN 不能为空",
                                                window,
                                                cx,
                                            );
                                        });
                                        return;
                                    }
                                    if let Ok(mut guard) = responder_for_ok.lock() {
                                        if let Some(tx) = guard.take() {
                                            let _ = tx.send(Some(pin));
                                        }
                                    }
                                    window.close_dialog(cx);
                                })
                                .child("确认"),
                        ),
                )
        });
    }

    pub(super) fn open_server_alias_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("输入设备别名"));
        let current_alias = self.settings_state.server_alias.clone();
        input_state.update(cx, |state, cx| {
            state.set_value(current_alias, window, cx);
        });
        let home_entity = cx.entity();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_ok = input_state.clone();
            let home_for_ok = home_entity.clone();

            dialog
                .title(dialog_title("编辑别名"))
                .overlay(true)
                .w(px(340.))
                .child(
                    div()
                        .w_full()
                        .child(Input::new(&input_state).appearance(true).large()),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("保存")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "server-alias",
                    "保存",
                    "取消",
                ))
                .on_ok(move |_event, window, cx| {
                    let alias = input_for_ok.read(cx).value().trim().to_string();
                    if alias.is_empty() {
                        home_for_ok.update(cx, |this, cx| {
                            this.open_simple_notice_dialog("别名不能为空", window, cx);
                        });
                        return false;
                    }
                    home_for_ok.update(cx, |this, cx| {
                        this.settings_state.server_alias = alias.clone();
                        this.sync_server_config_to_runtime(cx);
                        this.persist_settings();
                    });
                    true
                })
        });
    }

    pub(super) fn open_server_port_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("输入端口号"));
        let current_port = self.settings_state.server_port.to_string();
        input_state.update(cx, |state, cx| {
            state.set_value(current_port, window, cx);
        });
        let home_entity = cx.entity();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_ok = input_state.clone();
            let home_for_ok = home_entity.clone();

            dialog
                .title(dialog_title("编辑端口"))
                .overlay(true)
                .w(px(340.))
                .child(
                    div()
                        .w_full()
                        .child(Input::new(&input_state).appearance(true).large()),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("保存")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "server-port",
                    "保存",
                    "取消",
                ))
                .on_ok(move |_event, window, cx| {
                    let raw = input_for_ok.read(cx).value().trim().to_string();
                    let Ok(port) = raw.parse::<u16>() else {
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
                    home_for_ok.update(cx, |this, cx| {
                        this.settings_state.server_port = port;
                        this.sync_server_config_to_runtime(cx);
                        this.persist_settings();
                    });
                    true
                })
        });
    }

    pub(super) fn open_receive_pin_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("输入接收 PIN"));
        let current_pin = self.settings_state.receive_pin.clone();
        input_state.update(cx, |state, cx| {
            state.set_value(current_pin, window, cx);
        });
        let home_entity = cx.entity();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_ok = input_state.clone();
            let home_for_ok = home_entity.clone();

            dialog
                .title(dialog_title("设置接收 PIN"))
                .overlay(true)
                .w(px(340.))
                .child(
                    div()
                        .w_full()
                        .child(Input::new(&input_state).appearance(true).large()),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("保存")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "receive-pin",
                    "保存",
                    "取消",
                ))
                .on_ok(move |_event, window, cx| {
                    let pin = input_for_ok.read(cx).value().trim().to_string();
                    if pin.is_empty() {
                        home_for_ok.update(cx, |this, cx| {
                            this.open_simple_notice_dialog("PIN 不能为空", window, cx);
                        });
                        return false;
                    }
                    home_for_ok.update(cx, |this, cx| {
                        this.settings_state.receive_pin = pin.clone();
                        this.sync_server_config_to_runtime(cx);
                        this.persist_settings();
                    });
                    true
                })
        });
    }

    #[allow(dead_code)]
    pub(super) fn pick_receive_destination(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let window_handle = window.window_handle();
        let home_entity = cx.entity();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();
        let join = tokio_handle
            .spawn(async move { crate::platform::file_picker::pick_save_directory().await });
        cx.spawn(async move |_this, cx| {
            let picked = match join.await {
                Ok(Ok(path)) => path,
                Ok(Err(err)) => {
                    log::error!("pick receive destination failed: {}", err);
                    None
                }
                Err(err) => {
                    log::error!("pick receive destination task failed: {}", err);
                    None
                }
            };
            if let Some(path) = picked {
                let path_text = path.to_string_lossy().to_string();
                let _ = window_handle.update(cx, |_, window, cx| {
                    let _ = home_entity.update(cx, |this, cx| {
                        this.settings_state.destination = Some(path_text);
                        this.sync_server_config_to_runtime(cx);
                        this.persist_settings();
                        cx.notify();
                    });
                    window.refresh();
                });
            } else {
                let _ = window_handle.update(cx, |_, window, cx| {
                    let _ = home_entity.update(cx, |this, cx| {
                        this.open_simple_notice_dialog(
                            "未选择接收目录，保持当前配置。",
                            window,
                            cx,
                        );
                    });
                });
            }
        })
        .detach();
    }

    #[allow(dead_code)]
    pub(super) fn clear_receive_destination(&mut self, cx: &mut Context<Self>) {
        self.settings_state.destination = None;
        self.sync_server_config_to_runtime(cx);
        self.persist_settings();
    }

    pub(super) fn open_discovery_timeout_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("输入发现超时（毫秒）"));
        let current = self.settings_state.discovery_timeout.to_string();
        input_state.update(cx, |state, cx| {
            state.set_value(current, window, cx);
        });
        let home_entity = cx.entity();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_ok = input_state.clone();
            let home_for_ok = home_entity.clone();
            dialog
                .title(dialog_title("发现超时"))
                .overlay(true)
                .w(px(340.))
                .child(
                    div()
                        .w_full()
                        .child(Input::new(&input_state).appearance(true).large()),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("保存")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "discovery-timeout",
                    "保存",
                    "取消",
                ))
                .on_ok(move |_event, window, cx| {
                    let raw = input_for_ok.read(cx).value().trim().to_string();
                    let Ok(value) = raw.parse::<u32>() else {
                        home_for_ok.update(cx, |this, cx| {
                            this.open_simple_notice_dialog("请输入有效数字", window, cx);
                        });
                        return false;
                    };
                    home_for_ok.update(cx, |this, _cx| {
                        this.settings_state.discovery_timeout = value.max(200);
                        this.persist_settings();
                    });
                    true
                })
        });
    }

    pub(super) fn open_multicast_group_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("输入组播地址"));
        let current = self.settings_state.multicast_group.clone();
        input_state.update(cx, |state, cx| {
            state.set_value(current, window, cx);
        });
        let home_entity = cx.entity();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_ok = input_state.clone();
            let home_for_ok = home_entity.clone();
            dialog
                .title(dialog_title("组播地址"))
                .overlay(true)
                .w(px(340.))
                .child(
                    div()
                        .w_full()
                        .child(Input::new(&input_state).appearance(true).large()),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("保存")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "multicast-group",
                    "保存",
                    "取消",
                ))
                .on_ok(move |_event, window, cx| {
                    let raw = input_for_ok.read(cx).value().trim().to_string();
                    if raw.is_empty() {
                        home_for_ok.update(cx, |this, cx| {
                            this.open_simple_notice_dialog("组播地址不能为空", window, cx);
                        });
                        return false;
                    }
                    home_for_ok.update(cx, |this, _cx| {
                        this.settings_state.multicast_group = raw;
                        this.persist_settings();
                    });
                    true
                })
        });
    }

    pub(super) fn open_discovery_target_subnets_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut rows = self.settings_state.discovery_target_subnets.clone();
        if rows.is_empty() {
            rows.push(String::new());
        }
        self.open_discovery_target_subnets_dialog_with_items(rows, window, cx);
    }

    fn open_discovery_target_subnets_dialog_with_items(
        &mut self,
        mut items: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if items.is_empty() {
            items.push(String::new());
        }

        let mut input_states: Vec<Entity<InputState>> = Vec::with_capacity(items.len());
        for value in items {
            let state = cx.new(|cx| InputState::new(window, cx).placeholder("请输入"));
            state.update(cx, |s, cx| {
                s.set_value(value.clone(), window, cx);
            });
            input_states.push(state);
        }

        let home_entity = cx.entity();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let rows_for_save = input_states.clone();
            let home_for_save = home_entity.clone();

            let row_elements: Vec<AnyElement> = input_states
                .iter()
                .enumerate()
                .map(|(idx, input_state)| {
                    let input_state = input_state.clone();
                    let rows_for_add = input_states.clone();
                    let rows_for_remove = input_states.clone();
                    let home_for_add = home_entity.clone();
                    let home_for_remove = home_entity.clone();

                    h_flex()
                        .id(format!("discovery-target-subnet-row-{idx}"))
                        .w_full()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(&input_state).appearance(true).large()),
                        )
                        .child(
                            div()
                                .id(format!("discovery-target-subnet-add-{idx}"))
                                .w(px(24.))
                                .h(px(24.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_color(_cx.theme().muted_foreground)
                                .on_click(move |_event, window, cx| {
                                    let mut values = rows_for_add
                                        .iter()
                                        .map(|state| state.read(cx).value().trim().to_string())
                                        .collect::<Vec<_>>();
                                    if values.len() >= 32 {
                                        let _ = home_for_add.update(cx, |this, cx| {
                                            this.open_simple_notice_dialog(
                                                "最多支持 32 条发现目标规则。",
                                                window,
                                                cx,
                                            );
                                        });
                                        return;
                                    }
                                    values.insert(idx + 1, String::new());
                                    window.close_dialog(cx);
                                    let _ = home_for_add.update(cx, |this, cx| {
                                        this.open_discovery_target_subnets_dialog_with_items(
                                            values, window, cx,
                                        );
                                    });
                                })
                                .child(
                                app_icon(
                                    paths::PLUS,
                                    Size::Small,
                                    _cx.theme().foreground,
                                ),
                                ),
                        )
                        .child(
                            div()
                                .id(format!("discovery-target-subnet-remove-{idx}"))
                                .w(px(24.))
                                .h(px(24.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_color(_cx.theme().muted_foreground)
                                .on_click(move |_event, window, cx| {
                                    let mut values = rows_for_remove
                                        .iter()
                                        .map(|state| state.read(cx).value().trim().to_string())
                                        .collect::<Vec<_>>();
                                    if values.len() <= 1 {
                                        values = vec![String::new()];
                                    } else {
                                        values.remove(idx);
                                    }
                                    window.close_dialog(cx);
                                    let _ = home_for_remove.update(cx, |this, cx| {
                                        this.open_discovery_target_subnets_dialog_with_items(
                                            values, window, cx,
                                        );
                                    });
                                })
                                .child(
                                app_icon(
                                    paths::TRASH,
                                    Size::Small,
                                    _cx.theme().muted_foreground,
                                ),
                            ),
                        )
                        .into_any_element()
                })
                .collect();

            dialog
                .title(dialog_title("发现目标网段"))
                .overlay(true)
                .w(px(340.))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(10.))
                        .child(
                            div()
                                .w_full()
                                .text_xs()
                                .line_height(px(18.))
                                .text_color(_cx.theme().muted_foreground)
                                .child("每行一条规则"),
                        )
                        .child(
                            div()
                                .w_full()
                                .rounded_md()
                                .border_1()
                                .border_color(_cx.theme().border)
                                .bg(_cx.theme().secondary)
                                .p(px(8.))
                                .child(
                                    v_flex()
                                        .w_full()
                                        .gap(px(4.))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(_cx.theme().muted_foreground)
                                                .child("输入示例"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .line_height(px(18.))
                                                .text_color(_cx.theme().foreground)
                                                .child("192.168.2.0/24"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .line_height(px(18.))
                                                .text_color(_cx.theme().foreground)
                                                .child("192.168.2.*"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .line_height(px(18.))
                                                .text_color(_cx.theme().foreground)
                                                .child("192.168.2"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .line_height(px(18.))
                                                .text_color(_cx.theme().foreground)
                                                .child("192.168.2.15"),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .max_h(px(260.))
                                .overflow_y_scrollbar()
                                .child(v_flex().w_full().gap(px(8.)).children(row_elements)),
                        ),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("保存")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "discovery-target-subnets",
                    "保存",
                    "取消",
                ))
                .on_ok(move |_event, window, cx| {
                    let values = rows_for_save
                        .iter()
                        .map(|state| state.read(cx).value().trim().to_string())
                        .filter(|token| !token.is_empty())
                        .collect::<Vec<_>>();
                    let mut valid_rules: Vec<String> = Vec::new();
                    let mut invalid_rules: Vec<String> = Vec::new();
                    for token in values {
                        if crate::core::discovery::is_discovery_target_rule_valid(&token) {
                            if !valid_rules.iter().any(|item| item == &token) {
                                valid_rules.push(token);
                            }
                        } else {
                            invalid_rules.push(token);
                        }
                    }
                    if !invalid_rules.is_empty() {
                        let preview = invalid_rules
                            .iter()
                            .take(3)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ");
                        home_for_save.update(cx, |this, cx| {
                            this.open_simple_notice_dialog(
                                &format!(
                                    "存在无效网段规则：{}。仅支持 /24~/32、192.168.2.*、192.168.2 或单个 IP。",
                                    preview
                                ),
                                window,
                                cx,
                            );
                        });
                        return false;
                    }
                    home_for_save.update(cx, |this, _cx| {
                        this.settings_state.discovery_target_subnets = valid_rules;
                        this.persist_settings();
                    });
                    true
                })
        });
    }

    pub(super) fn open_network_filters_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("每行一个规则：192.168.1.*"));
        let current = self.settings_state.network_filters.join("\n");
        input_state.update(cx, |state, cx| {
            state.set_value(current, window, cx);
        });
        let home_entity = cx.entity();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_ok = input_state.clone();
            let home_for_ok = home_entity.clone();
            dialog
                .title(dialog_title("网络接口过滤规则"))
                .overlay(true)
                .w(px(380.))
                .child(
                    div()
                        .w_full()
                        .child(Input::new(&input_state).appearance(true).large()),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("保存")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "network-filters",
                    "保存",
                    "取消",
                ))
                .on_ok(move |_event, _window, cx| {
                    let raw = input_for_ok.read(cx).value().to_string();
                    let filters = raw
                        .lines()
                        .map(|line| line.trim().to_string())
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<_>>();
                    home_for_ok.update(cx, |this, cx| {
                        this.settings_state.network_filters = filters;
                        this.sync_server_config_to_runtime(cx);
                        this.persist_settings();
                    });
                    true
                })
        });
    }

    /// Opens LocalSend-like address input dialog (Label/IP tabs, single input).
    pub(super) fn open_send_to_address_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_send_to_address_dialog_with_mode(AddressInputMode::Label, window, cx);
    }

    fn open_send_to_address_dialog_with_mode(
        &mut self,
        mode: AddressInputMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let placeholder = match mode {
            AddressInputMode::Label => "#123",
            AddressInputMode::IpAddress => "输入IP地址",
        };
        let ip_input_state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        self.send_ip_input_state = Some(ip_input_state.clone());
        let home_entity = cx.entity();
        let prefixes = self.local_ip_prefixes();
        let example_text = match mode {
            AddressInputMode::Label => {
                if prefixes.is_empty() {
                    "示例：123\n可用网段：\n- 192.168.1.".to_string()
                } else {
                    let mut text = "示例：123\n可用网段：".to_string();
                    for p in prefixes.iter().take(3) {
                        text.push_str(&format!("\n- {}.", p));
                    }
                    text
                }
            }
            AddressInputMode::IpAddress => {
                if prefixes.is_empty() {
                    "示例：\n- 192.168.1.23\n- 192.168.1.123".to_string()
                } else {
                    let mut text = "示例：".to_string();
                    for p in prefixes.iter().take(3) {
                        text.push_str(&format!("\n- {}.123", p));
                    }
                    text
                }
            }
        };
        let selected_bg = cx.theme().background;
        let selected_fg = cx.theme().foreground;
        let idle_fg = cx.theme().muted_foreground;
        let label_selected = mode == AddressInputMode::Label;
        let ip_selected = mode == AddressInputMode::IpAddress;

        window.open_dialog(cx, move |dialog, _window, cx| {
            let ip_for_ok = ip_input_state.clone();
            let home_for_ok = home_entity.clone();
            let home_for_tag_tab = home_entity.clone();
            let home_for_ip_tab = home_entity.clone();
            let mode_for_ok = mode;

            dialog
                .title(dialog_title("输入地址"))
                .overlay(true)
                .w(px(340.))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(12.))
                        .child(
                            h_flex()
                                .w_full()
                                .h(px(40.))
                                .rounded(radius::FULL)
                                .p(px(3.))
                                .bg(cx.theme().muted)
                                .child(
                                    div()
                                        .id("address-mode-label")
                                        .flex_1()
                                        .h_full()
                                        .rounded(radius::FULL)
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .when(label_selected, |this| this.bg(selected_bg))
                                        .on_click(move |_event, window, cx| {
                                            if mode != AddressInputMode::Label {
                                                window.close_dialog(cx);
                                                home_for_tag_tab.update(cx, |this, cx| {
                                                    this.open_send_to_address_dialog_with_mode(
                                                        AddressInputMode::Label,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }
                                        })
                                        .child(
                                            div()
                                                .text_sm()
                                                .when(label_selected, |this| this.font_semibold())
                                                .text_color(if label_selected {
                                                    selected_fg
                                                } else {
                                                    idle_fg
                                                })
                                                .child("标签"),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("address-mode-ip")
                                        .flex_1()
                                        .h_full()
                                        .rounded(radius::FULL)
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .when(ip_selected, |this| this.bg(selected_bg))
                                        .on_click(move |_event, window, cx| {
                                            if mode != AddressInputMode::IpAddress {
                                                window.close_dialog(cx);
                                                home_for_ip_tab.update(cx, |this, cx| {
                                                    this.open_send_to_address_dialog_with_mode(
                                                        AddressInputMode::IpAddress,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }
                                        })
                                        .child(
                                            div()
                                                .text_sm()
                                                .when(ip_selected, |this| this.font_semibold())
                                                .text_color(if ip_selected {
                                                    selected_fg
                                                } else {
                                                    idle_fg
                                                })
                                                .child("IP 地址"),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .child(Input::new(&ip_input_state).appearance(true).large()),
                        )
                        .child(
                            div()
                                .w_full()
                                .rounded(radius::MD)
                                .bg(cx.theme().muted.opacity(0.7))
                                .px(px(12.))
                                .py(px(10.))
                                .child(
                                    div()
                                        .w_full()
                                        .text_xs()
                                        .line_height(px(18.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(example_text.clone()),
                                ),
                        ),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("确认")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(Self::build_confirm_dialog_footer(
                    "send-to-address",
                    "确认",
                    "取消",
                ))
                .on_ok(move |_event, window, cx| {
                    let raw = ip_for_ok.read(cx).value().trim().to_string();
                    if raw.is_empty() {
                        return false;
                    }
                    home_for_ok.update(cx, |this, cx| {
                        let port = this.settings_state.server_port;
                        match mode_for_ok {
                            AddressInputMode::IpAddress => {
                                this.execute_send(raw.clone(), port, window, cx);
                            }
                            AddressInputMode::Label => {
                                if let Some(ip) = this.resolve_labeled_ip(&raw) {
                                    this.execute_send(ip, port, window, cx);
                                } else {
                                    this.open_simple_notice_dialog(
                                        "无法根据标签推导可用 IP，请切换到“IP 地址”模式直接输入。",
                                        window,
                                        cx,
                                    );
                                }
                            }
                        }
                    });
                    true
                })
        });
    }

    fn resolve_labeled_ip(&self, label: &str) -> Option<String> {
        let suffix = label.trim().trim_start_matches('#');
        let suffix_num: u8 = suffix.parse().ok()?;
        let prefixes = self.local_ip_prefixes();
        let prefix = prefixes.first()?;
        Some(format!("{}.{}", prefix, suffix_num))
    }

    fn local_ip_prefixes(&self) -> Vec<String> {
        let mut prefixes = BTreeSet::new();
        for ip in &self.send_state.local_ips {
            if let Some(p) = ipv4_prefix(ip) {
                prefixes.insert(p);
            }
        }
        if let Some(ip) = detect_primary_route_ipv4() {
            if let Some(p) = ipv4_prefix(&ip.to_string()) {
                prefixes.insert(p);
            }
        }
        prefixes.into_iter().collect()
    }

    pub(super) fn open_send_target_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_send_to_address_dialog(window, cx);
    }

    /// Settings picker: gpui-component `RadioGroup` inside a dialog.
    pub(super) fn open_settings_choice_dialog(
        &mut self,
        title: impl Into<String>,
        options: &'static [&'static str],
        selected: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_pick: impl Fn(&mut HomePage, &'static str, &mut Context<HomePage>) + 'static,
    ) {
        let title = title.into();
        let selected = selected.into();
        let home_entity = cx.entity();
        let on_pick = Rc::new(on_pick);
        let selected_index = options
            .iter()
            .position(|option| *option == selected.as_str());

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let home_entity = home_entity.clone();
            let on_pick = on_pick.clone();
            dialog
                .title(dialog_title(title.clone()))
                .overlay(true)
                .w(px(320.))
                .child(
                    RadioGroup::vertical("settings-choice")
                        .w_full()
                        .selected_index(selected_index)
                        .children(
                            options
                                .iter()
                                .copied()
                                .map(|option| Radio::new(option).label(option)),
                        )
                        .on_click(move |ix, window, cx| {
                            let option = options[*ix];
                            home_entity.update(cx, |this, cx| {
                                on_pick(this, option, cx);
                            });
                            window.close_dialog(cx);
                        }),
                )
        });
    }
}
