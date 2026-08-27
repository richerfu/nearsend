//! HarmonyOS visual avoid-area insets for page chrome.

use crate::GlobalOpenHarmonyApp;
use gpui::{px, App, Pixels};
use openharmony_ability::AvoidAreaType;

#[derive(Clone, Copy, Debug, Default)]
pub struct SafeAreaInsets {
    pub top: Pixels,
    pub right: Pixels,
    pub bottom: Pixels,
    pub left: Pixels,
}

/// Extra inset so page chrome sits below the status bar / home indicator.
///
/// The XComponent may already be offset by `content_rect`; only the remaining
/// overlap against the system avoid area is applied, plus a small breathing gap.
pub fn current(cx: &App) -> SafeAreaInsets {
    let Some(app) = cx.try_global::<GlobalOpenHarmonyApp>() else {
        return fallback();
    };
    let ohos = &app.0;
    let scale = normalized_scale(ohos.scale());
    let content = ohos.content_rect();
    let window = ohos.window_rect();

    let mut top_px = 0.0_f32;
    let mut right_px = 0.0_f32;
    let mut bottom_px = 0.0_f32;
    let mut left_px = 0.0_f32;

    for area_type in [
        AvoidAreaType::System,
        AvoidAreaType::Cutout,
        AvoidAreaType::NavigationIndicator,
    ] {
        let Some(area) = ohos.avoid_area(area_type) else {
            continue;
        };
        if !area.visible {
            continue;
        }
        top_px = top_px.max(area.top_rect.height.max(0) as f32);
        right_px = right_px.max(area.right_rect.width.max(0) as f32);
        bottom_px = bottom_px.max(area.bottom_rect.height.max(0) as f32);
        left_px = left_px.max(area.left_rect.width.max(0) as f32);
    }

    let applied_top = content.top.max(0) as f32;
    let applied_left = content.left.max(0) as f32;
    let applied_right = (window.width - content.width - content.left).max(0) as f32;
    let applied_bottom = (window.height - content.height - content.top).max(0) as f32;

    let extra_top = ((top_px - applied_top) / scale).max(0.0);
    let extra_right = ((right_px - applied_right) / scale).max(0.0);
    let extra_bottom = ((bottom_px - applied_bottom) / scale).max(0.0);
    let extra_left = ((left_px - applied_left) / scale).max(0.0);

    // Keep a minimum gap so icon buttons never sit under status-bar glyphs.
    SafeAreaInsets {
        top: px(extra_top.max(8.0)),
        right: px(extra_right),
        bottom: px(extra_bottom.max(4.0)),
        left: px(extra_left),
    }
}

fn fallback() -> SafeAreaInsets {
    SafeAreaInsets {
        top: px(12.),
        right: px(0.),
        bottom: px(8.),
        left: px(0.),
    }
}

fn normalized_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}
