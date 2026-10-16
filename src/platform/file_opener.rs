use std::path::Path;

#[cfg(target_env = "ohos")]
use super::openharmony::{self, NearSendPlatformExt as _};

#[cfg(target_env = "ohos")]
const FILE_SHARE_READ_MODE: u32 = 1 << 0;

#[cfg(target_env = "ohos")]
fn normalize_to_file_uri(path: &Path) -> String {
    let raw = path.to_string_lossy();
    match ohos_fileuri_binding::get_uri_from_path(raw.as_ref()) {
        Ok(uri) => uri,
        Err(_) => {
            if raw.starts_with("file://") {
                raw.to_string()
            } else {
                format!("file://{}", raw)
            }
        }
    }
}

#[cfg(target_env = "ohos")]
fn canonicalize_ohos_uri(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // `file:///path` (missing bundleName) -> build canonical URI via fileuri API.
    if let Some(rest) = trimmed.strip_prefix("file://") {
        if rest.starts_with('/') {
            if let Ok(uri) = ohos_fileuri_binding::get_uri_from_path(rest) {
                return uri;
            }
        }
        return trimmed.to_string();
    }

    // Native path input -> build canonical URI via fileuri API.
    if trimmed.starts_with('/') {
        if let Ok(uri) = ohos_fileuri_binding::get_uri_from_path(trimmed) {
            return uri;
        }
    }

    trimmed.to_string()
}

#[cfg(target_env = "ohos")]
fn activate_uri_permission(uri: &str) {
    let policies = [ohos_fileshare_binding::PolicyInfo {
        uri: uri.to_string(),
        operation_mode: FILE_SHARE_READ_MODE,
    }];
    match ohos_fileshare_binding::activate_permission(&policies) {
        Ok(failed) if failed.is_empty() => {}
        Ok(failed) => log::warn!("failed to activate saved uri permission: {failed:?}"),
        Err(error) => log::warn!("failed to activate saved uri permission: {error}"),
    }
}

#[cfg(target_env = "ohos")]
async fn open_ohos_uri(target_uri: String) -> anyhow::Result<()> {
    openharmony::app()?
        .open_file(target_uri)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(())
}

#[cfg(target_env = "ohos")]
async fn open_ohos_directory_uri(target_uri: String) -> anyhow::Result<()> {
    openharmony::app()?
        .open_directory(target_uri)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(())
}

#[cfg(target_env = "ohos")]
pub async fn open_saved_uri(uri: &str) -> anyhow::Result<()> {
    let target_uri = canonicalize_ohos_uri(uri);
    if target_uri.is_empty() {
        return Err(anyhow::anyhow!("empty uri"));
    }
    activate_uri_permission(&target_uri);
    open_ohos_uri(target_uri).await
}

#[cfg(target_env = "ohos")]
pub async fn open_saved_file(path: &Path) -> anyhow::Result<()> {
    open_saved_uri(&normalize_to_file_uri(path)).await
}

#[cfg(target_env = "ohos")]
pub async fn open_saved_directory(path: &Path) -> anyhow::Result<()> {
    let target_uri = normalize_to_file_uri(path);
    if target_uri.trim().is_empty() {
        return Err(anyhow::anyhow!("empty directory uri"));
    }
    activate_uri_permission(&target_uri);
    open_ohos_directory_uri(target_uri).await
}
