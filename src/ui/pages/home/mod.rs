//! Home page: three tabs (Receive, Send, Settings) with bottom navigation.
//! Uses gpui-router; history is a separate route (see history page).

mod favorites;
mod navigation;
mod receive_state;
mod receive_tab;
mod render;
mod runtime;
mod send_actions;
mod send_state;
mod send_tab;
mod send_transfer_flow;
mod settings_actions;
mod settings_selects;
mod settings_state;
mod settings_tab;

pub use receive_state::{IncomingTransferRequest, QuickSaveMode, ReceivePageState};
pub use send_state::{
    SelectedFileInfo, SendContentType, SendMode, SendPageState, SendSessionStatus,
};
pub use settings_state::{
    ColorMode, NetworkFilterMode, SendModeSetting, SettingsPageState, ThemeMode,
};

use crate::state::{
    app_state::AppState,
    device_state::DeviceState,
    history_state::{HistoryEntry, HistoryEntryKind, HistoryState},
    receive_inbox_state::ReceiveInboxState,
    send_selection_state::SendSelectionState,
    transfer_state::{
        FileTransferInfo, TransferDirection, TransferInfo, TransferState, TransferStatus,
    },
};
pub(super) use crate::ui::components::chrome::{
    content_picker_grid, content_picker_tile, dialog_title,
};
use crate::ui::icons::{app_icon, paths};
use crate::ui::routes;
use gpui::{div, hsla, prelude::*, px, AnyElement, Context, Entity, IntoElement, Window};
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use gpui_component::notification::Notification;
use gpui_component::select::{SelectEvent, SelectState};
use gpui_component::{
    h_flex, v_flex, ActiveTheme as _, IndexPath, Sizable as _, Size, StyledExt as _, WindowExt as _,
};
use gpui_router::RouterState;
use localsend::http::state::ClientInfo;
use localsend::model::discovery::DeviceType;
use std::collections::BTreeSet;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::oneshot;

/// Tab identifier for home page bottom navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabType {
    Receive,
    Send,
    Settings,
}

/// Home page: receives / send / settings tabs + bottom nav.
pub struct HomePage {
    pub(super) app_state: Entity<AppState>,
    #[allow(dead_code)]
    pub(super) device_state: Entity<DeviceState>,
    pub(super) transfer_state: Entity<TransferState>,
    pub(super) history_state: Entity<HistoryState>,
    pub(super) send_selection_state: Entity<SendSelectionState>,
    pub(super) receive_inbox_state: Entity<ReceiveInboxState>,
    pub(super) current_tab: TabType,
    services_started: bool,
    pub(super) receive_state: ReceivePageState,
    pub(super) send_state: SendPageState,
    pub(super) settings_state: SettingsPageState,
    pub(super) server_refreshing: bool,
    server_refresh_op_id: u64,
    // Select states for settings dropdowns (lazy-initialized on first render)
    pub(super) theme_select: Option<Entity<SelectState<Vec<&'static str>>>>,
    pub(super) color_select: Option<Entity<SelectState<Vec<&'static str>>>>,
    pub(super) language_select: Option<Entity<SelectState<Vec<&'static str>>>>,
    pub(super) send_mode_default_select: Option<Entity<SelectState<Vec<&'static str>>>>,
    pub(super) device_type_select: Option<Entity<SelectState<Vec<&'static str>>>>,
    pub(super) device_model_select: Option<Entity<SelectState<Vec<&'static str>>>>,
    pub(super) network_filter_mode_select: Option<Entity<SelectState<Vec<&'static str>>>>,
    // Text input state for the message input dialog
    pub(super) text_input_state: Option<Entity<TextareaState>>,
    // Input states for the send-to-address dialog
    pub(super) send_ip_input_state: Option<Entity<InputState>>,
}

impl HomePage {
    fn build_confirm_dialog_footer(
        id_prefix: &str,
        ok_text: &str,
        cancel_text: &str,
    ) -> DialogFooter {
        DialogFooter::new()
            .child(
                DialogClose::new().child(
                    Button::new(format!("{id_prefix}-cancel")).label(cancel_text.to_string()),
                ),
            )
            .child(
                DialogAction::new().child(
                    Button::new(format!("{id_prefix}-ok"))
                        .label(ok_text.to_string())
                        .primary(),
                ),
            )
    }

    fn build_alert_dialog_footer(id_prefix: &str, ok_text: &str) -> DialogFooter {
        DialogFooter::new().child(
            DialogAction::new().child(
                Button::new(format!("{id_prefix}-ok"))
                    .label(ok_text.to_string())
                    .primary(),
            ),
        )
    }

