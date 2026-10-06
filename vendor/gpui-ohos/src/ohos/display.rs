use std::fmt::Debug;
use uuid::Uuid;

use openharmony_ability::OpenHarmonyApp;

use crate::{Bounds, DisplayId, Pixels, PlatformDisplay, Result, point, px, size};

#[derive(Clone)]
pub(crate) struct OhosDisplay {
    app: OpenHarmonyApp,
    id: DisplayId,
}

impl OhosDisplay {
    pub(crate) fn new(app: OpenHarmonyApp) -> Self {
        Self {
            app,
            id: DisplayId::new(0),
        }
    }
}

impl Debug for OhosDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OhosDisplay").field("id", &self.id).finish()
    }
}

impl PlatformDisplay for OhosDisplay {
    fn id(&self) -> DisplayId {
        self.id
    }

    fn uuid(&self) -> Result<Uuid> {
        // Generate a stable UUID for the display
        Ok(Uuid::from_bytes([
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01,
        ]))
    }

    fn bounds(&self) -> Bounds<Pixels> {
        let (width, height) = self.app.display_size();
        let scale = (self.app.scale() as f32).max(f32::EPSILON);
        if width > 0 && height > 0 {
            Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(width as f32 / scale), px(height as f32 / scale)),
            )
        } else {
            let content_rect = self.app.content_rect();
            Bounds::new(
                point(px(0.0), px(0.0)),
                size(
                    px(content_rect.width.max(1) as f32 / scale),
                    px(content_rect.height.max(1) as f32 / scale),
                ),
            )
        }
    }

    fn visible_bounds(&self) -> Bounds<Pixels> {
        // On OHOS, visible bounds are the same as full bounds
        self.bounds()
    }
}
