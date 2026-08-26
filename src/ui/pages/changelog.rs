//! Changelog page rendered from project `changelog.md`.

use crate::ui::components::chrome::{back_icon_button, page_header, muted_card};
use crate::ui::routes;
use crate::ui::theme::spacing;
use gpui::{div, prelude::*, px, Context, Entity, Window};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
    text::markdown,
    v_flex, ActiveTheme as _,
};

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
        v_flex()
            .size_full()
            .bg(cx.theme().muted.opacity(0.45))
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
                        .py(px(12.))
                        .child(
                            muted_card(cx).p(px(14.)).child(
                                markdown(include_str!("../../../changelog.md"))
                                    .selectable(true)
                                    .scrollable(false),
                            ),
                        )
                        .child(div().h(px(24.))),
                ),
            )
    }
}
