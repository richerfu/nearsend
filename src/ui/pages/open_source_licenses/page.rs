//! Open source licenses page in settings.

use super::data::{get_third_party_libs, ThirdPartyLib};
use crate::ui::components::chrome::{back_icon_button, page_header, muted_card, surface_card};
use crate::ui::routes;
use crate::ui::theme::{radius, spacing};
use gpui::{div, prelude::*, px, AnyElement, Context, Entity, ScrollHandle, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Sizable as _, Size, StyledExt as _,
};
use std::collections::HashSet;

const LICENSE_TEXT_MAX_HEIGHT: f32 = 320.0;

pub struct OpenSourceLicensesPage {
    pub root: Option<Entity<crate::app::AppRoot>>,
    expanded: HashSet<String>,
}

impl OpenSourceLicensesPage {
    pub fn new(root: Entity<crate::app::AppRoot>) -> Self {
        Self {
            root: Some(root),
            expanded: HashSet::new(),
        }
    }

    fn toggle_expanded(&mut self, lib_name: &str) {
        if self.expanded.contains(lib_name) {
            self.expanded.remove(lib_name);
        } else {
            self.expanded.insert(lib_name.to_string());
        }
    }
}

fn render_license_card(
    lib: ThirdPartyLib,
    is_expanded: bool,
    window: &mut Window,
    cx: &mut Context<OpenSourceLicensesPage>,
) -> AnyElement {
    let lib_name = lib.name.clone();
    let lib_name_for_toggle = lib_name.clone();
    let toggle_label = if is_expanded { "收起" } else { "展开" };
    let license_text_scroll = window
        .use_keyed_state(
            format!("open-source-license-text-scroll-{lib_name}"),
            cx,
            |_, _| ScrollHandle::default(),
        )
        .read(cx)
        .clone();

    surface_card(cx)
        .id(format!("license-card-{lib_name}"))
        .min_w(px(0.))
        .p(px(14.))
        .child(
            v_flex()
                .w_full()
                .gap(px(8.))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            div().flex_1().min_w(px(0.)).child(
                                div()
                                    .w_full()
                                    .text_base()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child(lib_name.clone()),
                            ),
                        )
                        .child(
                            Button::new(format!("license-toggle-{lib_name}"))
                                .with_variant(gpui_component::button::ButtonVariant::Secondary)
                                .outline()
                                .with_size(Size::Small)
                                .on_click(cx.listener(move |this, _event, _window, _cx| {
                                    this.toggle_expanded(&lib_name_for_toggle);
                                }))
                                .child(toggle_label),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .text_xs()
                        .line_height(px(18.))
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("License: {}", lib.license)),
                )
                .child(
                    div()
                        .w_full()
                        .text_xs()
                        .line_height(px(18.))
                        .text_color(cx.theme().muted_foreground)
                        .child(lib.repository),
                )
                .children(is_expanded.then(|| {
                    v_flex()
                        .w_full()
                        .rounded(radius::MD)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted.opacity(0.4))
                        .child(
                            v_flex()
                                .id(format!("open-source-license-text-scroll-area-{lib_name}"))
                                .w_full()
                                .min_h(px(0.))
                                .h(px(LICENSE_TEXT_MAX_HEIGHT))
                                .overflow_y_scroll()
                                .track_scroll(&license_text_scroll)
                                .overflow_x_hidden()
                                .p(px(10.))
                                .child(
                                    div()
                                        .w_full()
                                        .text_xs()
                                        .line_height(px(18.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(lib.license_text),
                                ),
                        )
                })),
        )
        .into_any_element()
}

impl gpui::Render for OpenSourceLicensesPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let libs = get_third_party_libs();
        let libs_count = libs.len();
        let expanded = self.expanded.clone();
        let list_scroll = window
            .use_keyed_state("open-source-licenses-list-scroll", cx, |_, _| {
                ScrollHandle::default()
            })
            .read(cx)
            .clone();

        v_flex()
            .size_full()
            .bg(cx.theme().muted.opacity(0.45))
            .child(page_header(
                "开源协议",
                back_icon_button("open-source-licenses-back", cx, |this, _window, cx| {
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
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .relative()
                    .child(
                        div()
                            .id("open-source-licenses-list-scroll-area")
                            .w_full()
                            .h_full()
                            .flex()
                            .flex_col()
                            .overflow_y_scroll()
                            .track_scroll(&list_scroll)
                            .child(
                                v_flex()
                                    .w_full()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .px(spacing::PAGE)
                                    .py(px(12.))
                                    .gap(px(12.))
                                    .child(
                                        muted_card(cx).p(px(14.))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .line_height(px(22.))
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(format!(
                                                        "以下展示 {libs_count} 个核心依赖包的 License 文本，点击条目可展开查看完整内容。"
                                                    )),
                                            ),
                                    )
                                    .children(libs.into_iter().map(|lib| {
                                        let is_expanded = expanded.contains(&lib.name);
                                        div()
                                            .w_full()
                                            .min_w(px(0.))
                                            .child(render_license_card(
                                                lib,
                                                is_expanded,
                                                window,
                                                cx,
                                            ))
                                    }))
                                    .child(div().h(px(24.))),
                            ),
                    )
                    .vertical_scrollbar(&list_scroll),
            )
    }
}
