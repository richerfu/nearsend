use gpui::{prelude::*, Hsla, Window};
use gpui_component::{tag::Tag, Sizable as _};

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
        tag.small().rounded_full().child(self.label)
    }
}
