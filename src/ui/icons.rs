//! SVG icon helpers.
//!
//! GPUI only paints `svg()` when `text_color` is set. Icon-only ghost buttons
//! often inherit a transparent color, which is why some NearSend icons vanish.
//! Always go through [`app_icon`] so the paint color is explicit.

use gpui::{Hsla, SharedString, Styled};
use gpui_component::{Icon, Sizable as _, Size};

pub mod paths {
    pub const ARROW_LEFT: &str = "icons/arrow-left.svg";
    pub const BOOK_OPEN: &str = "icons/book-open.svg";
    pub const CHECK: &str = "icons/check.svg";
    pub const CHEVRON_RIGHT: &str = "icons/chevron-right.svg";
    pub const COPY: &str = "icons/copy.svg";
    pub const DOWNLOAD: &str = "icons/download.svg";
    pub const EXTERNAL_LINK: &str = "icons/external-link.svg";
    pub const FILE: &str = "icons/file.svg";
    pub const FOLDER: &str = "icons/folder.svg";
    pub const GITHUB: &str = "icons/github.svg";
    pub const GLOBE: &str = "icons/globe.svg";
    pub const HEART: &str = "icons/heart.svg";
    pub const HISTORY: &str = "icons/history.svg";
    pub const IMAGE: &str = "icons/image.svg";
    pub const INBOX: &str = "icons/inbox.svg";
    pub const INFO: &str = "icons/info.svg";
    pub const LOADER: &str = "icons/loader.svg";
    pub const MONITOR: &str = "icons/monitor.svg";
    pub const MORE: &str = "icons/more-horizontal.svg";
    pub const PAUSE: &str = "icons/pause.svg";
    pub const PLAY: &str = "icons/play.svg";
    pub const PLUS: &str = "icons/plus.svg";
    pub const QR_CODE: &str = "icons/qr-code.svg";
    pub const REFRESH: &str = "icons/refresh.svg";
    pub const SEND: &str = "icons/send-horizontal.svg";
    pub const SERVER: &str = "icons/server.svg";
    pub const SETTINGS: &str = "icons/settings.svg";
    pub const SMARTPHONE: &str = "icons/smartphone.svg";
    pub const TARGET: &str = "icons/target.svg";
    pub const TRASH: &str = "icons/trash.svg";
    pub const UPLOAD: &str = "icons/upload.svg";
    pub const WIFI: &str = "icons/wifi.svg";
    pub const X: &str = "icons/x.svg";
}

/// Build an SVG icon that is guaranteed to paint.
pub fn app_icon(path: impl Into<SharedString>, size: Size, color: Hsla) -> Icon {
    Icon::default().path(path).with_size(size).text_color(color)
}
