use gpui::{div, prelude::*, px, IntoElement, Window};
use gpui_component::ActiveTheme as _;

/// Compact iOS / shadcn-style switch. Parent should wrap with on_click.
#[derive(IntoElement)]
pub struct Switch {
    checked: bool,
}

impl Switch {
    pub fn new(checked: bool) -> Self {
        Self { checked }
    }
}

impl gpui::RenderOnce for Switch {
    fn render(self, _window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let checked = self.checked;
        let track = if checked {
            cx.theme().primary
        } else {
            cx.theme().muted
        };

        div()
            .id("switch")
            .w(px(46.))
            .h(px(28.))
            .rounded_full()
            .bg(track)
            .relative()
            .flex_none()
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(if checked { px(20.) } else { px(2.) })
                    .w(px(24.))
                    .h(px(24.))
                    .rounded_full()
                    .bg(cx.theme().background)
                    .shadow(vec![gpui_component::box_shadow(
                        px(0.),
                        px(1.),
                        px(2.),
                        px(0.),
                        cx.theme().foreground.opacity(0.18),
                    )]),
            )
    }
}
