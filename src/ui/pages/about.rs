//! About page: project introduction and metadata.

use crate::ui::components::chrome::{back_icon_button, page_header, surface_card};
use crate::ui::routes;
use crate::ui::theme::spacing;
use gpui::{div, prelude::*, px, Context, Entity, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{h_flex, v_flex, ActiveTheme as _, StyledExt as _};

/// About page for NearSend.
pub struct AboutPage {
    pub root: Option<Entity<crate::app::AppRoot>>,
}

impl AboutPage {
    pub fn new(root: Entity<crate::app::AppRoot>) -> Self {
        Self { root: Some(root) }
    }
}

fn info_card(
    id: impl Into<String>,
    title: &'static str,
    body: impl IntoElement,
    cx: &mut Context<AboutPage>,
) -> impl IntoElement {
    surface_card(cx)
        .id(id.into())
        .p(px(14.))
        .child(v_flex().w_full().gap(px(8.))
            .child(
                div()
                    .text_base()
                    .font_semibold()
                    .text_color(cx.theme().foreground)
                    .child(title),
            )
            .child(body),
        )
}

impl gpui::Render for AboutPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let intro = div()
            .text_sm()
            .line_height(px(22.))
            .text_color(cx.theme().muted_foreground)
            .child(
                "NearSend 是一个基于 GPUI 构建的跨设备局域网传输应用，聚焦文件与文本在同一网络下的快速、安全分发体验。",
            );

        let capabilities = v_flex()
            .w_full()
            .gap(px(6.))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("1. 设备发现：自动发现同网段设备，支持按规则过滤网络接口。"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("2. 发送能力：支持单设备、多设备与链接分享三种发送模式。"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("3. 接收能力：支持 PIN 验证、自动接受策略与历史记录管理。"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("4. 安全选项：支持加密传输与访问控制配置。"),
            );

        let stack = v_flex()
            .w_full()
            .gap(px(6.))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("UI：GPUI + gpui-component + gpui-router"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("异步运行时：tokio"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("传输协议：LocalSend 协议生态"),
            );

        let metadata = v_flex()
            .w_full()
            .gap(px(8.))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("应用版本"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(cx.theme().foreground)
                            .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("项目名称"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(cx.theme().foreground)
                            .child("NearSend"),
                    ),
            );

        v_flex()
            .size_full()
            .bg(cx.theme().muted.opacity(0.45))
            .child(page_header(
                "关于",
                back_icon_button("about-back", cx, |this, _window, cx| {
                    if let Some(root) = &this.root {
                        let _ = root.update(cx, |root, cx| {
                            root.go_back_or_navigate(routes::HOME, cx);
                        });
                    }
                }),
                div(),
                cx,
            ))
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .px(spacing::PAGE)
                    .py(px(12.))
                    .gap(px(12.))
                    .child(info_card("about-intro", "项目简介", intro, cx))
                    .child(info_card(
                        "about-capabilities",
                        "核心能力",
                        capabilities,
                        cx,
                    ))
                    .child(info_card("about-stack", "技术栈", stack, cx))
                    .child(info_card("about-meta", "应用信息", metadata, cx))
                    .child(div().h(px(24.))),
            )
    }
}
