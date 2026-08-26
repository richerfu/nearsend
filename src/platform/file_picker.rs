use std::path::{Path, PathBuf};

#[cfg(target_env = "ohos")]
use napi_ohos::Error;
use napi_ohos::Result;
use openharmony_ability_plugin_files::{dialog_type, FileDialogOptions, FilesExt as _};

use super::openharmony::{self, NearSendPlatformExt as _};

#[cfg(target_env = "ohos")]
const FILE_SHARE_READ_MODE: u32 = 1 << 0;
#[cfg(target_env = "ohos")]
const FILE_SHARE_WRITE_MODE: u32 = 1 << 1;

async fn show_file_dialog(options: FileDialogOptions) -> Result<Vec<String>> {
    let response = openharmony::app()?.show_file_dialog(options).await?;
    Ok(response.files)
}

pub async fn pick_files() -> Result<Vec<String>> {
    let uris =
        show_file_dialog(FileDialogOptions::new(dialog_type::OPEN_FILE).allow_many(true)).await?;
    #[cfg(target_env = "ohos")]
    persist_uris_or_err(&uris, FILE_SHARE_READ_MODE)?;
    Ok(uris)
}

pub async fn pick_folders() -> Result<Vec<String>> {
    let uris = show_file_dialog(FileDialogOptions::new(dialog_type::OPEN_FOLDER)).await?;
    #[cfg(target_env = "ohos")]
    persist_uris_or_err(&uris, FILE_SHARE_READ_MODE)?;
    Ok(uris)
}

#[allow(dead_code)]
pub async fn pick_save_directory() -> Result<Option<PathBuf>> {
    let uri = show_file_dialog(FileDialogOptions::new(dialog_type::OPEN_FOLDER))
        .await?
        .into_iter()
        .next();
    let Some(uri) = uri else {
        return Ok(None);
    };
    #[cfg(target_env = "ohos")]
    persist_uris_or_err(
        std::slice::from_ref(&uri),
        FILE_SHARE_READ_MODE | FILE_SHARE_WRITE_MODE,
    )?;
    Ok(picker_uri_to_path(&uri))
}

#[cfg_attr(not(target_env = "ohos"), allow(dead_code))]
pub async fn pick_save_file(file_name: String) -> Result<Option<(String, PathBuf)>> {
    let uri = openharmony::app()?.pick_save_file(file_name).await?;
    if uri.trim().is_empty() {
        return Ok(None);
    }
    #[cfg(target_env = "ohos")]
    persist_uris_or_err(
        std::slice::from_ref(&uri),
        FILE_SHARE_READ_MODE | FILE_SHARE_WRITE_MODE,
    )?;
    Ok(picker_uri_to_path_with_uri(&uri))
}

#[cfg(target_env = "ohos")]
fn persist_uris_or_err(uris: &[String], operation_mode: u32) -> Result<()> {
    let policies = uris
        .iter()
        .map(|uri| uri.trim())
        .filter(|uri| !uri.is_empty())
        .map(|uri| ohos_fileshare_binding::PolicyInfo {
            uri: uri.to_string(),
            operation_mode,
        })
        .collect::<Vec<_>>();
    if policies.is_empty() {
        return Ok(());
    }
    let failed = ohos_fileshare_binding::persist_permission(&policies).map_err(|err| {
        Error::from_reason(format!("persist picker uri permission failed: {err}"))
    })?;
    if failed.is_empty() {
        Ok(())
    } else {
        Err(Error::from_reason(format!(
            "persist picker uri permission partially failed: {failed:?}"
        )))
    }
}

/// Convert picker output (URI or path) to PathBuf.
/// On OpenHarmony, prefer `ohos-fileuri-binding` to resolve URIs to native paths.
#[allow(dead_code)]
pub fn picker_uri_to_path(uri: &str) -> Option<PathBuf> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return None;
    }
    uri_to_native_path(trimmed)
}

/// Convert picker output to `(canonical_uri, native_path)`.
/// On OpenHarmony this canonicalizes to a standard file URI (bundleName + path when available).
pub fn picker_uri_to_path_with_uri(uri: &str) -> Option<(String, PathBuf)> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return None;
    }
    let native_path = uri_to_native_path(trimmed)?;
    let canonical_uri = canonicalize_uri(trimmed, &native_path);
    Some((canonical_uri, native_path))
}

#[cfg(target_env = "ohos")]
fn canonicalize_uri(original: &str, native_path: &Path) -> String {
    if original.starts_with("file://") && !original.trim_start_matches("file://").starts_with('/') {
        return original.to_string();
    }

    if let Some(path) = native_path.to_str() {
        if let Ok(uri) = ohos_fileuri_binding::get_uri_from_path(path) {
            return uri;
        }
    }

    if original.starts_with("file://") {
        original.to_string()
    } else if let Some(path) = native_path.to_str() {
        format!("file://{path}")
    } else {
        original.to_string()
    }
}

#[cfg(not(target_env = "ohos"))]
fn canonicalize_uri(original: &str, native_path: &Path) -> String {
    if original.starts_with("file://") {
        original.to_string()
    } else if let Some(path) = native_path.to_str() {
        format!("file://{path}")
    } else {
        original.to_string()
    }
}

#[cfg(target_env = "ohos")]
fn uri_to_native_path(uri: &str) -> Option<PathBuf> {
    match ohos_fileuri_binding::get_path_from_uri(uri) {
        Ok(path) => Some(PathBuf::from(path)),
        Err(err) => {
            log::warn!("failed to convert picker uri via ohos-fileuri-binding: {err}");
            if let Some(path) = uri.strip_prefix("file://") {
                return Some(PathBuf::from(path));
            }
            Some(PathBuf::from(uri))
        }
    }
}

#[cfg(not(target_env = "ohos"))]
fn uri_to_native_path(uri: &str) -> Option<PathBuf> {
    if let Some(path) = uri.strip_prefix("file://") {
        return Some(PathBuf::from(path));
    }
    Some(PathBuf::from(uri))
}
