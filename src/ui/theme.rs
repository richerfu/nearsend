//! Mobile-first theme tokens aligned with shadcn + HarmonyOS spacing.

use gpui::{hsla, App, Hsla};
use gpui_component::theme::Theme;

/// Mobile-first spacing constants
pub mod spacing {
    use gpui::px;

    #[allow(dead_code)]
    pub const XS: gpui::Pixels = px(4.);
    pub const SM: gpui::Pixels = px(8.);
    pub const MD: gpui::Pixels = px(16.);
    #[allow(dead_code)]
    pub const LG: gpui::Pixels = px(24.);
    #[allow(dead_code)]
    pub const XL: gpui::Pixels = px(32.);
    pub const PAGE: gpui::Pixels = px(16.);
    #[allow(dead_code)]
    pub const SECTION: gpui::Pixels = px(20.);
}

/// Corner radii (shadcn-like)
pub mod radius {
    use gpui::px;

    #[allow(dead_code)]
    pub const SM: gpui::Pixels = px(8.);
    pub const MD: gpui::Pixels = px(12.);
    pub const LG: gpui::Pixels = px(16.);
    pub const FULL: gpui::Pixels = px(999.);
}

/// Mobile-first sizing constants
pub mod sizing {
    use gpui::px;

    #[allow(dead_code)]
    pub const BUTTON_HEIGHT: gpui::Pixels = px(44.);
    pub const CARD_PADDING: gpui::Pixels = px(14.);
    #[allow(dead_code)]
    pub const CARD_BORDER_RADIUS: gpui::Pixels = px(16.);
    pub const TAB_BAR_HEIGHT: gpui::Pixels = px(58.);
    pub const HEADER_HEIGHT: gpui::Pixels = px(52.);
    pub const ICON_BUTTON: gpui::Pixels = px(40.);
    pub const TOUCH: gpui::Pixels = px(44.);
}

/// Brand green from the NearSend logo.
pub fn brand_primary() -> Hsla {
    hsla(113.0 / 360.0, 0.42, 0.44, 1.0)
}

pub fn apply_nearsend_theme(cx: &mut App) {
    let brand = brand_primary();
    let brand_hover = hsla(113.0 / 360.0, 0.44, 0.38, 1.0);
    let brand_active = hsla(113.0 / 360.0, 0.46, 0.32, 1.0);
    let on_brand = hsla(0.0, 0.0, 1.0, 1.0);

    let theme = Theme::global_mut(cx);
    theme.radius = radius::MD;
    theme.radius_lg = radius::LG;
    theme.primary = brand;
    theme.primary_hover = brand_hover;
    theme.primary_active = brand_active;
    theme.primary_foreground = on_brand;
    theme.button_primary = brand;
    theme.button_primary_hover = brand_hover;
    theme.button_primary_active = brand_active;
    theme.button_primary_foreground = on_brand;
    theme.ring = brand;
    theme.progress_bar = brand;
    theme.slider_bar = brand;
}
