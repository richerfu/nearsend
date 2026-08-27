//! Send content selection and send-mode related actions/dialogs.

use super::*;

impl HomePage {
    pub(super) fn handle_pick_content(
        &mut self,
        content_type: SendContentType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_state.send_content_type = content_type;
        match content_type {
            SendContentType::Text => self.open_text_input_dialog(window, cx),
            SendContentType::File => self.handle_pick_from_system(false, window, cx),
            SendContentType::Folder => self.handle_pick_from_system(true, window, cx),
            SendContentType::Media => self.handle_pick_clipboard(window, cx),
        }
    }

    fn handle_pick_from_system(
        &mut self,
        pick_folder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let window_handle = window.window_handle();
        let home_entity = cx.entity();
        let send_selection_state = self.send_selection_state.clone();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();
        let join = tokio_handle.spawn(async move {
            let uris = if pick_folder {
                crate::platform::file_picker::pick_folders().await
            } else {
                crate::platform::file_picker::pick_files().await
            };
            match uris {
                Ok(uris) => {
                    if uris.is_empty() {
                        PathPickOutcome::Cancelled
                    } else {
                        let picked = uris
                            .into_iter()
                            .filter_map(|uri| {
                                crate::platform::file_picker::picker_uri_to_path_with_uri(&uri)
                            })
                            .collect::<Vec<_>>();
                        if picked.is_empty() {
                            PathPickOutcome::Failed
                        } else {
                            PathPickOutcome::Success(picked)
                        }
                    }
                }
                Err(err) => {
                    log::error!("pick from system failed: {}", err);
                    PathPickOutcome::Failed
                }
            }
        });

        cx.spawn(async move |_this, cx| {
            let outcome = match join.await {
                Ok(outcome) => outcome,
                Err(err) => {
                    log::error!("picker task failed: {}", err);
                    PathPickOutcome::Failed
                }
            };
            match outcome {
                PathPickOutcome::Success(picked) => {
                    let mut added = 0usize;
                    let _ = send_selection_state.update(cx, |state, _| {
                        added = state.add_picker_paths_recursive(picked.clone());
                    });
                    if added > 0 {
                        return;
                    }
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = home_entity.update(cx, |this, cx| {
                            this.open_simple_notice_dialog(
                                "未添加到可发送文件，请确认已授权并且文件可读。",
                                window,
                                cx,
                            );
                        });
                    });
                }
                PathPickOutcome::Cancelled => {}
                PathPickOutcome::Failed => {
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = home_entity.update(cx, |this, cx| {
                            this.open_simple_notice_dialog("选择文件失败。", window, cx);
                        });
                    });
                }
            }
        })
        .detach();
    }

    fn handle_pick_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let window_handle = window.window_handle();
        let home_entity = cx.entity();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();

        let join = tokio_handle.spawn(async move {
            let permission_granted =
                match crate::platform::clipboard::ensure_read_clipboard_permission().await {
                    Ok(granted) => granted,
                    Err(err) => {
                        log::error!("ensure read clipboard permission failed: {}", err);
                        false
                    }
                };
            if !permission_granted {
                return ClipboardPickOutcome::PermissionDenied;
            }

            let text = match crate::platform::clipboard::read_clipboard_text().await {
                Ok(text) => text,
                Err(err) => {
                    log::error!("read clipboard text failed: {}", err);
                    return ClipboardPickOutcome::ReadFailed;
                }
            };
            if text.is_empty() {
                return ClipboardPickOutcome::Empty;
            }

            ClipboardPickOutcome::Success(text)
        });

        cx.spawn(async move |_this, cx| {
            let outcome = match join.await {
                Ok(outcome) => outcome,
                Err(err) => {
                    log::error!("clipboard task failed: {}", err);
                    ClipboardPickOutcome::ReadFailed
                }
            };

            match outcome {
                ClipboardPickOutcome::Success(text) => {
                    if text.is_empty() {
                        return;
                    }
                    let _ = home_entity.update(cx, |this, cx| {
                        this.send_selection_state.update(cx, |state, _| {
                            state.add_text(text.clone());
                        });
                    });
                }
                ClipboardPickOutcome::Empty => {
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = home_entity.update(cx, |this, cx| {
                            this.show_clipboard_empty_toast(window, cx);
                        });
                    });
                }
                ClipboardPickOutcome::PermissionDenied => {
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = home_entity.update(cx, |this, cx| {
                            this.open_simple_notice_dialog("无权限。请开启权限。", window, cx);
                        });
                    });
                }
                ClipboardPickOutcome::ReadFailed => {
                    let _ = window_handle.update(cx, |_, window, cx| {
                        let _ = home_entity.update(cx, |this, cx| {
                            this.open_simple_notice_dialog("读取剪贴板失败。", window, cx);
                        });
                    });
                }
            }
        })
        .detach();
    }

    pub(super) fn open_add_content_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let home_entity = cx.entity();
        window.open_dialog(cx, move |dialog, _window, cx| {
            let home_file = home_entity.clone();
            let home_folder = home_entity.clone();
            let home_text = home_entity.clone();
            let home_clipboard = home_entity.clone();
            dialog
                .title(dialog_title("添加内容"))
                .overlay(true)
                .w(px(340.))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(12.))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("选择要加入的内容类型"),
                        )
                        .child(content_picker_grid([
                            content_picker_tile(
                                "add-file",
                                paths::FILE,
                                "文件",
                                cx,
                                move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    home_file.update(cx, |this, cx| {
                                        this.handle_pick_content(SendContentType::File, window, cx);
                                    });
                                },
                            ),
                            content_picker_tile(
                                "add-folder",
                                paths::FOLDER,
                                "文件夹",
                                cx,
                                move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    home_folder.update(cx, |this, cx| {
                                        this.handle_pick_content(
                                            SendContentType::Folder,
                                            window,
                                            cx,
                                        );
                                    });
                                },
                            ),
                            content_picker_tile(
                                "add-text",
                                paths::BOOK_OPEN,
                                "文本",
                                cx,
                                move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    home_text.update(cx, |this, cx| {
                                        this.handle_pick_content(SendContentType::Text, window, cx);
                                    });
                                },
                            ),
                            content_picker_tile(
                                "add-clipboard",
                                paths::COPY,
                                "剪贴板",
                                cx,
                                move |_event, window, cx| {
                                    window.close_dialog(cx);
                                    home_clipboard.update(cx, |this, cx| {
                                        this.handle_pick_content(
                                            SendContentType::Media,
                                            window,
                                            cx,
                                        );
                                    });
                                },
                            ),
                        ])),
                )
                .footer(Self::build_close_footer("add-content", "关闭"))
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("关闭"))
        });
    }

    #[allow(dead_code)]
    pub(super) fn open_send_mode_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current_mode = self.settings_state.send_mode_default;
        let home_entity = cx.entity();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let home_single = home_entity.clone();
            let home_multiple = home_entity.clone();
            let home_link = home_entity.clone();
            dialog
                .title(dialog_title("发送模式"))
                .overlay(true)
                .w(px(320.))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(8.))
                        .child(
                            Button::new("send-mode-single")
                                .with_variant(gpui_component::button::ButtonVariant::Secondary)
                                .outline()
                                .w_full()
                                .on_click(move |_event, window, cx| {
                                    let _ = home_single.update(cx, |this, _| {
                                        this.apply_send_mode_default(SendMode::Single);
                                    });
                                    window.close_dialog(cx);
                                })
                                .child(
                                    h_flex()
                                        .w_full()
                                        .justify_between()
                                        .items_center()
                                        .child(div().text_sm().child("单接收者"))
                                        .child(
                                            if matches!(current_mode, SendModeSetting::Single) {
                                                app_icon(
                                                    paths::CHECK,
                                                    Size::Small,
                                                    _cx.theme().primary,
                                                )
                                            } else {
                                                app_icon(
                                                    paths::MORE,
                                                    Size::Small,
                                                    _cx.theme().muted_foreground,
                                                )
                                            },
                                        ),
                                ),
                        )
                        .child(
                            Button::new("send-mode-multiple")
                                .with_variant(gpui_component::button::ButtonVariant::Secondary)
                                .outline()
                                .w_full()
                                .on_click(move |_event, window, cx| {
                                    let _ = home_multiple.update(cx, |this, _| {
                                        this.apply_send_mode_default(SendMode::Multiple);
                                    });
                                    window.close_dialog(cx);
                                })
                                .child(
                                    h_flex()
                                        .w_full()
                                        .justify_between()
                                        .items_center()
                                        .child(div().text_sm().child("多个接收者"))
                                        .child(
                                            if matches!(current_mode, SendModeSetting::Multiple) {
                                                app_icon(
                                                    paths::CHECK,
                                                    Size::Small,
                                                    _cx.theme().primary,
                                                )
                                            } else {
                                                app_icon(
                                                    paths::MORE,
                                                    Size::Small,
                                                    _cx.theme().muted_foreground,
                                                )
                                            },
                                        ),
                                ),
                        )
                        .child(
                            Button::new("send-mode-link")
                                .with_variant(gpui_component::button::ButtonVariant::Secondary)
                                .outline()
                                .w_full()
                                .on_click(move |_event, window, cx| {
                                    let mut has_selected_files = false;
                                    let _ = home_link.update(cx, |this, _cx| {
                                        has_selected_files =
                                            !this.send_state.selected_files.is_empty();
                                        if has_selected_files {
                                            this.apply_send_mode_default(SendMode::Link);
                                        }
                                    });
                                    window.close_dialog(cx);
                                    if has_selected_files {
                                        let _ = home_link.update(cx, |this, cx| {
                                            this.navigate_to(routes::SEND_LINK, cx);
                                        });
                                        window.refresh();
                                    } else {
                                        let _ = home_link.update(cx, |this, cx| {
                                            this.open_simple_notice_dialog(
                                                "请先选择要发送的文件或文本",
                                                window,
                                                cx,
                                            );
                                        });
                                    }
                                })
                                .child(
                                    h_flex()
                                        .w_full()
                                        .justify_between()
                                        .items_center()
                                        .child(div().text_sm().child("通过分享链接发送"))
                                        .child(if matches!(current_mode, SendModeSetting::Link) {
                                            app_icon(paths::CHECK, Size::Small, _cx.theme().primary)
                                        } else {
                                            app_icon(
                                                paths::MORE,
                                                Size::Small,
                                                _cx.theme().muted_foreground,
                                            )
                                        }),
                                ),
                        ),
                )
                .footer(Self::build_alert_dialog_footer("send-mode", "关闭"))
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("关闭"))
        });
    }

    pub(super) fn open_send_mode_help_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(dialog_title("发送模式说明"))
                .overlay(true)
                .w(px(360.))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(10.))
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(_cx.theme().foreground)
                                .child("单接收者"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(_cx.theme().muted_foreground)
                                .line_height(px(20.))
                                .child("一次只发送给 1 台设备。发送完成后会自动清空当前发送列表。"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(_cx.theme().foreground)
                                .child("多个接收者"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(_cx.theme().muted_foreground)
                                .line_height(px(20.))
                                .child("可同时选择多个接收者。发送完成后会保留当前发送列表，便于再次发送。"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(_cx.theme().foreground)
                                .child("链接分享"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(_cx.theme().muted_foreground)
                                .line_height(px(20.))
                                .child("生成分享链接和二维码，对方可在浏览器打开并下载，无需安装 NearSend。"),
                        ),
                )
                .footer(Self::build_alert_dialog_footer("send-mode-help", "关闭"))
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("关闭"))
        });
    }

    #[allow(dead_code)]
    pub(super) fn cycle_send_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next_mode = match self.send_state.send_mode {
            SendMode::Single => SendMode::Multiple,
            SendMode::Multiple => SendMode::Link,
            SendMode::Link => SendMode::Single,
        };
        let mode_text = match next_mode {
            SendMode::Single => "单设备发送模式",
            SendMode::Multiple => "多设备发送模式（基础）",
            SendMode::Link => "链接分享模式",
        };
        if matches!(next_mode, SendMode::Link) && !self.ensure_has_selected_files(window, cx) {
            return;
        }
        self.apply_send_mode_current(next_mode);
        if matches!(next_mode, SendMode::Link) {
            self.navigate_to(routes::SEND_LINK, cx);
            window.refresh();
        } else {
            self.open_simple_notice_dialog(mode_text, window, cx);
        }
    }

    #[allow(dead_code)]
    fn open_share_link_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut entries = Vec::new();
        for file in &self.send_state.selected_files {
            if let Some(text) = &file.text_content {
                entries.push(crate::core::share_links::SharedEntry::Text {
                    name: if file.name.is_empty() {
                        "message.txt".to_string()
                    } else {
                        file.name.clone()
                    },
                    content: text.clone(),
                });
            } else {
                entries.push(crate::core::share_links::SharedEntry::File {
                    name: file.name.clone(),
                    path: file.path.clone(),
                    file_type: file.file_type.clone(),
                });
            }
        }
        let Some(share_id) = crate::core::share_links::create_share(entries) else {
            self.open_simple_notice_dialog("创建分享链接失败。", window, cx);
            return;
        };
        let scheme = if self.settings_state.encryption {
            "https"
        } else {
            "http"
        };
        let host = if let Some(ip) = detect_primary_route_ipv4() {
            ip.to_string()
        } else if let Some(ip) = self.send_state.local_ips.first() {
            ip.clone()
        } else {
            "127.0.0.1".to_string()
        };
        let link = format!(
            "{}://{}:{}/share/{}",
            scheme, host, self.settings_state.server_port, share_id
        );

        let home_entity = cx.entity();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let home_for_copy = home_entity.clone();
            let link_for_copy = link.clone();
            dialog
                .title(dialog_title("分享链接"))
                .overlay(true)
                .w(px(380.))
                .child(
                    v_flex()
                        .w_full()
                        .gap(px(10.))
                        .child(
                            div()
                                .text_sm()
                                .text_color(_cx.theme().muted_foreground)
                                .child("将下方链接分享给对方浏览器下载："),
                        )
                        .child(
                            div()
                                .w_full()
                                .rounded_md()
                                .px(px(10.))
                                .py(px(8.))
                                .bg(_cx.theme().muted)
                                .overflow_hidden()
                                .truncate()
                                .text_sm()
                                .child(link.clone()),
                        )
                        .child(
                            Button::new("share-link-copy")
                                .primary()
                                .on_click(move |_event, window, cx| {
                                    let link_text = link_for_copy.clone();
                                    home_for_copy.update(cx, |this, cx| {
                                        let window_handle = window.window_handle();
                                        let home_entity = cx.entity();
                                        let tokio_handle =
                                            this.app_state.read(cx).tokio_handle.clone();
                                        let join = tokio_handle.spawn(async move {
                                            crate::platform::clipboard::write_clipboard_text(
                                                link_text,
                                            )
                                            .await
                                            .unwrap_or(false)
                                        });
                                        cx.spawn(async move |_this, cx| {
                                            let copied = join.await.unwrap_or(false);
                                            let _ = window_handle.update(cx, |_, window, cx| {
                                                let _ = home_entity.update(cx, |this, cx| {
                                                    if copied {
                                                        this.show_copy_success_toast(window, cx);
                                                    } else {
                                                        this.open_simple_notice_dialog(
                                                            "复制失败，请手动复制链接。",
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                });
                                            });
                                        })
                                        .detach();
                                    });
                                })
                                .child("复制链接"),
                        ),
                )
                .footer(Self::build_alert_dialog_footer("share-link", "关闭"))
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("关闭"))
        });
    }
}
