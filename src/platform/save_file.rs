use std::path::{Component, PathBuf};
use tokio::fs::File;

#[derive(Debug, Clone)]
pub struct SavedFileLocation {
    pub native_path: PathBuf,
    pub original_uri: Option<String>,
}

pub async fn create_incoming_file(
    session_id: &str,
    wire_file_name: &str,
) -> std::io::Result<(SavedFileLocation, File)> {
    create_incoming_file_impl(session_id, wire_file_name).await
}

#[cfg(target_env = "ohos")]
async fn create_incoming_file_impl(
    _session_id: &str,
    wire_file_name: &str,
) -> std::io::Result<(SavedFileLocation, File)> {
    let suggested_name = suggested_file_name(wire_file_name);
    let (save_uri, save_path) = crate::platform::file_picker::pick_save_file(suggested_name)
        .await
        .map_err(|err| std::io::Error::other(format!("pick save file failed: {}", err)))?
        .ok_or_else(|| std::io::Error::other("save file canceled"))?;
    if let Some(parent) = save_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let file = File::create(&save_path).await?;
    Ok((
        SavedFileLocation {
            native_path: save_path,
            original_uri: Some(save_uri),
        },
        file,
    ))
}

#[cfg(not(target_env = "ohos"))]
async fn create_incoming_file_impl(
    session_id: &str,
    wire_file_name: &str,
) -> std::io::Result<(SavedFileLocation, File)> {
    let base = crate::platform::preferences_path::get_preferences_path()
        .join("near-send-received")
        .join(session_id);
    tokio::fs::create_dir_all(&base).await?;
    let file_path = base.join(sanitize_relative_file_path(wire_file_name));
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let file = File::create(&file_path).await?;
    Ok((
        SavedFileLocation {
            native_path: file_path,
            original_uri: None,
        },
        file,
    ))
}

fn sanitize_relative_file_path(name: &str) -> PathBuf {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return PathBuf::from(format!("{}.bin", uuid::Uuid::new_v4()));
    }
    let normalized = trimmed.replace('\\', "/");
    let mut safe = PathBuf::new();
    for component in PathBuf::from(&normalized).components() {
        if let Component::Normal(part) = component {
            safe.push(part);
        }
    }
    if safe.as_os_str().is_empty() {
        PathBuf::from(format!("{}.bin", uuid::Uuid::new_v4()))
    } else {
        safe
    }
}

fn suggested_file_name(name: &str) -> String {
    let safe_path = sanitize_relative_file_path(name);
    safe_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_string())
        .unwrap_or_else(|| format!("{}.bin", uuid::Uuid::new_v4()))
}
