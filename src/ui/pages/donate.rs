//! Donate page: support information and contact details.

use crate::ui::components::chrome::{back_icon_button, page_header, surface_card};
use crate::ui::icons::{app_icon, paths};
use crate::ui::routes;
use crate::ui::theme::spacing;
use gpui::{div, prelude::*, px, AnyElement, Context, Entity, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Size, StyledExt as _};

const DONATE_EMAIL: &str = "richerfu@qq.com";
const DONATE_GITHUB: &str = "https://github.com/richerfu";
const DONATE_WEBSITE: &str = "https://richerfu.win/";

/// Donate page for NearSend settings.
pub struct DonatePage {
    pub root: Option<Entity<crate::app::AppRoot>>,
}

impl DonatePage {
    pub fn new(root: Entity<crate::app::AppRoot>) -> Self {
        Self { root: Some(root) }
    }
}

fn info_card(
    id: impl Into<String>,
    title: &'static str,
    body: impl IntoElement,
    cx: &mut Context<DonatePage>,
) -> impl IntoElement {
    surface_card(cx)
        .id(id.into())
        .p(px(14.))
        .child(
            v_flex()
                .w_full()
                .gap(px(8.))
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

fn info_item(
    id: impl Into<String>,
    icon: &'static str,
    label: &'static str,
    value: &'static str,
    cx: &mut Context<DonatePage>,
) -> AnyElement {
    h_flex()
        .id(id.into())
        .w_full()
        .items_start()
        .gap(px(8.))
        .child(
            div()
                .h(px(24.))
                .w(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child(app_icon(icon, Size::Small, cx.theme().muted_foreground)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.))
                .gap(px(4.))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                )
                .child(
                    div()
                        .text_sm()
                        .line_height(px(20.))
                        .text_color(cx.theme().foreground)
                        .child(value),
                ),
        )
        .into_any_element()
}

impl gpui::Render for DonatePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let intro = div()
            .text_sm()
            .line_height(px(22.))
            .text_color(cx.theme().muted_foreground)
            .child(
                "如果你觉得 NearSend 对你有帮助，欢迎通过以下方式联系我支持项目。谢谢你的关注与支持。",
            );

        let contact_info = v_flex()
            .w_full()
            .gap(px(12.))
            .child(info_item(
                "donate-email",
                paths::INBOX,
                "邮箱",
                DONATE_EMAIL,
                cx,
            ))
            .child(info_item(
                "donate-github",
                paths::GITHUB,
                "GitHub",
                DONATE_GITHUB,
                cx,
            ))
            .child(info_item(
                "donate-website",
                paths::GLOBE,
                "个人网站",
                DONATE_WEBSITE,
                cx,
            ));

        v_flex()
            .size_full()
            .bg(cx.theme().muted.opacity(0.45))
            .child(page_header(
                "捐赠",
                back_icon_button("donate-back", cx, |this, _window, cx| {
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
                    .child(info_card("donate-intro", "支持项目", intro, cx))
                    .child(info_card("donate-contact", "个人信息", contact_info, cx))
                    .child(div().h(px(24.))),
            )
    }
}
