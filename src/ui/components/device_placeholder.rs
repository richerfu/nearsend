use crate::ui::icons::{app_icon, paths};
use crate::ui::theme::{radius, sizing, spacing};
use gpui::{div, prelude::*, px, Animation, AnimationExt as _, IntoElement, Window};
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Size};
use std::time::Duration;

const DEVICE_ICONS: &[&str] = &[
    paths::SMARTPHONE,
    paths::MONITOR,
    paths::GLOBE,
    paths::SERVER,
];

/// Empty nearby-device placeholder with rotating device icons.
#[derive(Clone, Copy, IntoElement)]
pub struct DevicePlaceholder;

impl gpui::RenderOnce for DevicePlaceholder {
    fn render(self, _window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let muted = cx.theme().muted;
        let skeleton = cx.theme().muted_foreground.opacity(0.18);
        let icon_count = DEVICE_ICONS.len();
        let cycle_ms: u64 = 3000 * icon_count as u64;
        let icon_color = cx.theme().muted_foreground;

        div()
            .relative()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border.opacity(0.8))
            .rounded(radius::LG)
            .p(sizing::CARD_PADDING)
            .child(
                h_flex()
                    .gap(spacing::MD)
                    .items_center()
                    .w_full()
                    .child(
                        div()
                            .w(px(42.))
                            .h(px(42.))
                            .bg(muted)
                            .rounded(radius::MD)
                            .flex()
                            .items_center()
                            .justify_center()
                            .with_animation(
                                "device-icon-rotate",
                                Animation::new(Duration::from_millis(cycle_ms)).repeat(),
                                move |this, delta| {
                                    let elapsed = delta * cycle_ms as f32;
                                    let per_icon_ms = 3000.0_f32;
                                    let mut idx = (elapsed / per_icon_ms).floor() as usize;
                                    if idx >= icon_count {
                                        idx = icon_count - 1;
                                    }
                                    let local = elapsed - (idx as f32 * per_icon_ms);
                                    let fade = 300.0_f32;
                                    let alpha = if local < fade {
                                        local / fade
                                    } else if local > per_icon_ms - fade {
                                        (per_icon_ms - local) / fade
                                    } else {
                                        1.0
                                    };
                                    this.opacity(alpha.clamp(0.0, 1.0)).child(app_icon(
                                        DEVICE_ICONS[idx],
                                        Size::Small,
                                        icon_color,
                                    ))
                                },
                            ),
                    )
                    .child(
                        v_flex()
                            .gap(px(8.))
                            .flex_1()
                            .child(div().w(px(100.)).h(px(12.)).bg(skeleton).rounded(px(6.)))
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .child(div().w(px(52.)).h(px(18.)).bg(skeleton).rounded(px(9.)))
                                    .child(
                                        div().w(px(88.)).h(px(18.)).bg(skeleton).rounded(px(9.)),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(px(12.))
                    .top(px(14.))
                    .w(px(22.))
                    .h(px(22.))
                    .rounded_full()
                    .bg(muted),
            )
    }
}
