use gpui::{AssetSource, Result, SharedString};
use gpui_component_assets::Assets as ComponentAssets;
use std::borrow::Cow;

/// Asset source that adds near-send icons on top of gpui-component assets.
pub struct NearSendAssets(pub ComponentAssets);

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
struct LocalIcons;

impl AssetSource for NearSendAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let candidates = canonical_path_candidates(path);

        for candidate in &candidates {
            if let Some(file) = LocalIcons::get(candidate) {
                return Ok(Some(file.data));
            }
        }

        for candidate in &candidates {
            match self.0.load(candidate) {
                Ok(Some(bytes)) => return Ok(Some(bytes)),
                Ok(None) => {}
                Err(_) => {}
            }
        }

        Ok(None)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let candidates = canonical_path_candidates(path);
        let mut list: Vec<SharedString> = Vec::new();

        for candidate in &candidates {
            if let Ok(entries) = self.0.list(candidate) {
                for entry in entries {
                    if !list.contains(&entry) {
                        list.push(entry);
                    }
                }
            }
        }

        let normalized_prefix = normalize_icon_path(path);
        let inject_all_custom_icons = normalized_prefix.is_empty()
            || normalized_prefix == "icons"
            || normalized_prefix == "icons/";

        for custom_icon in LocalIcons::iter() {
            let custom_icon = custom_icon.as_ref();
            if inject_all_custom_icons || custom_icon.starts_with(&normalized_prefix) {
                let entry: SharedString = custom_icon.to_string().into();
                if !list.contains(&entry) {
                    list.push(entry);
                }
            }
        }

        Ok(list)
    }
}

fn canonical_path_candidates(path: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut push = |candidate: &str| {
        let value = candidate.trim().replace('\\', "/");
        let value = collapse_double_slashes(&value);
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        if !candidates.iter().any(|existing| existing == value) {
            candidates.push(value.to_string());
        }
    };

    push(path);

    let trimmed = path.trim();
    push(trimmed);

    if let Some((without_query_or_hash, _)) = trimmed.split_once(['?', '#']) {
        push(without_query_or_hash);
    }

    let normalized = normalize_icon_path(path);
    push(&normalized);
    if let Some(index) = normalized.find("icons/") {
        push(&normalized[index..]);
    }

    if let Some(stripped) = trimmed.strip_prefix("./") {
        push(stripped);
    }
    if let Some(stripped) = trimmed.strip_prefix('/') {
        push(stripped);
    }
    if let Some(stripped) = trimmed.strip_prefix("assets/") {
        push(stripped);
    }
    if let Some(stripped) = trimmed.strip_prefix("/assets/") {
        push(stripped);
    }
    if trimmed.ends_with(".svg") {
        if let Some(file_name) = trimmed
            .split('/')
            .next_back()
            .filter(|name| !name.trim().is_empty())
        {
            push(file_name);
            push(&format!("icons/{}", file_name));
        }
    }

    candidates
}

fn normalize_icon_path(path: &str) -> String {
    let mut owned = path.trim().replace('\\', "/");
    owned = collapse_double_slashes(&owned);
    let mut value = owned.as_str();

    if let Some((without_query_or_hash, _)) = value.split_once(['?', '#']) {
        value = without_query_or_hash;
    }

    if let Some(stripped) = value.strip_prefix("./") {
        value = stripped;
    }

    if let Some(stripped) = value.strip_prefix('/') {
        value = stripped;
    }

    if let Some(stripped) = value.strip_prefix("assets/") {
        value = stripped;
    }

    if let Some(index) = value.find("icons/") {
        value = &value[index..];
    }

    value.to_string()
}

fn collapse_double_slashes(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut previous_was_slash = false;
    for ch in value.chars() {
        if ch == '/' {
            if previous_was_slash {
                continue;
            }
            previous_was_slash = true;
        } else {
            previous_was_slash = false;
        }
        out.push(ch);
    }
    out
}
