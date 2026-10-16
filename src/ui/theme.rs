//! Responsive shadcn-aligned theme tokens for HarmonyOS.

use gpui::{hsla, rgb, Anchor, App, Hsla, WindowAppearance};
use gpui_component::theme::{Theme, ThemeMode as ComponentThemeMode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::System => "跟随系统",
            Self::Light => "浅色",
            Self::Dark => "深色",
        }
    }

    pub(crate) fn resolve(self, appearance: WindowAppearance) -> ComponentThemeMode {
        match self {
            Self::System => appearance.into(),
            Self::Light => ComponentThemeMode::Light,
            Self::Dark => ComponentThemeMode::Dark,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    #[default]
    System,
    LocalSend,
    Oled,
    Custom,
}

impl ColorMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::System => "NearSend",
            Self::LocalSend => "LocalSend",
            Self::Oled => "OLED",
            Self::Custom => "自定义",
        }
    }
}

pub(crate) const ACCENT_PRESETS: &[(&str, u32)] = &[
    ("绿色", 0x5CA34B),
    ("青色", 0x009688),
    ("蓝色", 0x2563EB),
    ("紫色", 0x7C3AED),
    ("橙色", 0xEA580C),
    ("粉色", 0xDB2777),
];

/// Theme accents are opaque RGB colors. Validate before parsing so malformed
/// persisted values (including non-ASCII input) cannot panic or reset settings.
pub(crate) fn parse_accent_color(value: &str) -> Option<Hsla> {
    let value = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if !matches!(value.len(), 3 | 6) || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut color = u32::from_str_radix(value, 16).ok()?;
    if value.len() == 3 {
        color = ((color & 0xF00) << 8 | (color & 0xF0) << 4 | color & 0xF) * 17;
    }
    Some(rgb(color).into())
}

pub(crate) fn accent_color_hex(color: Hsla) -> String {
    let color = color.to_rgb();
    format!(
        "#{:02X}{:02X}{:02X}",
        (color.r * 255.).round() as u8,
        (color.g * 255.).round() as u8,
        (color.b * 255.).round() as u8,
    )
}

fn luminance(color: Hsla) -> f32 {
    let color = color.to_rgb();
    let linear = |channel: f32| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

pub(crate) fn accent_foreground(color: Hsla) -> Hsla {
    if luminance(color) > 0.179 {
        hsla(0., 0., 0., 1.)
    } else {
        hsla(0., 0., 1., 1.)
    }
}

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
    pub const SIDEBAR_WIDTH: gpui::Pixels = px(236.);
}

/// Brand green from the NearSend logo.
pub fn brand_primary() -> Hsla {
    hsla(113.0 / 360.0, 0.42, 0.44, 1.0)
}

pub(crate) struct NearSendTheme {
    mode: ThemeMode,
    color_mode: ColorMode,
    seed: Hsla,
}

impl NearSendTheme {
    pub(crate) fn new(mode: ThemeMode, color_mode: ColorMode, custom_color: &str) -> Self {
        let seed = match color_mode {
            ColorMode::System => brand_primary(),
            ColorMode::LocalSend | ColorMode::Oled => rgb(0x009688).into(),
            ColorMode::Custom => parse_accent_color(custom_color).unwrap_or_else(brand_primary),
        };
        Self {
            mode,
            color_mode,
            seed,
        }
    }

    pub(crate) fn seed(&self) -> Hsla {
        self.seed
    }

    pub(crate) fn apply(&self, appearance: WindowAppearance, cx: &mut App) {
        let mode = self.mode.resolve(appearance);
        // Reload the base palette so leaving OLED/custom never retains old colors.
        Theme::change(mode, None, cx);
        Theme::update(cx, |theme| self.apply_colors(theme));
    }

