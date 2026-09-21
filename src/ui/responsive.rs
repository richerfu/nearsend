//! Responsive layout selection for phone and 2-in-1 windows.

use crate::GlobalOpenHarmonyApp;
use gpui::{px, App, Pixels, Window};
use napi_derive_ohos::napi;
use std::sync::{OnceLock, RwLock};

const DESKTOP_BREAKPOINT: f32 = 840.0;
const POINTER_DESKTOP_BREAKPOINT: f32 = 840.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DeviceClass {
    Phone,
    TwoInOne,
    #[default]
    Other,
}

static DEVICE_CLASS: OnceLock<RwLock<DeviceClass>> = OnceLock::new();

#[napi]
pub fn set_device_type(device_type: String) {
    let normalized = device_type.trim().to_ascii_lowercase();
    let device_class = match normalized.as_str() {
        "phone" | "default" => DeviceClass::Phone,
        "2in1" | "pc" => DeviceClass::TwoInOne,
        _ => DeviceClass::Other,
    };
    if let Ok(mut current) = DEVICE_CLASS
        .get_or_init(|| RwLock::new(DeviceClass::Other))
        .write()
    {
        *current = device_class;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    Phone,
    Desktop,
}

#[derive(Clone, Copy, Debug)]
pub struct ResponsiveLayout {
    pub mode: LayoutMode,
    pub content_width: Pixels,
    pub page_padding: Pixels,
}

impl ResponsiveLayout {
    pub fn current(window: &Window, cx: &App) -> Self {
        let width = f32::from(window.viewport_size().width);
        let has_pointer = cx
            .try_global::<GlobalOpenHarmonyApp>()
            .map(|app| app.0.config().has_pointer_device)
            .unwrap_or(false);
        let device_class = DEVICE_CLASS
            .get_or_init(|| RwLock::new(DeviceClass::Other))
            .read()
            .map(|current| *current)
            .unwrap_or_default();
        let mode = classify_layout(width, has_pointer, device_class);

        match mode {
            LayoutMode::Phone => Self {
                mode,
                content_width: px(640.),
                page_padding: px(16.),
            },
            LayoutMode::Desktop => Self {
                mode,
                content_width: px(1180.),
                page_padding: px(24.),
            },
        }
    }

    pub fn is_desktop(self) -> bool {
        self.mode == LayoutMode::Desktop
    }

    pub fn content_max_width(self, desktop_width: f32) -> Pixels {
        match self.mode {
            LayoutMode::Phone => self.content_width,
            LayoutMode::Desktop => px(desktop_width),
        }
    }
}

fn classify_layout(width: f32, has_pointer: bool, device_class: DeviceClass) -> LayoutMode {
    let desktop_width = match device_class {
        DeviceClass::Phone => f32::INFINITY,
        DeviceClass::TwoInOne => POINTER_DESKTOP_BREAKPOINT,
        DeviceClass::Other if has_pointer => POINTER_DESKTOP_BREAKPOINT,
        DeviceClass::Other => DESKTOP_BREAKPOINT,
    };

    if width >= desktop_width {
        LayoutMode::Desktop
    } else {
        LayoutMode::Phone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_layout_stays_compact() {
        assert_eq!(
            classify_layout(430.0, false, DeviceClass::Phone),
            LayoutMode::Phone
        );
        assert_eq!(
            classify_layout(1200.0, true, DeviceClass::Phone),
            LayoutMode::Phone
        );
        assert_eq!(
            classify_layout(839.0, true, DeviceClass::TwoInOne),
            LayoutMode::Phone
        );
    }

    #[test]
    fn wide_and_pointer_windows_use_desktop_layout() {
        assert_eq!(
            classify_layout(840.0, false, DeviceClass::Other),
            LayoutMode::Desktop
        );
        assert_eq!(
            classify_layout(840.0, false, DeviceClass::TwoInOne),
            LayoutMode::Desktop
        );
        assert_eq!(
            classify_layout(840.0, true, DeviceClass::Other),
            LayoutMode::Desktop
        );
    }

    #[test]
    fn content_width_can_be_specialized_for_desktop_pages() {
        let phone = ResponsiveLayout {
            mode: LayoutMode::Phone,
            content_width: px(640.),
            page_padding: px(16.),
        };
        let desktop = ResponsiveLayout {
            mode: LayoutMode::Desktop,
            content_width: px(1180.),
            page_padding: px(24.),
        };

        assert_eq!(phone.content_max_width(800.0), px(640.));
        assert_eq!(desktop.content_max_width(800.0), px(800.));
    }
}
