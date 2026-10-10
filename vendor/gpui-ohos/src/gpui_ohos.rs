//! OpenHarmony platform support for GPUI.
//!
//! The platform backend lives outside the Zed workspace and is injected with
//! [`gpui::Application::with_platform`], matching the architecture used by
//! `gpui-mobile` for Android and iOS.

mod ohos;

pub use gpui::*;
pub use ohos::current_platform;
