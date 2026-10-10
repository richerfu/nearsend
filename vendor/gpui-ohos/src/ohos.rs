mod capture_pixels;
mod credentials;
mod dispatcher;
mod display;
mod frame_request;
mod keyboard;
mod platform;
mod render_cache;
mod screen_capture;
mod task_queue;
mod text_system;
mod touch_scroll;
mod viewport;
mod wgpu_atlas;
mod wgpu_context;
mod wgpu_renderer;
mod window;
mod worker_state;
mod workers;

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
