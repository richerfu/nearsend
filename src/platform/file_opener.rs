use std::path::Path;

#[cfg(target_env = "ohos")]
use super::openharmony::{self, NearSendPlatformExt as _};

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
pub async fn open_saved_uri(uri: &str) -> anyhow::Result<()> {
    let target_uri = canonicalize_ohos_uri(uri);
    if target_uri.is_empty() {
        return Err(anyhow::anyhow!("empty uri"));
    }
    openharmony::app()?
        .open_file(target_uri)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(())
}

#[cfg(not(target_env = "ohos"))]
pub fn open_saved_uri(uri: &str) -> anyhow::Result<()> {
    if let Some(path) = uri.strip_prefix("file://") {
        return open_saved_file(Path::new(path));
    }
    open_saved_file(Path::new(uri))
}

#[cfg(target_env = "ohos")]
pub async fn open_saved_file(path: &Path) -> anyhow::Result<()> {
    open_saved_uri(&normalize_to_file_uri(path)).await
}

#[cfg(not(target_env = "ohos"))]
pub fn open_saved_file(path: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(path);
        c
    };

    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path);
        c
    };

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]).arg(path);
        c
    };

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        return Err(anyhow::anyhow!(
            "open file is not supported on this platform"
        ));
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let status = cmd.status()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "open file command exited with status {}",
                status
            ))
        }
    }
}
