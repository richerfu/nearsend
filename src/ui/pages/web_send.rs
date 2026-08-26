use crate::core::share_links::SharedEntry;
use crate::ui::components::chrome::{
    back_icon_button, dialog_title, header_icon_button, page_header,
};
use crate::ui::icons::{app_icon, paths};
use crate::ui::pages::HomePage;
use crate::ui::routes;
use crate::ui::theme::spacing;
use gpui::{div, hsla, prelude::*, px, Context, Entity, Window};
use gpui_component::input::{Input, InputState};
use gpui_component::notification::Notification;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    dialog::{DialogAction, DialogClose, DialogFooter},
    h_flex, v_flex, ActiveTheme as _, Sizable as _, Size, StyledExt as _, WindowExt as _,
};
use gpui_router::RouterState;
use qrcode::types::Color;
use qrcode::QrCode;
use std::collections::HashMap;
use std::time::Duration;

struct CachedQr {
    size: usize,
    modules: Vec<bool>,
}

pub struct WebSendPage {
    pub root: Option<Entity<crate::app::AppRoot>>,
    home_entity: Entity<HomePage>,
    share_links: Vec<String>,
    share_id: Option<String>,
    error_text: Option<String>,
    qr_cache: HashMap<String, CachedQr>,
}

impl WebSendPage {
    pub fn new(root: Entity<crate::app::AppRoot>, home_entity: Entity<HomePage>) -> Self {
        Self {
            root: Some(root),
            home_entity,
            share_links: Vec::new(),
            share_id: None,
            error_text: None,
            qr_cache: HashMap::new(),
        }
    }

    fn build_links_for_home(home: &HomePage, share_id: &str) -> Vec<String> {
        let scheme = if home.settings_state.encryption {
            "https"
        } else {
            "http"
        };
        let mut hosts = home.send_state.local_ips.clone();
        if hosts.is_empty() {
            hosts.push("127.0.0.1".to_string());
        }
        let pin_query = if home.settings_state.require_pin {
            let pin = home.settings_state.receive_pin.trim();
            if pin.is_empty() {
                String::new()
            } else {
                format!("?pin={}", urlencoding::encode(pin))
            }
        } else {
            String::new()
        };
        hosts
            .into_iter()
            .map(|host| {
                format!(
                    "{}://{}:{}/share/{}{}",
                    scheme, host, home.settings_state.server_port, share_id, pin_query
                )
            })
            .collect()
    }

    fn refresh_links_from_share_id(&mut self, cx: &mut Context<Self>) {
        let Some(share_id) = self.share_id.clone() else {
            self.share_links.clear();
            return;
        };
        let mut links = Vec::new();
        let _ = self.home_entity.update(cx, |home, _| {
            links = Self::build_links_for_home(home, &share_id);
        });
        self.share_links = links;
        self.qr_cache
            .retain(|link, _| self.share_links.contains(link));
    }

    fn ensure_share_link(&mut self, cx: &mut Context<Self>) {
        if self.share_id.is_some() || self.error_text.is_some() {
            return;
        }
        let mut generated_id: Option<String> = None;
        let mut generated_error: Option<String> = None;
        let _ = self.home_entity.update(cx, |home, _cx| {
            if home.send_state.selected_files.is_empty() {
                generated_error = Some("请先选择要发送的文件或文本".to_string());
                return;
            }
            let mut entries = Vec::new();
            for file in &home.send_state.selected_files {
                if let Some(text) = &file.text_content {
                    entries.push(SharedEntry::Text {
                        name: if file.name.is_empty() {
                            "message.txt".to_string()
                        } else {
                            file.name.clone()
                        },
                        content: text.clone(),
                    });
                } else {
                    entries.push(SharedEntry::File {
                        name: file.name.clone(),
                        path: file.path.clone(),
                        file_type: file.file_type.clone(),
                    });
                }
            }
            let Some(share_id) = crate::core::share_links::create_share(entries) else {
                generated_error = Some("创建分享链接失败".to_string());
                return;
            };
            generated_id = Some(share_id);
        });
        self.share_id = generated_id;
        self.error_text = generated_error;
        self.refresh_links_from_share_id(cx);
    }

