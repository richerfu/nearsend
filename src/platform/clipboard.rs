use napi_ohos::Result;
#[cfg(target_env = "ohos")]
use ohos_ability_access_control_binding::check_self_permission;
use openharmony_ability_plugin_permission::PermissionExt as _;

use super::openharmony::{self, NearSendPlatformExt as _};

const READ_PASTEBOARD_PERMISSION: &str = "ohos.permission.READ_PASTEBOARD";
#[allow(dead_code)]
const WRITE_PASTEBOARD_PERMISSION: &str = "ohos.permission.WRITE_PASTEBOARD";

fn has_permission(permission: &str) -> bool {
    #[cfg(target_env = "ohos")]
    {
        check_self_permission(permission)
    }

    #[cfg(not(target_env = "ohos"))]
    {
        let _ = permission;
        true
    }
}

pub fn has_read_clipboard_permission() -> bool {
    has_permission(READ_PASTEBOARD_PERMISSION)
}

async fn request_permissions(permissions: Vec<String>) -> Result<bool> {
    let result = openharmony::app()?.request_permission(permissions).await?;
    for item in result {
        if item.code != 0 {
            log::warn!("permission denied: {} code={}", item.permission, item.code);
            return Ok(false);
        }
    }

    Ok(true)
}

pub async fn ensure_read_clipboard_permission() -> Result<bool> {
    if has_read_clipboard_permission() {
        return Ok(true);
    }
    request_permissions(vec![READ_PASTEBOARD_PERMISSION.to_string()]).await
}

#[allow(dead_code)]
pub async fn ensure_write_clipboard_permission() -> Result<bool> {
    if has_permission(WRITE_PASTEBOARD_PERMISSION) {
        return Ok(true);
    }
    request_permissions(vec![WRITE_PASTEBOARD_PERMISSION.to_string()]).await
}

pub async fn read_clipboard_text() -> Result<String> {
    openharmony::app()?.read_clipboard_text().await
}

pub async fn write_clipboard_text(text: String) -> Result<bool> {
    openharmony::app()?.write_clipboard_text(text).await
}
