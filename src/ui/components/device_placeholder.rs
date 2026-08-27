use super::sparse_cycle::{SparseCyclePhase, SparseCycleState};
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
    fn render(self, window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let muted = cx.theme().muted;
        let skeleton = cx.theme().muted_foreground.opacity(0.18);
        let icon_count = DEVICE_ICONS.len();
        let icon_color = cx.theme().muted_foreground;
        let cycle_state = window.use_keyed_state("device-placeholder-cycle", cx, |_, cx| {
            SparseCycleState::new(
                icon_count,
                Duration::from_millis(3000),
                Duration::from_millis(300),
                cx,
            )
        });
        let (icon_index, phase, transition_duration) = {
            let state = cycle_state.read(cx);
            (state.index(), state.phase(), state.transition_duration())
        };
        let icon_path = DEVICE_ICONS[icon_index];
        let icon_slot = div()
            .w(px(42.))
            .h(px(42.))
            .bg(muted)
            .rounded(radius::MD)
            .flex()
            .items_center()
            .justify_center();
        let icon_slot = match phase {
            SparseCyclePhase::FadeIn => icon_slot
                .with_animation(
                    format!("device-icon-fade-in-{icon_index}"),
                    Animation::new(transition_duration),
                    move |this, delta| {
                        this.opacity(delta)
                            .child(app_icon(icon_path, Size::Small, icon_color))
                    },
                )
                .into_any_element(),
            SparseCyclePhase::Stable => icon_slot
                .child(app_icon(icon_path, Size::Small, icon_color))
                .into_any_element(),
            SparseCyclePhase::FadeOut => icon_slot
                .with_animation(
                    format!("device-icon-fade-out-{icon_index}"),
                    Animation::new(transition_duration),
                    move |this, delta| {
                        this.opacity(1.0 - delta).child(app_icon(
                            icon_path,
                            Size::Small,
                            icon_color,
                        ))
                    },
                )
                .into_any_element(),
        };

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
                    .child(icon_slot)
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