    fn build_close_footer(id_prefix: &str, text: &str) -> DialogFooter {
        DialogFooter::new().child(
            DialogClose::new()
                .child(Button::new(format!("{id_prefix}-close")).label(text.to_string())),
        )
    }

    fn is_message_like_prepared(files: &[crate::core::receive_events::IncomingFileMeta]) -> bool {
        files.len() == 1
            && files
                .first()
                .map(|f| f.file_type.starts_with("text/") && f.preview.is_some())
                .unwrap_or(false)
    }

    fn normalized_peer_token(info: &ClientInfo, ip: &str, port: u16) -> String {
        if info.token.is_empty() {
            format!("{}:{}", ip, port)
        } else {
            info.token.clone()
        }
    }

    fn normalize_peer_info(mut info: ClientInfo, ip: &str, port: u16) -> ClientInfo {
        if info.token.is_empty() {
            info.token = format!("{}:{}", ip, port);
        }
        info
    }

    fn should_display_peer(info: &ClientInfo, ip: Option<&str>) -> bool {
        let alias = info.alias.trim();
        if alias.is_empty() {
            return false;
        }
        if let Some(ip) = ip.map(str::trim).filter(|ip| !ip.is_empty()) {
            if alias == ip {
                return false;
            }
        }
        true
    }

    pub fn new(
        app_state: Entity<AppState>,
        device_state: Entity<DeviceState>,
        transfer_state: Entity<TransferState>,
        history_state: Entity<HistoryState>,
        send_selection_state: Entity<SendSelectionState>,
        receive_inbox_state: Entity<ReceiveInboxState>,
    ) -> Self {
        let alias = generate_random_alias();
        let mut receive_state = ReceivePageState::default();
        let settings_file_exists =
            crate::platform::preferences_path::get_preferences_file_path("settings.json").exists();
        let mut settings_state = SettingsPageState::load_or_default();
        if !settings_file_exists || settings_state.server_alias.trim().is_empty() {
            settings_state.server_alias = alias;
            settings_state.persist_to_disk();
        }
        receive_state.server_alias = settings_state.server_alias.clone();
        receive_state.quick_save_mode = if settings_state.quick_save {
            QuickSaveMode::On
        } else if settings_state.quick_save_favorites {
            QuickSaveMode::Favorites
        } else {
            QuickSaveMode::Off
        };

        let mut send_state = SendPageState::default();
        send_state.send_mode = match settings_state.send_mode_default {
            SendModeSetting::Single => SendMode::Single,
            SendModeSetting::Multiple => SendMode::Multiple,
            SendModeSetting::Link => SendMode::Link,
        };

        Self {
            app_state,
            device_state,
            transfer_state,
            history_state,
            send_selection_state,
            receive_inbox_state,
            current_tab: TabType::Receive,
            services_started: false,
            receive_state,
            send_state,
            settings_state,
            server_refreshing: false,
            server_refresh_op_id: 0,
            theme_select: None,
            color_select: None,
            language_select: None,
            send_mode_default_select: None,
            device_type_select: None,
            device_model_select: None,
            network_filter_mode_select: None,
            text_input_state: None,
            send_ip_input_state: None,
        }
    }

    pub(super) fn persist_settings(&self) {
        self.settings_state.persist_to_disk();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AddressInputMode {
    Label,
    IpAddress,
}

enum ClipboardPickOutcome {
    Success(String),
    Empty,
    PermissionDenied,
    ReadFailed,
}

enum PathPickOutcome {
    Success(Vec<(String, std::path::PathBuf)>),
    Cancelled,
    Failed,
}

enum SendUiMessage {
    Notice(String),
    UpdateStatus {
        status: SendSessionStatus,
        message: Option<String>,
    },
    RequestPin {
        show_invalid_pin: bool,
        responder: oneshot::Sender<Option<String>>,
    },
    RefreshProgress,
}

fn detect_primary_route_ipv4() -> Option<Ipv4Addr> {
    let probes = [("224.0.0.167", 53317), ("1.1.1.1", 80), ("8.8.8.8", 80)];
    for (host, port) in probes {
        let socket = match std::net::UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(_) => continue,
        };
        if socket.connect((host, port)).is_err() {
            continue;
        }
        let local = match socket.local_addr() {
            Ok(addr) => addr,
            Err(_) => continue,
        };
        if let std::net::IpAddr::V4(ip) = local.ip() {
            return Some(ip);
        }
    }
    None
}

fn detect_local_ipv4s(settings: &SettingsPageState) -> Vec<String> {
    let mut local_ips = Vec::<Ipv4Addr>::new();
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for iface in interfaces {
            if iface.is_loopback() {
                continue;
            }
            if let if_addrs::IfAddr::V4(v4) = iface.addr {
                if v4.ip.is_link_local() {
                    continue;
                }
                local_ips.push(v4.ip);
            }
        }
    }