    fn regenerate_share_link(&mut self, cx: &mut Context<Self>) {
        self.share_id = None;
        self.share_links.clear();
        self.error_text = None;
        self.qr_cache.clear();
        self.ensure_share_link(cx);
    }

    fn update_share_link_auto_accept(&mut self, value: bool, cx: &mut Context<Self>) {
        let _ = self.home_entity.update(cx, |home, _cx| {
            home.settings_state.share_via_link_auto_accept = value;
            home.persist_settings();
        });
    }

    fn update_require_pin(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let _ = self.home_entity.update(cx, |home, _cx| {
            home.settings_state.require_pin = enabled;
            if enabled && home.settings_state.receive_pin.trim().is_empty() {
                home.settings_state.receive_pin = "123456".to_string();
            }
            home.persist_settings();
            let require_pin = home.settings_state.require_pin;
            let receive_pin = home.settings_state.receive_pin.clone();
            let server_entity = home.app_state.read(_cx).server.clone();
            let tokio_handle = home.app_state.read(_cx).tokio_handle.clone();
            server_entity.update(_cx, |server, _| {
                server.set_receive_pin_config(require_pin, receive_pin, &tokio_handle);
            });
        });
        self.refresh_links_from_share_id(cx);
    }

    fn update_link_encryption(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let _ = self.home_entity.update(cx, |home, _cx| {
            home.settings_state.encryption = enabled;
            home.persist_settings();
            home.restart_local_server_with_current_config(_cx);
        });
        self.refresh_links_from_share_id(cx);
    }

