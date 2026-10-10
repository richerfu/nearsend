//! Settings tab state and types (theme, color, receive/send/network options).
use crate::ui::theme::NearSendTheme;
pub use crate::ui::theme::{ColorMode, ThemeMode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SendModeSetting {
    #[default]
    Single,
    Multiple,
    Link,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkFilterMode {
    #[default]
    All,
    Whitelist,
    Blacklist,
}

/// Settings tab state
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsPageState {
    pub theme_mode: ThemeMode,
    pub color_mode: ColorMode,
    pub custom_theme_color: String,
    pub language: String,
    pub animations: bool,
    pub advanced: bool,
    pub quick_save: bool,
    pub quick_save_favorites: bool,
    pub require_pin: bool,
    pub receive_pin: String,
    pub destination: Option<String>,
    pub save_to_gallery: bool,
    pub auto_finish: bool,
    pub save_to_history: bool,
    pub send_mode_default: SendModeSetting,
    pub share_via_link_auto_accept: bool,
    #[serde(skip)]
    pub server_running: bool,
    #[serde(skip)]
    pub server_paused: bool,
    pub server_alias: String,
    pub server_port: u16,
    pub device_type: String,
    pub device_model: String,
    pub network_filtered: bool,
    pub network_filter_mode: NetworkFilterMode,
    pub network_filters: Vec<String>,
    pub discovery_target_subnets: Vec<String>,
    pub discovery_timeout: u32,
    pub encryption: bool,
    pub multicast_group: String,
}

impl SettingsPageState {
    pub(crate) fn theme(&self) -> NearSendTheme {
        NearSendTheme::new(self.theme_mode, self.color_mode, &self.custom_theme_color)
    }

    pub fn load_or_default() -> Self {
        let path = crate::platform::preferences_path::get_preferences_file_path("settings.json");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Self::default();
        };

        match serde_json::from_str::<Self>(&raw) {
            Ok(state) => state,
            Err(err) => {
                log::warn!("failed to parse settings file {}: {}", path.display(), err);
                Self::default()
            }
        }
    }

    pub fn persist_to_disk(&self) {
        let path = crate::platform::preferences_path::get_preferences_file_path("settings.json");
        if let Some(dir) = path.parent() {
            if let Err(err) = std::fs::create_dir_all(dir) {
                log::warn!(
                    "failed to create preferences dir {}: {}",
                    dir.display(),
                    err
                );
                return;
            }
        }

        let Ok(serialized) = serde_json::to_string_pretty(self) else {
            log::warn!("failed to serialize settings state");
            return;
        };
        if let Err(err) = std::fs::write(&path, serialized) {
            log::warn!("failed to write settings file {}: {}", path.display(), err);
        }
    }
}

impl Default for SettingsPageState {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            color_mode: ColorMode::System,
            custom_theme_color: "#5CA34B".to_string(),
            language: "System".to_string(),
            animations: true,
            advanced: false,
            quick_save: false,
            quick_save_favorites: false,
            require_pin: false,
            receive_pin: "123456".to_string(),
            destination: None,
            save_to_gallery: true,
            auto_finish: true,
            save_to_history: true,
            send_mode_default: SendModeSetting::Single,
            share_via_link_auto_accept: false,
            server_running: false,
            server_paused: false,
            server_alias: "NearSend".to_string(),
            server_port: 53317,
            device_type: "Desktop".to_string(),
            device_model: "".to_string(),
            network_filtered: false,
            network_filter_mode: NetworkFilterMode::All,
            network_filters: Vec::new(),
            discovery_target_subnets: Vec::new(),
            discovery_timeout: 900,
            encryption: false,
            multicast_group: "224.0.0.167".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::accent_color_hex;

    #[test]
    fn loads_existing_settings_without_resetting_user_preferences() {
        let settings: SettingsPageState = serde_json::from_str(
            r#"{
            "theme_mode": "dark", "color_mode": "local_send",
            "server_alias": "我的设备", "server_port": 53318, "require_pin": true
        }"#,
        )
        .unwrap();
        assert!(settings.theme_mode == ThemeMode::Dark);
        assert!(settings.color_mode == ColorMode::LocalSend);
        assert_eq!(settings.server_alias, "我的设备");
        assert_eq!(settings.server_port, 53318);
        assert!(settings.require_pin);
        assert_eq!(
            settings.custom_theme_color,
            SettingsPageState::default().custom_theme_color
        );
    }

    #[test]
    fn custom_palette_survives_settings_serialization() {
        let settings = SettingsPageState {
            theme_mode: ThemeMode::Dark,
            color_mode: ColorMode::Custom,
            custom_theme_color: "#2563EB".into(),
            ..SettingsPageState::default()
        };
        let saved = serde_json::to_string(&settings).unwrap();
        let restored: SettingsPageState = serde_json::from_str(&saved).unwrap();
        assert!(restored.theme_mode == ThemeMode::Dark);
        assert!(restored.color_mode == ColorMode::Custom);
        assert_eq!(restored.custom_theme_color, "#2563EB");
        assert_eq!(accent_color_hex(restored.theme().seed()), "#2563EB");
    }
}
