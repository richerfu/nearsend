use gpui::{
    div, percentage, prelude::*, px, svg, Animation, AnimationExt as _, IntoElement,
    Transformation, Window,
};
use gpui_component::{v_flex, ActiveTheme as _, StyledExt as _};
use std::time::Duration;

/// Logo component: shows app logo image with optional "NearSend" text.
#[derive(IntoElement)]
pub struct Logo {
    with_text: bool,
    size: f32,
    spinning: bool,
    duration_secs: u32,
}

impl Logo {
    pub fn new() -> Self {
        Self {
            with_text: false,
            size: 200.0,
            spinning: false,
            duration_secs: 15,
        }
    }

    pub fn with_text(mut self, with_text: bool) -> Self {
        self.with_text = with_text;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn spinning(mut self, spinning: bool) -> Self {
        self.spinning = spinning;
        self
    }

    pub fn duration(mut self, duration_secs: u32) -> Self {
        self.duration_secs = duration_secs;
        self
    }
}

impl Default for Logo {
    fn default() -> Self {
        Self::new()
    }
}

impl gpui::RenderOnce for Logo {
    fn render(self, _window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let size = self.size;
        let with_text = self.with_text;
        let spinning = self.spinning;
        let duration = Duration::from_secs(self.duration_secs.max(1) as u64);
        // gpui::svg 只有在存在 text_color 时才会触发 paint_svg。
        // 每次渲染读取当前主题色，让静态和旋转 logo 同步跟随主题。
        let svg_color = cx.theme().primary;

        let logo = if spinning {
            svg()
                .w(px(size))
                .h(px(size))
                .text_color(svg_color)
                .path("icons/logo.svg")
                .with_animation(
                    "logo-rotate",
                    Animation::new(duration).repeat(),
                    |svg, delta| svg.with_transformation(Transformation::rotate(percentage(delta))),
                )
                .into_any_element()
        } else {
            svg()
                .w(px(size))
                .h(px(size))
                .text_color(svg_color)
                .path("icons/logo.svg")
                .into_any_element()
        };

        if with_text {
            v_flex()
                .gap(px(8.))
                .items_center()
                .child(logo)
                .child(
                    div()
                        .text_3xl()
                        .font_bold()
                        .text_color(cx.theme().foreground)
                        .child("NearSend"),
                )
                .into_any_element()
        } else {
            logo
        }
    }
}
