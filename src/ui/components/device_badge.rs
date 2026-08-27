use gpui::{prelude::*, px, Hsla, Window};
use gpui_component::{tag::Tag, StyledExt as _};

/// Device metadata chip built on gpui-component `Tag`.
#[derive(IntoElement)]
pub struct DeviceBadge {
    label: String,
    colors: Option<(Hsla, Hsla, Hsla)>,
}

impl DeviceBadge {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            colors: None,
        }
    }

    pub fn colors(mut self, background: Hsla, foreground: Hsla, border: Hsla) -> Self {
        self.colors = Some((background, foreground, border));
        self
    }
}

impl gpui::RenderOnce for DeviceBadge {
    fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        let tag = match self.colors {
            Some((bg, fg, border)) => Tag::custom(bg, fg, border),
            None => Tag::secondary().outline(),
        };
        // Tag::small() is py_0p5 + line-height 1, which flattens CJK pills.
        tag.rounded_full()
            .h(px(22.))
            .px(px(8.))
            .flex_none()
            .items_center()
            .text_xs()
            .font_medium()
            .line_height(px(16.))
            .child(self.label)
    }
}
