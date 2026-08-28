//! Changelog page rendered from project `changelog.md`.

use crate::ui::components::chrome::{back_icon_button, page_header, surface_card};
use crate::ui::routes;
use crate::ui::theme::{radius, spacing};
use gpui::{div, prelude::*, px, Context, Entity, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{h_flex, v_flex, ActiveTheme as _, StyledExt as _};

const CHANGELOG: &str = include_str!("../../../changelog.md");

struct ChangelogSection {
    version: String,
    items: Vec<String>,
}

fn parse_changelog(src: &str) -> Vec<ChangelogSection> {
    let mut sections = Vec::new();
    let mut current: Option<ChangelogSection> = None;

    for line in src.lines() {
        let line = line.trim();
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some(section) = current.take() {
                if !section.items.is_empty() {
                    sections.push(section);
                }
            }
            if heading != "说明" {
                current = Some(ChangelogSection {
                    version: heading.to_string(),
                    items: Vec::new(),
                });
            }
            continue;
        }

        if let Some(item) = line.strip_prefix("- ") {
            if let Some(section) = current.as_mut() {
                section.items.push(item.to_string());
            }
        }
    }

    if let Some(section) = current {
        if !section.items.is_empty() {
            sections.push(section);
        }
    }

    sections
}

pub struct ChangelogPage {
    pub root: Option<Entity<crate::app::AppRoot>>,
}

impl ChangelogPage {
    pub fn new(root: Entity<crate::app::AppRoot>) -> Self {
        Self { root: Some(root) }
    }
}

impl gpui::Render for ChangelogPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sections = parse_changelog(CHANGELOG);

        v_flex()
            .size_full()
            .child(page_header(
                "更新日志",
                back_icon_button("changelog-back", cx, |this, _window, cx| {
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
                div().flex_1().w_full().overflow_y_scrollbar().child(
                    v_flex()
                        .w_full()
                        .px(spacing::PAGE)
                        .pt(px(12.))
                        .pb(px(12.))
                        .gap(px(12.))
                        .children(sections.into_iter().enumerate().map(|(index, section)| {
                            let is_latest = index == 0;
                            surface_card(cx).p(px(14.)).child(
                                v_flex()
                                    .w_full()
                                    .gap(px(10.))
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .gap(px(8.))
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_semibold()
                                                    .text_color(cx.theme().foreground)
                                                    .child(section.version.clone()),
                                            )
                                            .when(is_latest, |this| {
                                                this.child(
                                                    div()
                                                        .px(px(8.))
                                                        .py(px(2.))
                                                        .rounded(radius::FULL)
                                                        .bg(cx.theme().primary.opacity(0.14))
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_medium()
                                                                .text_color(cx.theme().primary)
                                                                .child("当前"),
                                                        ),
                                                )
                                            }),
                                    )
                                    .child(v_flex().w_full().gap(px(8.)).children(
                                        section.items.into_iter().map(|item| {
                                            h_flex()
                                                .w_full()
                                                .items_start()
                                                .gap(px(8.))
                                                .child(
                                                    div()
                                                        .mt(px(8.))
                                                        .w(px(5.))
                                                        .h(px(5.))
                                                        .rounded_full()
                                                        .bg(cx.theme().muted_foreground)
                                                        .flex_none(),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w(px(0.))
                                                        .text_sm()
                                                        .line_height(px(22.))
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(item),
                                                )
                                        }),
                                    )),
                            )
                        })),
                ),
            )
    }
}
