use std::path::{Component, Path, PathBuf};
use tokio::fs::{File, OpenOptions};

#[derive(Debug, Clone)]
pub struct SavedFileLocation {
    pub native_path: PathBuf,
    pub original_uri: Option<String>,
}

pub async fn create_incoming_file(
    session_id: &str,
    wire_file_name: &str,
    default_save_directory: Option<&Path>,
) -> std::io::Result<(SavedFileLocation, File)> {
    create_incoming_file_impl(session_id, wire_file_name, default_save_directory).await
}

#[cfg(target_env = "ohos")]
async fn create_incoming_file_impl(
    _session_id: &str,
    wire_file_name: &str,
    default_save_directory: Option<&Path>,
) -> std::io::Result<(SavedFileLocation, File)> {
    if let Some(directory) = default_save_directory {
        let (save_path, file) = create_unique_file(directory, wire_file_name).await?;
        let original_uri = save_path
            .to_str()
            .and_then(|path| ohos_fileuri_binding::get_uri_from_path(path).ok());
        return Ok((
            SavedFileLocation {
                native_path: save_path,
                original_uri,
            },
            file,
        ));
    }

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

async fn create_unique_file(
    directory: &Path,
    wire_file_name: &str,
) -> std::io::Result<(PathBuf, File)> {
    let requested_path = directory.join(sanitize_relative_file_path(wire_file_name));
    if let Some(parent) = requested_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    for collision_index in 0..10_000_u32 {
        let candidate = collision_candidate(&requested_path, collision_index);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
            .await
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "unable to allocate a unique received file name",
    ))
}

fn collision_candidate(path: &Path, collision_index: u32) -> PathBuf {
    if collision_index == 0 {
        return path.to_path_buf();
    }

    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let file_name = match path.extension().and_then(|value| value.to_str()) {
        Some(extension) if !extension.is_empty() => {
            format!("{stem} ({collision_index}).{extension}")
        }
        _ => format!("{stem} ({collision_index})"),
    };
    path.with_file_name(file_name)
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

#[cfg(test)]
mod tests {
    use super::{collision_candidate, sanitize_relative_file_path};
    use std::path::{Path, PathBuf};

    #[test]
    fn sanitizes_relative_directory_without_parent_traversal() {
        assert_eq!(
            sanitize_relative_file_path("../folder/../../report.txt"),
            PathBuf::from("folder/report.txt")
        );
    }

    #[test]
    fn adds_collision_suffix_before_extension() {
        assert_eq!(
            collision_candidate(Path::new("/save/folder/report.txt"), 2),
            PathBuf::from("/save/folder/report (2).txt")
        );
    }
}
