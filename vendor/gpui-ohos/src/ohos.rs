mod credentials;
mod dispatcher;
mod display;
mod keyboard;
mod platform;
mod screen_capture;
mod text_system;
mod touch_scroll;
mod wgpu_atlas;
mod wgpu_context;
mod wgpu_renderer;
mod window;

use openharmony_ability::OpenHarmonyApp;

pub fn current_platform(app: OpenHarmonyApp, _headless: bool) -> std::rc::Rc<dyn gpui::Platform> {
    std::rc::Rc::new(
        platform::OhosPlatform::new(app)
            .inspect_err(|err| {
                log::error!("Failed to initialize OHOS platform: {}", err);
            })
            .unwrap_or_else(|_| panic!("Failed to initialize OHOS platform")),
    )
}