    let primary = detect_primary_route_ipv4();
    rank_ipv4_addresses(&mut local_ips, primary);

    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for ip in local_ips {
        let ip_text = ip.to_string();
        if !ip_matches_network_filters(&ip_text, settings) {
            continue;
        }
        if seen.insert(ip_text.clone()) {
            out.push(ip_text);
        }
    }

    if out.is_empty() {
        if let Some(ip) = primary {
            out.push(ip.to_string());
        }
    }

    out
}

fn is_network_filter_active(settings: &SettingsPageState) -> bool {
    !matches!(settings.network_filter_mode, NetworkFilterMode::All)
        && !settings.network_filters.is_empty()
}

fn ip_matches_network_filters(ip: &str, settings: &SettingsPageState) -> bool {
    if matches!(settings.network_filter_mode, NetworkFilterMode::All)
        || settings.network_filters.is_empty()
    {
        return true;
    }
    let matched = settings
        .network_filters
        .iter()
        .any(|rule| wildcard_ip_match(ip, rule));
    match settings.network_filter_mode {
        NetworkFilterMode::All => true,
        NetworkFilterMode::Whitelist => matched,
        NetworkFilterMode::Blacklist => !matched,
    }
}

fn wildcard_ip_match(ip: &str, rule: &str) -> bool {
    let value = ip.trim();
    let pattern = rule.trim();
    if value.is_empty() || pattern.is_empty() {
        return false;
    }
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    value == pattern
}

fn normalize_device_type_label(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn parse_device_type(value: &str) -> Option<DeviceType> {
    match normalize_device_type_label(value).as_str() {
        "mobile" => Some(DeviceType::Mobile),
        "desktop" => Some(DeviceType::Desktop),
        "web" => Some(DeviceType::Web),
        "headless" => Some(DeviceType::Headless),
        "server" => Some(DeviceType::Server),
        _ => Some(DeviceType::Desktop),
    }
}

fn normalized_device_model(settings: &SettingsPageState) -> Option<String> {
    let text = settings.device_model.trim();
    if text.is_empty() {
        Some("OpenHarmony".to_string())
    } else {
        Some(text.to_string())
    }
}

fn rank_ipv4_addresses(list: &mut Vec<Ipv4Addr>, primary: Option<Ipv4Addr>) {
    list.sort_by(|a, b| {
        let score = |ip: &Ipv4Addr| -> i32 {
            if Some(*ip) == primary {
                10
            } else if ip.octets()[3] == 1 {
                0
            } else {
                1
            }
        };
        score(b)
            .cmp(&score(a))
            .then_with(|| a.octets().cmp(&b.octets()))
    });
}

fn generate_random_alias() -> String {
    const ADJECTIVES: &[&str] = &[
        "可爱", "漂亮", "大气", "明亮", "清爽", "聪明", "酷炫", "灵巧", "机敏", "坚定", "活力",
        "高效", "奇妙", "迅捷", "优雅", "清新", "友好", "华丽", "卓越", "英俊", "热情", "善良",
        "温柔", "神秘", "整洁", "和善", "耐心", "秀丽", "强大", "富足", "低调", "智慧", "稳重",
        "特别", "沉着", "强韧", "利落", "睿智",
    ];
    const FRUITS: &[&str] = &[
        "苹果",
        "牛油果",
        "香蕉",
        "黑莓",
        "蓝莓",
        "西兰花",
        "胡萝卜",
        "樱桃",
        "椰子",
        "葡萄",
        "柠檬",
        "生菜",
        "芒果",
        "甜瓜",
        "蘑菇",
        "洋葱",
        "橙子",
        "木瓜",
        "桃子",
        "梨子",
        "菠萝",
        "土豆",
        "南瓜",
        "覆盆子",
        "草莓",
        "番茄",
    ];

    let seed = uuid::Uuid::new_v4();
    let bytes = seed.as_bytes();
    let adj_idx = (u16::from(bytes[0]) << 8 | u16::from(bytes[1])) as usize % ADJECTIVES.len();
    let fruit_idx = (u16::from(bytes[2]) << 8 | u16::from(bytes[3])) as usize % FRUITS.len();
    format!("{}的{}", ADJECTIVES[adj_idx], FRUITS[fruit_idx])
}

fn ipv4_prefix(ip: &str) -> Option<String> {
    let mut parts = ip.split('.');
    let a: u8 = parts.next()?.parse().ok()?;
    let b: u8 = parts.next()?.parse().ok()?;
    let c: u8 = parts.next()?.parse().ok()?;
    Some(format!("{}.{}.{}", a, b, c))
}
