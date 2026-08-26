//! Open source licenses page in settings.

use super::data::{get_third_party_libs, ThirdPartyLib};
use crate::ui::components::chrome::{back_icon_button, page_header};
use crate::ui::icons::{app_icon, paths};
use crate::ui::routes;
use crate::ui::theme::{radius, spacing};
use gpui::{div, prelude::*, px, radians, AnyElement, Context, Entity, ScrollHandle, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Size, StyledExt as _};
use std::collections::HashSet;
use std::f32::consts::FRAC_PI_2;

const LICENSE_TEXT_MAX_HEIGHT: f32 = 220.0;

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

fn render_license_row(
    lib: ThirdPartyLib,
    is_expanded: bool,
    window: &mut Window,
    cx: &mut Context<OpenSourceLicensesPage>,
) -> AnyElement {
    let lib_name = lib.name.clone();
    let lib_name_for_toggle = lib_name.clone();
    let license_text_scroll = window
        .use_keyed_state(
            format!("open-source-license-text-scroll-{lib_name}"),
            cx,
            |_, _| ScrollHandle::default(),
        )
        .read(cx)
        .clone();

    let chevron = app_icon(
        paths::CHEVRON_RIGHT,
        Size::Small,
        cx.theme().muted_foreground,
    )
    .rotate(if is_expanded {
        radians(FRAC_PI_2)
    } else {
        radians(0.)
    });

    v_flex()
        .w_full()
        .min_w(px(0.))
        .child(
            h_flex()
                .id(format!("license-row-{lib_name}"))
                .w_full()
                .items_center()
                .min_h(px(52.))
                .py(px(8.))
                .gap(px(12.))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _event, _window, _cx| {
                    this.toggle_expanded(&lib_name_for_toggle);
                }))
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
                                .child(lib_name.clone()),
                        )
                        .child(
                            div()
                                .w_full()
                                .overflow_hidden()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(lib.repository),
                        ),
                )
                .child(
                    div()
                        .px(px(8.))
                        .py(px(3.))
                        .rounded(radius::FULL)
                        .bg(cx.theme().muted)
                        .child(
                            div()
                                .text_xs()
                                .font_medium()
                                .text_color(cx.theme().muted_foreground)
                                .child(lib.license),
                        ),
                )
                .child(chevron),
        )
        .children(is_expanded.then(|| {
            v_flex().w_full().pb(px(10.)).child(
                v_flex()
                    .w_full()
                    .rounded(radius::MD)
                    .bg(cx.theme().muted.opacity(0.7))
                    .child(
                        v_flex()
                            .id(format!("open-source-license-text-scroll-area-{lib_name}"))
                            .w_full()
                            .min_h(px(0.))
                            .h(px(LICENSE_TEXT_MAX_HEIGHT))
                            .overflow_y_scroll()
                            .track_scroll(&license_text_scroll)
                            .overflow_x_hidden()
                            .p(px(12.))
                            .child(
                                div()
                                    .w_full()
                                    .text_xs()
                                    .line_height(px(18.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(lib.license_text),
                            ),
                    ),
            )
        }))
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
                    .child(
                        div()
                            .id("open-source-licenses-list-scroll-area")
                            .w_full()
                            .h_full()
                            .overflow_y_scroll()
                            .track_scroll(&list_scroll)
                            .child(
                                v_flex()
                                    .w_full()
                                    .min_w(px(0.))
                                    .px(spacing::PAGE)
                                    .pt(px(12.))
                                    .pb(px(12.))
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .px(px(4.))
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "{libs_count} 个核心依赖，点按条目查看 License"
                                            )),
                                    )
                                    .child({
                                        let mut group = v_flex()
                                            .w_full()
                                            .bg(cx.theme().background)
                                            .border_1()
                                            .border_color(cx.theme().border.opacity(0.75))
                                            .rounded(radius::LG)
                                            .px(px(14.));
                                        for (index, lib) in libs.into_iter().enumerate() {
                                            if index > 0 {
                                                group = group.child(
                                                    div()
                                                        .h(px(1.))
                                                        .bg(cx.theme().border.opacity(0.7)),
                                                );
                                            }
                                            let is_expanded = expanded.contains(&lib.name);
                                            group = group.child(render_license_row(
                                                lib,
                                                is_expanded,
                                                window,
                                                cx,
                                            ));
                                        }
                                        group
                                    }),
                            ),
                    )
                    .vertical_scrollbar(&list_scroll),
            )
    }
}
