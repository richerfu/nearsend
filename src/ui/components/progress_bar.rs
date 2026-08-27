use gpui::{ElementId, IntoElement, Window};
use gpui_component::progress::Progress;

/// App progress bar: gpui-component `Progress` with a 0.0–1.0 value.
#[derive(IntoElement)]
pub struct ProgressBar {
    id: ElementId,
    progress: Option<f64>,
}

impl ProgressBar {
    pub fn new(id: impl Into<ElementId>, progress: Option<f64>) -> Self {
        Self {
            id: id.into(),
            progress,
        }
    }
}

impl gpui::RenderOnce for ProgressBar {
    fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
        let value = (self.progress.unwrap_or(0.0).clamp(0.0, 1.0) as f32) * 100.0;
        Progress::new(self.id).value(value)
    }
}
