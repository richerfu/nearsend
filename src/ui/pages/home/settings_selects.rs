//! Settings select-state initialization.

use super::*;

impl HomePage {
    pub(super) fn init_select_states(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.language_select.is_some() {
            return;
        }

        // Language select
        let language_idx = match self.settings_state.language.as_str() {
            "简体中文" => 1,
            "English" => 2,
            "日本語" => 3,
            _ => 0,
        };
        let language_select = cx.new(|cx| {
            SelectState::new(
                vec!["系统", "简体中文", "English", "日本語"],
                Some(IndexPath::default().row(language_idx)),
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &language_select,
            window,
            |this, _, event: &SelectEvent<Vec<&'static str>>, _win, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.settings_state.language = value.to_string();
                    this.persist_settings();
                    cx.notify();
                }
            },
        )
        .detach();
        self.language_select = Some(language_select);

        let send_mode_default_idx = match self.settings_state.send_mode_default {
            SendModeSetting::Single => 0,
            SendModeSetting::Multiple => 1,
            SendModeSetting::Link => 2,
        };
        let send_mode_default_select = cx.new(|cx| {
            SelectState::new(
                vec!["单设备", "多设备", "链接分享"],
                Some(IndexPath::default().row(send_mode_default_idx)),
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &send_mode_default_select,
            window,
            |this, _, event: &SelectEvent<Vec<&'static str>>, _win, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    match *value {
                        "多设备" => this.apply_send_mode_default(SendMode::Multiple),
                        "链接分享" => this.apply_send_mode_default(SendMode::Link),
                        _ => this.apply_send_mode_default(SendMode::Single),
                    }
                    cx.notify();
                }
            },
        )
        .detach();
        self.send_mode_default_select = Some(send_mode_default_select);

        let device_type_idx =
            match normalize_device_type_label(&self.settings_state.device_type).as_str() {
                "mobile" => 0,
                "desktop" => 1,
                "web" => 2,
                "server" => 3,
                "headless" => 4,
                _ => 1,
            };
        let device_type_select = cx.new(|cx| {
            SelectState::new(
                vec!["Mobile", "Desktop", "Web", "Server", "Headless"],
                Some(IndexPath::default().row(device_type_idx)),
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &device_type_select,
            window,
            |this, _, event: &SelectEvent<Vec<&'static str>>, _win, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.settings_state.device_type = value.to_string();
                    this.sync_server_config_to_runtime(cx);
                    this.persist_settings();
                    cx.notify();
                }
            },
        )
        .detach();
        self.device_type_select = Some(device_type_select);

        let device_model_idx = match self.settings_state.device_model.trim() {
            "OpenHarmony" => 1,
            "Android" => 2,
            "iPhone" => 3,
            "iPad" => 4,
            "Windows" => 5,
            "macOS" => 6,
            "Linux" => 7,
            _ => 0,
        };
        let device_model_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    "自动",
                    "OpenHarmony",
                    "Android",
                    "iPhone",
                    "iPad",
                    "Windows",
                    "macOS",
                    "Linux",
                ],
                Some(IndexPath::default().row(device_model_idx)),
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &device_model_select,
            window,
            |this, _, event: &SelectEvent<Vec<&'static str>>, _win, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    let next_model = if *value == "自动" {
                        "".to_string()
                    } else {
                        value.to_string()
                    };
                    if this.settings_state.device_model != next_model {
                        this.settings_state.device_model = next_model;
                        this.sync_server_config_to_runtime(cx);
                        this.persist_settings();
                        cx.notify();
                    }
                }
            },
        )
        .detach();
        self.device_model_select = Some(device_model_select);

        let network_filter_mode_idx = match self.settings_state.network_filter_mode {
            NetworkFilterMode::All => 0,
            NetworkFilterMode::Whitelist => 1,
            NetworkFilterMode::Blacklist => 2,
        };
        let network_filter_mode_select = cx.new(|cx| {
            SelectState::new(
                vec!["全部", "白名单", "黑名单"],
                Some(IndexPath::default().row(network_filter_mode_idx)),
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &network_filter_mode_select,
            window,
            |this, _, event: &SelectEvent<Vec<&'static str>>, _win, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.settings_state.network_filter_mode = match *value {
                        "白名单" => NetworkFilterMode::Whitelist,
                        "黑名单" => NetworkFilterMode::Blacklist,
                        _ => NetworkFilterMode::All,
                    };
                    this.sync_server_config_to_runtime(cx);
                    this.persist_settings();
                    cx.notify();
                }
            },
        )
        .detach();
        self.network_filter_mode_select = Some(network_filter_mode_select);
    }
}
