use std::path::PathBuf;

/// Canonical selected-send entry shared by send tab and the edit page.
#[derive(Clone, Debug)]
pub struct SendSelectionItem {
    pub path: PathBuf,
    pub source_uri: Option<String>,
    pub name: String,
    pub size: u64,
    pub file_type: String,
    pub text_content: Option<String>,
}

impl SendSelectionItem {
    pub fn from_text(text: String) -> Self {
        let size = text.len() as u64;
        let name = text.clone();
        Self {
            path: PathBuf::from(name.clone()),
            source_uri: None,
            name,
            size,
            file_type: "text/plain".to_string(),
            text_content: Some(text),
        }
    }
}

#[derive(Default)]
pub struct SendSelectionState {
    items: Vec<SendSelectionItem>,
    revision: u64,
}

impl SendSelectionState {
    pub fn items(&self) -> &[SendSelectionItem] {
        &self.items
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub fn total_size(&self) -> u64 {
        self.items.iter().map(|f| f.size).sum()
    }

    pub fn clear(&mut self) {
        if !self.items.is_empty() {
            self.items.clear();
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn add_text(&mut self, text: String) {
        self.items.push(SendSelectionItem::from_text(text));
        self.revision = self.revision.wrapping_add(1);
    }

    #[allow(dead_code)]
    pub fn add_paths_recursive(&mut self, paths: Vec<PathBuf>) -> usize {
        let mut added = 0usize;
        for path in paths {
            if path.is_dir() {
                added += self.add_directory_recursive(path);
            } else {
                added += self.add_single_file(path);
            }
        }
        added
    }

    pub fn add_picker_paths_recursive(&mut self, picked: Vec<(String, PathBuf)>) -> usize {
        let mut added = 0usize;
        for (uri, path) in picked {
            if path.is_dir() {
                added += self.add_directory_recursive(path);
            } else {
                added += self.add_single_file_with_uri(path, Some(uri));
            }
        }
        added
    }

    pub fn update_text(&mut self, index: usize, text: String) {
        if let Some(item) = self.items.get_mut(index) {
            let size = text.len() as u64;
            item.name = text.clone();
            item.path = PathBuf::from(text.clone());
            item.source_uri = None;
            item.file_type = "text/plain".to_string();
            item.size = size;
            item.text_content = Some(text);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn add_directory_recursive(&mut self, root: PathBuf) -> usize {
        let root_name = root
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "folder".to_string());
        self.add_directory_entries(&root, &root, &root_name)
    }

    fn add_directory_entries(
        &mut self,
        root: &PathBuf,
        current: &PathBuf,
        root_name: &str,
    ) -> usize {
        let mut added = 0usize;
        let Ok(entries) = std::fs::read_dir(current) else {
            return 0;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                added += self.add_directory_entries(root, &path, root_name);
            } else if path.is_file() {
                let Ok(relative) = path.strip_prefix(root) else {
                    continue;
                };
                let relative_name = relative.to_string_lossy().replace('\\', "/");
                let display_name = format!("{}/{}", root_name, relative_name);
                added += self.add_file_with_name(path, None, display_name);
            }
        }
        added
    }

    #[allow(dead_code)]
    fn add_single_file(&mut self, path: PathBuf) -> usize {
        self.add_single_file_with_uri(path, None)
    }

    fn add_single_file_with_uri(&mut self, path: PathBuf, source_uri: Option<String>) -> usize {
        let name = path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        self.add_file_with_name(path, source_uri, name)
    }

    fn add_file_with_name(
        &mut self,
        path: PathBuf,
        source_uri: Option<String>,
        display_name: String,
    ) -> usize {
        if !path.is_file() || self.items.iter().any(|item| item.path == path) {
            return 0;
        }
        let metadata = match std::fs::metadata(&path) {
            Ok(meta) => meta,
            Err(_) => return 0,
        };
        self.items.push(SendSelectionItem {
            path,
            source_uri,
            name: display_name.clone(),
            size: metadata.len(),
            file_type: infer_file_type(&display_name),
            text_content: None,
        });
        self.revision = self.revision.wrapping_add(1);
        1
    }
}

#[cfg(test)]
mod tests {
    use super::SendSelectionState;

    #[test]
    fn revision_tracks_same_size_edits_and_actual_removals() {
        let mut state = SendSelectionState::default();
        state.clear();
        state.remove(0);
        assert_eq!(state.revision(), 0);

        state.add_text("old".into());
        let revision = state.revision();
        let total_size = state.total_size();
        state.update_text(0, "new".into());
        assert_ne!(state.revision(), revision);
        assert_eq!(state.total_size(), total_size);

        let revision = state.revision();
        state.update_text(99, "ignored".into());
        state.remove(99);
        assert_eq!(state.revision(), revision);
        state.remove(0);
        assert_ne!(state.revision(), revision);

        state.add_text("last".into());
        let revision = state.revision();
        state.clear();
        assert_ne!(state.revision(), revision);
        assert!(state.items().is_empty());
    }
}

fn infer_file_type(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.ends_with(".txt")
        || lower.ends_with(".md")
        || lower.ends_with(".json")
        || lower.ends_with(".csv")
        || lower.ends_with(".log")
    {
        "text/plain".to_string()
    } else if lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".bmp")
        || lower.ends_with(".svg")
    {
        "image/*".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}
