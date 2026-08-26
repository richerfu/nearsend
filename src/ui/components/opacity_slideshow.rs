use super::sparse_cycle::{SparseCyclePhase, SparseCycleState};
use gpui::{div, prelude::*, Animation, AnimationExt as _, IntoElement, Window};
use gpui_component::ActiveTheme as _;
use std::time::Duration;

/// Simple slideshow that cycles through text children.
#[derive(IntoElement)]
pub struct OpacitySlideshow {
    children: Vec<String>,
    duration_millis: u64,
    switch_duration_millis: u64,
    running: bool,
}

impl OpacitySlideshow {
    pub fn new(children: Vec<String>) -> Self {
        Self {
            children,
            duration_millis: 6000,
            switch_duration_millis: 300,
            running: true,
        }
    }

    pub fn duration_millis(mut self, duration_millis: u64) -> Self {
        self.duration_millis = duration_millis;
        self
    }

    pub fn switch_duration_millis(mut self, switch_duration_millis: u64) -> Self {
        self.switch_duration_millis = switch_duration_millis;
        self
    }

    pub fn running(mut self, running: bool) -> Self {
        self.running = running;
        self
    }
}

impl gpui::RenderOnce for OpacitySlideshow {
    fn render(self, window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        if self.children.is_empty() {
            return div().into_any_element();
        }

        let text_style = div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .text_center();

        if !self.running || self.children.len() == 1 {
            return text_style
                .child(self.children[0].clone())
                .into_any_element();
        }

        let item_duration = Duration::from_millis(self.duration_millis);
        let transition_duration = Duration::from_millis(self.switch_duration_millis);
        let cycle_state = window.use_keyed_state("opacity-slideshow-cycle", cx, |_, cx| {
            SparseCycleState::new(self.children.len(), item_duration, transition_duration, cx)
        });
        let (index, phase, transition_duration) = {
            let state = cycle_state.read(cx);
            (state.index(), state.phase(), state.transition_duration())
        };
        let text = self.children[index].clone();

        match phase {
            SparseCyclePhase::FadeIn => text_style
                .with_animation(
                    format!("opacity-slideshow-fade-in-{index}"),
                    Animation::new(transition_duration),
                    move |this, delta| this.opacity(delta).child(text.clone()),
                )
                .into_any_element(),
            SparseCyclePhase::Stable => text_style.child(text).into_any_element(),
            SparseCyclePhase::FadeOut => text_style
                .with_animation(
                    format!("opacity-slideshow-fade-out-{index}"),
                    Animation::new(transition_duration),
                    move |this, delta| this.opacity(1.0 - delta).child(text.clone()),
                )
                .into_any_element(),
        }
    }
}