    fn open_pin_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("输入访问 PIN"));
        let current = self.home_entity.read(cx).settings_state.receive_pin.clone();
        input.update(cx, |state, cx| state.set_value(current, window, cx));
        let page = cx.entity();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_ok = input.clone();
            let page_for_ok = page.clone();
            dialog
                .title(dialog_title("设置访问 PIN"))
                .overlay(true)
                .w(px(340.))
                .child(
                    div()
                        .w_full()
                        .child(Input::new(&input).appearance(true).large()),
                )
                .button_props(
                    gpui_component::dialog::DialogButtonProps::default()
                        .ok_text("保存")
                        .show_cancel(true)
                        .cancel_text("取消"),
                )
                .footer(build_confirm_dialog_footer("web-send-pin", "保存", "取消"))
                .on_ok(move |_event, window, cx| {
                    let pin = input_for_ok.read(cx).value().trim().to_string();
                    if pin.is_empty() {
                        return false;
                    }
                    page_for_ok.update(cx, |this, cx| {
                        let _ = this.home_entity.update(cx, |home, _cx| {
                            home.settings_state.receive_pin = pin.clone();
                            home.settings_state.require_pin = true;
                            home.persist_settings();
                            let server_entity = home.app_state.read(_cx).server.clone();
                            let tokio_handle = home.app_state.read(_cx).tokio_handle.clone();
                            server_entity.update(_cx, |server, _| {
                                server.set_receive_pin_config(true, pin.clone(), &tokio_handle);
                            });
                        });
                        this.refresh_links_from_share_id(cx);
                        window.close_dialog(cx);
                    });
                    true
                })
        });
    }

    fn open_notice_dialog(&self, msg: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = msg.to_string();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(dialog_title("提示"))
                .overlay(true)
                .w(px(320.))
                .child(div().w_full().text_sm().child(text.clone()))
                .footer(build_alert_dialog_footer("web-send-notice", "确定"))
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("确定"))
        });
    }

    fn show_copy_success_toast(&self, window: &mut Window, cx: &mut Context<Self>) {
        struct CopySuccessToast;
        window.push_notification(
            Notification::new()
                .id::<CopySuccessToast>()
                .autohide(false)
                .content(|_, _, _| {
                    div()
                        .w_full()
                        .text_xs()
                        .text_center()
                        .child("复制成功")
                        .into_any_element()
                })
                .w(px(92.))
                .py(px(4.))
                .px(px(10.))
                .rounded_full()
                .shadow_none()
                .border_color(hsla(0.0, 0.0, 0.0, 0.0))
                .bg(hsla(0.0, 0.0, 0.12, 0.92))
                .text_color(hsla(0.0, 0.0, 1.0, 0.96)),
            cx,
        );
        let window_handle = window.window_handle();
        let tokio_handle = self
            .home_entity
            .read(cx)
            .app_state
            .read(cx)
            .tokio_handle
            .clone();
        let dismiss = tokio_handle.spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
        });
        cx.spawn(async move |_this, cx| {
            let _ = dismiss.await;
            let _ = window_handle.update(cx, |_, window, cx| {
                window.remove_notification::<CopySuccessToast>(cx);
            });
        })
        .detach();
    }

    fn make_compact_qr(content: &str) -> Option<CachedQr> {
        let qr = QrCode::new(content.as_bytes()).ok()?;
        let src_width = qr.width();
        let src = qr.to_colors();
        let border = 2usize;
        let dst_size = src_width + border * 2;
        let mut modules = vec![false; dst_size * dst_size];
        for y in 0..dst_size {
            for x in 0..dst_size {
                if x >= border && x < border + src_width && y >= border && y < border + src_width {
                    let sx = x - border;
                    let sy = y - border;
                    modules[y * dst_size + x] = src[sy * src_width + sx] == Color::Dark;
                }
            }
        }
        Some(CachedQr {
            size: dst_size,
            modules,
        })
    }

    fn open_qr_dialog(
        &mut self,
        link: &str,
        pin_hint: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cached_qr = if let Some(cached) = self.qr_cache.get(link) {
            CachedQr {
                size: cached.size,
                modules: cached.modules.clone(),
            }
        } else {
            let generated = Self::make_compact_qr(link).unwrap_or_else(|| CachedQr {
                size: 1,
                modules: vec![false],
            });
            self.qr_cache.insert(
                link.to_string(),
                CachedQr {
                    size: generated.size,
                    modules: generated.modules.clone(),
                },
            );
            generated
        };
        let qr_size = cached_qr.size;
        let qr_modules = cached_qr.modules;
        let module_px = ((220.0 / qr_size as f32).floor()).clamp(3.0, 7.0);
        let qr_box_px = module_px * qr_size as f32;
        let link_text = link.to_string();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(dialog_title("分享链接二维码"))
                .overlay(true)
                .w(px(360.))
                .child(
                    v_flex()
                        .w_full()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            div()
                                .w_full()
                                .max_w(px(300.))
                                .overflow_hidden()
                                .text_xs()
                                .text_color(_cx.theme().muted_foreground)
                                .truncate()
                                .child(link_text.clone()),
                        )
                        .child(
                            div()
                                .w_full()
                                .max_w(px(300.))
                                .rounded_md()
                                .bg(_cx.theme().muted)
                                .p(px(8.))
                                .child(
                                    div().w_full().flex().justify_center().child(
                                        v_flex()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(_cx.theme().border)
                                            .bg(_cx.theme().background)
                                            .p(px(6.))
                                            .w(px(qr_box_px))
                                            .h(px(qr_box_px))
                                            .children((0..qr_size).map(|row| {
                                                h_flex()
                                                    .w_full()
                                                    .h(px(module_px))
                                                    .gap(px(0.))
                                                    .children((0..qr_size).map(|col| {
                                                        let dark = qr_modules[row * qr_size + col];
                                                        div().w(px(module_px)).h(px(module_px)).bg(
                                                            if dark {
                                                                _cx.theme().foreground
                                                            } else {
                                                                _cx.theme().background
                                                            },
                                                        )
                                                    }))
                                            })),
                                    ),
                                ),
                        )
                        .when_some(pin_hint.clone(), |this, pin| {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(_cx.theme().foreground)
                                    .child(format!("访问 PIN：{}", pin)),
                            )
                        }),
                )
                .footer(build_alert_dialog_footer("web-send-qr", "关闭"))
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("关闭"))
        });
    }
}