    fn apply_colors(&self, theme: &mut Theme) {
        let dark = theme.is_dark();
        let primary = hsla(
            self.seed.h,
            self.seed.s,
            if dark {
                self.seed.l.clamp(0.65, 0.8)
            } else {
                self.seed.l.clamp(0.3, 0.44)
            },
            1.,
        );
        let on_primary = accent_foreground(primary);
        // Move interaction states away from the foreground's brightness so
        // their text contrast never becomes worse than the normal button.
        let direction = if on_primary.l == 0. { 1. } else { -1. };
        let hover = hsla(
            primary.h,
            primary.s,
            (primary.l + direction * 0.04).clamp(0., 1.),
            1.,
        );
        let active = hsla(
            primary.h,
            primary.s,
            (primary.l + direction * 0.08).clamp(0., 1.),
            1.,
        );
        let tint = hsla(
            self.seed.h,
            self.seed.s * 0.3,
            if dark { 0.2 } else { 0.94 },
            1.,
        );

        theme.radius = radius::MD;
        theme.radius_lg = radius::LG;
        theme.primary = primary;
        theme.primary_hover = hover;
        theme.primary_active = active;
        theme.primary_foreground = on_primary;
        theme.button_primary = primary;
        theme.button_primary_hover = hover;
        theme.button_primary_active = active;
        theme.button_primary_foreground = on_primary;
        theme.ring = primary;
        theme.progress_bar = primary;
        theme.slider_bar = primary;
        theme.caret = primary;
        theme.link = primary;
        theme.link_hover = hover;
        theme.link_active = active;
        theme.selection = primary.opacity(0.25);
        theme.accent = tint;
        theme.accent_foreground = theme.foreground;
        theme.list_active = tint;
        theme.list_active_border = primary;
        theme.table_active = tint;
        theme.table_active_border = primary;
        theme.sidebar_primary = primary;
        theme.sidebar_primary_foreground = on_primary;
        theme.sidebar_accent = tint;
        theme.sidebar_accent_foreground = theme.foreground;
        if self.color_mode == ColorMode::Oled && dark {
            theme.background = hsla(0., 0., 0., 1.);
            theme.sidebar = theme.background;
            theme.muted = hsla(0., 0., 0.06, 1.);
        }
        // Keep these application defaults when brightness is changed.
        theme.overlay = hsla(0., 0., 0., 0.58);
        theme.notification.placement = Anchor::BottomCenter;
        theme.notification.margins.bottom = gpui::px(72.);
        // Clipped dialog bodies cannot paint an outer focus ring.
        theme.focus_ring = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_system_appearance_only_when_requested() {
        for appearance in [WindowAppearance::Light, WindowAppearance::Dark] {
            assert!(ThemeMode::System.resolve(appearance) == ComponentThemeMode::from(appearance));
            assert!(ThemeMode::Light.resolve(appearance) == ComponentThemeMode::Light);
            assert!(ThemeMode::Dark.resolve(appearance) == ComponentThemeMode::Dark);
        }
    }

    #[test]
    fn accepts_rgb_colors_and_rejects_malformed_input() {
        for (input, expected) in [
            (" #aBc ", "#AABBCC"),
            ("2563eb", "#2563EB"),
            ("#000", "#000000"),
            ("#fff", "#FFFFFF"),
        ] {
            assert_eq!(
                accent_color_hex(parse_accent_color(input).unwrap()),
                expected
            );
        }
        for input in [
            "",
            "#",
            "#12",
            "#12345",
            "#12345678",
            "#gggggg",
            "蓝色",
            "é1234",
            "##123456",
        ] {
            assert!(parse_accent_color(input).is_none(), "accepted {input:?}");
        }
    }

    #[test]
    fn invalid_saved_color_falls_back_without_affecting_the_mode() {
        let theme = NearSendTheme::new(ThemeMode::Dark, ColorMode::Custom, "损坏的颜色");
        assert_eq!(theme.seed(), brand_primary());
        assert!(theme.mode == ThemeMode::Dark);
    }

    #[test]
    fn oled_uses_black_only_in_dark_mode() {
        let settings = NearSendTheme::new(ThemeMode::System, ColorMode::Oled, "");
        let mut light = Theme::default();
        let background = light.background;
        settings.apply_colors(&mut light);
        assert_eq!(light.background, background);
        let mut dark = Theme {
            mode: ComponentThemeMode::Dark,
            ..Theme::default()
        };
        settings.apply_colors(&mut dark);
        assert_eq!(dark.background, hsla(0., 0., 0., 1.));
    }

    #[test]
    fn custom_accents_reach_controls_and_keep_button_text_readable() {
        for r in (0..=255).step_by(51) {
            for g in (0..=255).step_by(51) {
                for b in (0..=255).step_by(51) {
                    let value = format!("#{r:02X}{g:02X}{b:02X}");
                    let settings = NearSendTheme::new(ThemeMode::System, ColorMode::Custom, &value);
                    for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
                        let mut theme = Theme {
                            mode,
                            ..Theme::default()
                        };
                        settings.apply_colors(&mut theme);
                        assert_eq!(theme.primary, theme.button_primary);
                        assert_eq!(theme.primary, theme.progress_bar);
                        assert_eq!(theme.primary, theme.slider_bar);
                        assert_eq!(theme.primary, theme.ring);
                        for background in [theme.primary, theme.primary_hover, theme.primary_active]
                        {
                            let a = luminance(background);
                            let b = luminance(theme.primary_foreground);
                            let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                            assert!(contrast >= 4.5, "{value}: button text contrast {contrast}");
                        }
                    }
                }
            }
        }
    }
}
