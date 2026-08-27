use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};

static PREFERENCES_PATH: LazyLock<RwLock<Option<PathBuf>>> = LazyLock::new(|| RwLock::new(None));

pub fn set_preferences_path(preferences_path: String) -> Result<(), String> {
    let trimmed = preferences_path.trim();
    if trimmed.is_empty() {
        return Err("preferences_path is empty".to_string());
    }

    let mut guard = PREFERENCES_PATH
        .write()
        .map_err(|_| "failed to lock PREFERENCES_PATH".to_string())?;
    guard.replace(PathBuf::from(trimmed));
    Ok(())
}

pub fn get_preferences_path() -> PathBuf {
    PREFERENCES_PATH
        .read()
        .ok()
        .and_then(|guard| guard.clone())
        .unwrap_or_else(std::env::temp_dir)
}

pub fn get_preferences_file_path(file_name: &str) -> PathBuf {
    get_preferences_path().join(file_name)
}