fn build_confirm_dialog_footer(id_prefix: &str, ok_text: &str, cancel_text: &str) -> DialogFooter {
    DialogFooter::new()
        .child(
            DialogClose::new()
                .child(Button::new(format!("{id_prefix}-cancel")).label(cancel_text.to_string())),
        )
        .child(
            DialogAction::new().child(
                Button::new(format!("{id_prefix}-ok"))
                    .label(ok_text.to_string())
                    .primary(),
            ),
        )
}

fn build_alert_dialog_footer(id_prefix: &str, ok_text: &str) -> DialogFooter {
    DialogFooter::new().child(
        DialogAction::new().child(
            Button::new(format!("{id_prefix}-ok"))
                .label(ok_text.to_string())
                .primary(),
        ),
    )
}

impl gpui::Render for WebSendPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_share_link(cx);
        let links = self.share_links.clone();
        let error = self.error_text.clone();
        let encryption = self.home_entity.read(cx).settings_state.encryption;
        let auto_accept = self
            .home_entity
            .read(cx)
            .settings_state
            .share_via_link_auto_accept;
        let require_pin = self.home_entity.read(cx).settings_state.require_pin;
        let pin = self.home_entity.read(cx).settings_state.receive_pin.clone();
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(page_header(
                "分享链接",
                back_icon_button("web-send-back", cx, |this, window, cx| {
                    if let Some(root) = &this.root {
                        let _ = root.update(cx, |this, cx| {
                            this.go_back_or_navigate(routes::HOME, cx);
                        });
                    } else if let Some(entry) =
                        crate::ui::router_history::RouterHistoryState::global_mut(cx)
                            .history
                            .go_back()
                    {
                        RouterState::global_mut(cx).location.pathname = entry.pathname;
                    } else {
                        RouterState::global_mut(cx).location.pathname = routes::HOME.into();
                    }
                    window.refresh();
                }),
                header_icon_button("web-send-refresh-link", paths::REFRESH, cx, |this, _window, cx| {
                    this.regenerate_share_link(cx);
                }),
                cx,
            ))
            .child(
                div().flex_1().w_full().overflow_y_scrollbar().child(
                    v_flex()
                        .w_full()
                        .px(spacing::PAGE)
                        .py(px(16.))
                        .gap(px(12.))
                        .when_some(error.clone(), |this, err| {
                            this.child(
                                div()
                                    .rounded_md()
                                    .bg(cx.theme().danger.opacity(0.1))
                                    .px(px(10.))
                                    .py(px(8.))
                                    .text_sm()
                                    .text_color(cx.theme().danger)
                                    .child(err),
                            )
                        })
                        .when(error.is_none(), |this| {
                            this.child(
                                v_flex()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("将下方链接分享给接收方浏览器："),
                                    )
                                    .children(links.iter().enumerate().map(|(index, link)| {
                                        let copy_link_text = link.clone();
                                        let qr_link_text = link.clone();
                                        let button_id = format!("web-send-copy-{}", index);
                                        let qr_button_id = format!("web-send-qr-{}", index);
                                        let pin_hint = if require_pin {
                                            Some(pin.clone())
                                        } else {
                                            None
                                        };
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .gap(px(8.))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .rounded_md()
                                                    .bg(cx.theme().muted)
                                                    .px(px(10.))
                                                    .py(px(7.))
                                                    .text_sm()
                                                    .truncate()
                                                    .child(link.clone()),
                                            )
                                            .child(
                                                Button::new(qr_button_id)
                                                    .ghost()
                                                    .on_click(cx.listener(
                                                        move |this, _event, window, cx| {
                                                            this.open_qr_dialog(
                                                                &qr_link_text,
                                                                pin_hint.clone(),
                                                                window,
                                                                cx,
                                                            );
                                                        },
                                                    ))
                                                    .child(
                                                        app_icon(
                                                            paths::QR_CODE,
                                                            Size::Small,
                                                            cx.theme().foreground,
                                                        ),
                                                    ),
                                            )
                                            .child(
                                                Button::new(button_id)
                                                    .ghost()
                                                    .on_click(cx.listener(
                                                        move |this, _event, window, cx| {
                                                            let page = cx.entity();
                                                            let window_handle = window.window_handle();
                                                            let tokio_handle = this
                                                                .home_entity
                                                                .read(cx)
                                                                .app_state
                                                                .read(cx)
                                                                .tokio_handle
                                                                .clone();
                                                            let link_for_copy = copy_link_text.clone();
                                                            let join = tokio_handle.spawn(async move {
                                                                crate::platform::clipboard::write_clipboard_text(
                                                                    link_for_copy,
                                                                )
                                                                .await
                                                                .unwrap_or(false)
                                                            });
                                                            cx.spawn(async move |_this, cx| {
                                                                let copied = join.await.unwrap_or(false);
                                                                let _ = window_handle.update(cx, |_, window, cx| {
                                                                    let _ = page.update(cx, |this, _cx| {
                                                                        if copied {
                                                                            this.show_copy_success_toast(
                                                                                window,
                                                                                _cx,
                                                                            );
                                                                        } else {
                                                                            this.open_notice_dialog(
                                                                                "复制失败，请手动复制链接。",
                                                                                window,
                                                                                _cx,
                                                                            );
                                                                        }
                                                                    });
                                                                });
                                                            })
                                                            .detach();
                                                        },
                                                    ))
                                                    .child(
                                                        app_icon(
                                                            paths::COPY,
                                                            Size::Small,
                                                            cx.theme().foreground,
                                                        ),
                                                    ),
                                            )
                                    }))
                            )
                        })
                        .child(
                            div()
                                .rounded_lg()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().secondary)
                                .p(px(12.))
                                .child(
                                    v_flex()
                                        .gap(px(10.))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_semibold()
                                                .text_color(cx.theme().foreground)
                                                .child("访问设置"),
                                        )
                                        .child(
                                            h_flex()
                                                .justify_between()
                                                .items_center()
                                                .child(div().text_sm().child("启用加密 (HTTPS)"))
                                                .child(
                                                    div()
                                                        .id("web-link-enable-encryption")
                                                        .cursor_pointer()
                                                        .on_click(cx.listener(move |this, _ev, _w, cx| {
                                                            this.update_link_encryption(!encryption, cx);
                                                        }))
                                                        .child(crate::ui::components::switch::Switch::new(encryption)),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .justify_between()
                                                .items_center()
                                                .child(div().text_sm().child("分享链接自动接受"))
                                                .child(
                                                    div()
                                                        .id("web-link-auto-accept")
                                                        .cursor_pointer()
                                                        .on_click(cx.listener(move |this, _ev, _w, cx| {
                                                            this.update_share_link_auto_accept(!auto_accept, cx);
                                                        }))
                                                        .child(crate::ui::components::switch::Switch::new(auto_accept)),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .justify_between()
                                                .items_center()
                                                .child(div().text_sm().child("访问需要 PIN"))
                                                .child(
                                                    div()
                                                        .id("web-link-require-pin")
                                                        .cursor_pointer()
                                                        .on_click(cx.listener(move |this, _ev, _w, cx| {
                                                            this.update_require_pin(!require_pin, cx);
                                                        }))
                                                        .child(crate::ui::components::switch::Switch::new(require_pin)),
                                                ),
                                        )
                                        .when(require_pin, |this| {
                                            this.child(
                                                h_flex()
                                                    .justify_between()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w(px(0.))
                                                            .overflow_hidden()
                                                            .truncate()
                                                            .text_sm()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(format!("当前 PIN：{}", pin)),
                                                    )
                                                    .child(
                                                        Button::new("web-link-edit-pin")
                                                            .ghost()
                                                            .on_click(cx.listener(
                                                                |this, _ev, window, cx| {
                                                                    this.open_pin_dialog(window, cx);
                                                                },
                                                            ))
                                                            .child("编辑"),
                                                    ),
                                            )
                                        }),
                                ),
                        ),
                ),
            )
    }
}
