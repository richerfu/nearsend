//! Send transfer flow logic extracted from HomePage to keep home/mod.rs maintainable.

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

impl HomePage {
    /// Execute the send flow: build entries from selected_files, spawn thread with tokio runtime.
    pub(super) fn execute_send(
        &mut self,
        ip: String,
        port: u16,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use localsend::http::dto::{ProtocolType, RegisterDto};

        #[derive(Clone)]
        enum SendFileEntry {
            Text {
                content: String,
            },
            File {
                path: std::path::PathBuf,
                name: String,
                size: u64,
                file_type: String,
            },
        }

        let files: Vec<SendFileEntry> = self
            .send_state
            .selected_files
            .iter()
            .map(|f| {
                if let Some(text) = &f.text_content {
                    SendFileEntry::Text {
                        content: text.clone(),
                    }
                } else {
                    SendFileEntry::File {
                        path: f.path.clone(),
                        name: f.name.clone(),
                        size: f.size,
                        file_type: f.file_type.clone(),
                    }
                }
            })
            .collect();
        if let Err(message) = self.validate_before_send(&ip, port, files.len()) {
            self.open_simple_notice_dialog(&message, window, cx);
            return;
        }
        self.send_state.last_send_ip = Some(ip.clone());
        self.send_state.last_send_port = Some(port);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.send_state.active_send_cancel_flag = Some(cancel_flag.clone());
        let active_context = Arc::new(Mutex::new(super::send_state::ActiveSendContext {
            ip: ip.clone(),
            port,
            scheme: None,
            session_id: None,
        }));
        self.send_state.active_send_context = Some(active_context.clone());
        let is_single_text_message =
            files.len() == 1 && matches!(files.first(), Some(SendFileEntry::Text { .. }));
        let has_binary_file = files
            .iter()
            .any(|entry| matches!(entry, SendFileEntry::File { .. }));
        self.send_state.pending_send = true;
        self.send_state.session_status = SendSessionStatus::Preparing;
        self.send_state.session_status_text = Some("正在准备发送...".to_string());
        if has_binary_file {
            self.navigate_to(routes::TRANSFER_PROGRESS, cx);
            window.refresh();
        }

        // Build RegisterDto from our client info
        let client_info = self.app_state.read(cx).client_info.clone();
        let our_info = if let Some(info) = client_info {
            RegisterDto {
                alias: info.alias,
                version: info.version,
                device_model: info.device_model,
                device_type: info.device_type,
                token: info.token,
                port: self.receive_state.server_port,
                protocol: ProtocolType::Http,
                has_web_interface: false,
            }
        } else {
            log::error!("No client info available for send");
            return;
        };

        // Grab cert so we can create a fresh LsHttpClient in the transfer task
        let cert = self.app_state.read(cx).cert.clone();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();
        let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel::<SendUiMessage>();
        let window_handle = window.window_handle();
        let home_entity = cx.entity();
        let sent_files_for_history = self.send_state.selected_files.clone();
        let target_device_name = self
            .send_state
            .target_device
            .as_ref()
            .map(|d| d.alias.clone())
            .unwrap_or_else(|| format!("{}:{}", ip, port));
        let target_device_name_for_transfer = target_device_name.clone();
        let history_state = self.history_state.clone();
        let transfer_state = self.transfer_state.read(cx).clone();
        let transfer_id = if has_binary_file {
            Some(uuid::Uuid::new_v4().to_string())
        } else {
            None
        };
        self.send_state.active_transfer_id = transfer_id.clone();
        log::info!(
            "Starting send to {}:{} with {} files",
            ip,
            port,
            files.len()
        );

        // Spawn on the shared tokio runtime
        tokio_handle.spawn(async move {
            // ===== Compatibility path for official LocalSend app (macOS / mobile): v2 endpoints + v2 DTO =====
            #[derive(Clone, serde::Serialize)]
            #[serde(rename_all = "lowercase")]
            enum V2Protocol {
                Http,
                Https,
            }

            #[derive(Clone, serde::Serialize)]
            #[serde(rename_all = "lowercase")]
            enum V2DeviceType {
                Mobile,
                Desktop,
                Web,
                Headless,
                Server,
            }

            #[derive(Clone, serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct V2InfoRegisterDto {
                alias: String,
                version: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                device_model: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                device_type: Option<V2DeviceType>,
                fingerprint: String,
                port: u16,
                protocol: V2Protocol,
                download: bool,
            }

            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct V2FileDto {
                id: String,
                file_name: String,
                size: u64,
                file_type: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                preview: Option<String>,
            }

            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct V2PrepareUploadRequestDto {
                info: V2InfoRegisterDto,
                files: std::collections::HashMap<String, V2FileDto>,
            }

            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct V2PrepareUploadResponseDto {
                session_id: String,
                files: std::collections::HashMap<String, String>,
            }

            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct V2RegisterResponseDto {
                alias: String,
                version: Option<String>,
                #[serde(default)]
                device_model: Option<String>,
                #[serde(default)]
                device_type: Option<String>,
                #[serde(default)]
                token: Option<String>,
                #[serde(default)]
                fingerprint: Option<String>,
            }

            fn to_v2_device_type(
                device_type: &Option<localsend::model::discovery::DeviceType>,
            ) -> Option<V2DeviceType> {
                match device_type {
                    Some(localsend::model::discovery::DeviceType::Mobile) => {
                        Some(V2DeviceType::Mobile)
                    }
                    Some(localsend::model::discovery::DeviceType::Desktop) => {
                        Some(V2DeviceType::Desktop)
                    }
                    Some(localsend::model::discovery::DeviceType::Web) => Some(V2DeviceType::Web),
                    Some(localsend::model::discovery::DeviceType::Headless) => {
                        Some(V2DeviceType::Headless)
                    }
                    Some(localsend::model::discovery::DeviceType::Server) => {
                        Some(V2DeviceType::Server)
                    }
                    None => None,
                }
            }

            fn from_wire_device_type(
                value: Option<&str>,
            ) -> Option<localsend::model::discovery::DeviceType> {
                match value {
                    Some("mobile") | Some("MOBILE") => {
                        Some(localsend::model::discovery::DeviceType::Mobile)
                    }
                    Some("desktop") | Some("DESKTOP") => {
                        Some(localsend::model::discovery::DeviceType::Desktop)
                    }
                    Some("web") | Some("WEB") => Some(localsend::model::discovery::DeviceType::Web),
                    Some("headless") | Some("HEADLESS") => {
                        Some(localsend::model::discovery::DeviceType::Headless)
                    }
                    Some("server") | Some("SERVER") => {
                        Some(localsend::model::discovery::DeviceType::Server)
                    }
                    _ => None,
                }
            }

            fn remember_peer(peer: crate::core::discovery::DiscoveredDevice) {
                crate::core::discovery::register_passive_device(peer);
            }

            let prepared_entries: Vec<(String, SendFileEntry)> = files
                .iter()
                .cloned()
                .map(|entry| (uuid::Uuid::new_v4().to_string(), entry))
                .collect();

            if let Some(active_transfer_id) = transfer_id.clone() {
                let mut transfer_files = Vec::new();
                for (file_id, entry) in &prepared_entries {
                    match entry {
                        SendFileEntry::Text { .. } => {}
                        SendFileEntry::File { name, size, .. } => {
                            transfer_files.push(FileTransferInfo {
                                file_id: file_id.clone(),
                                file_name: name.clone(),
                                file_size: *size,
                                bytes_transferred: 0,
                                status: TransferStatus::Pending,
                            });
                        }
                    }
                }
                let total_bytes: u64 = transfer_files.iter().map(|f| f.file_size).sum();
                transfer_state
                    .add_transfer(TransferInfo {
                        id: active_transfer_id,
                        device_name: target_device_name_for_transfer.clone(),
                        status: TransferStatus::Pending,
                        direction: TransferDirection::Send,
                        progress: 0.0,
                        bytes_sent: 0,
                        total_bytes,
                        file_name: if transfer_files.len() == 1 {
                            transfer_files
                                .first()
                                .map(|f| f.file_name.clone())
                                .unwrap_or_else(|| "发送任务".to_string())
                        } else {
                            format!("{} 个项目", transfer_files.len())
                        },
                        speed_bytes_per_sec: 0,
                        eta_seconds: None,
                        files: transfer_files,
                    })
                    .await;
                let _ = notify_tx.send(SendUiMessage::RefreshProgress);
            }

            let files_for_v2 = prepared_entries.clone();
            let mut v2_file_map = std::collections::HashMap::new();
            let mut v2_entry_map = std::collections::HashMap::new();

            for (file_id, entry) in files_for_v2 {
                let dto = match &entry {
                    SendFileEntry::Text { content } => V2FileDto {
                        id: file_id.clone(),
                        file_name: "message.txt".to_string(),
                        size: content.as_bytes().len() as u64,
                        file_type: "text/plain".to_string(),
                        preview: if is_single_text_message {
                            Some(content.clone())
                        } else {
                            None
                        },
                    },
                    SendFileEntry::File {
                        name,
                        size,
                        file_type,
                        ..
                    } => V2FileDto {
                        id: file_id.clone(),
                        file_name: name.clone(),
                        size: *size,
                        file_type: file_type.clone(),
                        preview: None,
                    },
                };
                v2_file_map.insert(file_id.clone(), dto);
                v2_entry_map.insert(file_id, entry);
            }

            let mut v2_last_err: Option<String> = None;
            let mut v2_no_upload_success = false;
            let mut remembered_peer: Option<crate::core::discovery::DiscoveredDevice> = None;
            let v2_http_client = match reqwest::Client::builder()
                .use_rustls_tls()
                .danger_accept_invalid_certs(true)
                .build()
            {
                Ok(client) => client,
                Err(e) => {
                    log::warn!("failed to create v2 compatibility client: {}", e);
                    // fall through to legacy logic
                    reqwest::Client::new()
                }
            };

            let v2_sender_info = V2InfoRegisterDto {
                alias: our_info.alias.clone(),
                version: our_info.version.clone(),
                device_model: our_info.device_model.clone(),
                device_type: to_v2_device_type(&our_info.device_type),
                // Official LocalSend v2 expects fingerprint field.
                // Reuse token as a stable unique sender identifier.
                fingerprint: our_info.token.clone(),
                port: our_info.port,
                protocol: V2Protocol::Http,
                download: false,
            };

            let v2_payload = V2PrepareUploadRequestDto {
                info: v2_sender_info.clone(),
                files: v2_file_map,
            };

            let v2_protocols = [("http", V2Protocol::Http), ("https", V2Protocol::Https)];
            let mut v2_selected_scheme: Option<&str> = None;
            let mut v2_response: Option<V2PrepareUploadResponseDto> = None;

            for (scheme, _scheme_enum) in v2_protocols {
                let register_url =
                    format!("{}://{}:{}/api/localsend/v2/register", scheme, ip, port);
                match v2_http_client
                    .post(&register_url)
                    .json(&v2_sender_info)
                    .send()
                    .await
                {
                    Ok(res) => {
                        if !res.status().is_success() {
                            log::warn!(
                                "v2 register failed via {} for {}:{} status={}",
                                scheme,
                                ip,
                                port,
                                res.status()
                            );
                        } else {
                            match res.json::<V2RegisterResponseDto>().await {
                                Ok(body) => {
                                    remembered_peer =
                                        Some(crate::core::discovery::DiscoveredDevice {
                                            info: ClientInfo {
                                                alias: body.alias,
                                                version: body
                                                    .version
                                                    .unwrap_or_else(|| "1.0".to_string()),
                                                device_model: body.device_model,
                                                device_type: from_wire_device_type(
                                                    body.device_type.as_deref(),
                                                ),
                                                token: body
                                                    .fingerprint
                                                    .or(body.token)
                                                    .unwrap_or_default(),
                                            },
                                            ip: ip.clone(),
                                            port,
                                            https: scheme == "https",
                                        });
                                }
                                Err(e) => {
                                    log::debug!("decode v2 register response failed: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "v2 register failed via {} for {}:{}: {}",
                            scheme,
                            ip,
                            port,
                            e
                        );
                    }
                }

                let prepare_url = format!(
                    "{}://{}:{}/api/localsend/v2/prepare-upload",
                    scheme, ip, port
                );
                let mut pin: Option<String> = None;
                let mut pin_first_attempt = true;
                loop {
                    let mut req = v2_http_client.post(&prepare_url).json(&v2_payload);
                    if let Some(pin_value) = pin.as_ref() {
                        req = req.query(&[("pin", pin_value)]);
                    }
                    match req.send().await {
                        Ok(res) => {
                            if res.status() == reqwest::StatusCode::NO_CONTENT {
                                // LocalSend message-only transfer: accepted without binary upload.
                                v2_selected_scheme = Some(scheme);
                                v2_no_upload_success = true;
                                break;
                            }
                            if !res.status().is_success() {
                                if res.status() == reqwest::StatusCode::UNAUTHORIZED {
                                    let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                                        status: SendSessionStatus::PinRequired,
                                        message: Some("需要输入接收端 PIN".to_string()),
                                    });
                                    let (pin_tx, pin_rx) = oneshot::channel::<Option<String>>();
                                    let _ = notify_tx.send(SendUiMessage::RequestPin {
                                        show_invalid_pin: !pin_first_attempt,
                                        responder: pin_tx,
                                    });
                                    pin_first_attempt = false;
                                    pin = match pin_rx.await {
                                        Ok(value) => value,
                                        Err(_) => None,
                                    };
                                    if pin.is_none() {
                                        let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                                            status: SendSessionStatus::CancelledByUser,
                                            message: Some("已取消输入 PIN".to_string()),
                                        });
                                        let _ = notify_tx.send(SendUiMessage::Notice(
                                            "已取消输入 PIN。".to_string(),
                                        ));
                                        return;
                                    }
                                    continue;
                                }
                                if matches!(
                                    res.status(),
                                    reqwest::StatusCode::FORBIDDEN
                                        | reqwest::StatusCode::CONFLICT
                                        | reqwest::StatusCode::TOO_MANY_REQUESTS
                                ) {
                                    log::warn!(
                                        "v2 prepare-upload terminal status via {} for {}:{}: {}",
                                        scheme,
                                        ip,
                                        port,
                                        res.status()
                                    );
                                    let _ =
                                        notify_tx.send(SendUiMessage::Notice(match res.status() {
                                            reqwest::StatusCode::FORBIDDEN => {
                                                "对方已拒绝本次传输请求。".to_string()
                                            }
                                            reqwest::StatusCode::CONFLICT => {
                                                "对方当前正忙，请稍后重试。".to_string()
                                            }
                                            reqwest::StatusCode::TOO_MANY_REQUESTS => {
                                                "PIN 尝试次数过多，请稍后再试。".to_string()
                                            }
                                            _ => "发送失败，请稍后重试。".to_string(),
                                        }));
                                    let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                                        status: match res.status() {
                                            reqwest::StatusCode::FORBIDDEN => {
                                                SendSessionStatus::Declined
                                            }
                                            reqwest::StatusCode::CONFLICT => {
                                                SendSessionStatus::RecipientBusy
                                            }
                                            reqwest::StatusCode::TOO_MANY_REQUESTS => {
                                                SendSessionStatus::TooManyAttempts
                                            }
                                            _ => SendSessionStatus::Failed,
                                        },
                                        message: Some(format!("发送失败（{}）", res.status())),
                                    });
                                    if let Some(active_transfer_id) = transfer_id.clone() {
                                        transfer_state
                                            .update_transfer_status(
                                                &active_transfer_id,
                                                TransferStatus::Failed,
                                            )
                                            .await;
                                        let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                                    }
                                    return;
                                }
                                let msg = format!("status={}", res.status());
                                log::warn!(
                                    "v2 prepare-upload failed via {} for {}:{}: {}",
                                    scheme,
                                    ip,
                                    port,
                                    msg
                                );
                                v2_last_err = Some(msg);
                                break;
                            }
                            match res.json::<V2PrepareUploadResponseDto>().await {
                                Ok(parsed) => {
                                    v2_selected_scheme = Some(scheme);
                                    v2_response = Some(parsed);
                                    break;
                                }
                                Err(e) => {
                                    let msg =
                                        format!("decode prepare-upload response failed: {}", e);
                                    log::warn!(
                                        "v2 prepare-upload decode failed via {} for {}:{}: {}",
                                        scheme,
                                        ip,
                                        port,
                                        msg
                                    );
                                    v2_last_err = Some(msg);
                                }
                            }
                            break;
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            log::warn!(
                                "v2 prepare-upload failed via {} for {}:{}: {}",
                                scheme,
                                ip,
                                port,
                                msg
                            );
                            v2_last_err = Some(msg);
                            break;
                        }
                    }
                }
                if v2_no_upload_success || v2_response.is_some() {
                    break;
                }
            }
            if v2_no_upload_success {
                if let Some(active_transfer_id) = transfer_id.clone() {
                    transfer_state
                        .update_transfer_status(&active_transfer_id, TransferStatus::Completed)
                        .await;
                    let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                }
                if let Some(peer) = remembered_peer.clone() {
                    remember_peer(peer);
                }
                log::info!("v2 message transfer complete (no upload required)");
                let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                    status: SendSessionStatus::Completed,
                    message: Some("发送完成".to_string()),
                });
                return;
            }

            if let (Some(scheme), Some(v2_response)) = (v2_selected_scheme, v2_response) {
                if let Ok(mut ctx) = active_context.lock() {
                    ctx.scheme = Some(scheme.to_string());
                    ctx.session_id = Some(v2_response.session_id.clone());
                }
                let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                    status: SendSessionStatus::Sending,
                    message: Some("正在发送...".to_string()),
                });
                if let Some(active_transfer_id) = transfer_id.clone() {
                    transfer_state
                        .update_transfer_status(&active_transfer_id, TransferStatus::InProgress)
                        .await;
                    let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                }
                log::info!(
                    "v2 prepare-upload succeeded for {}:{} over {}, session={}",
                    ip,
                    port,
                    scheme,
                    v2_response.session_id
                );

                if v2_response.files.is_empty() {
                    log::info!(
                        "v2 transfer complete with empty file selection, session_id={}",
                        v2_response.session_id
                    );
                    if let Some(active_transfer_id) = transfer_id.clone() {
                        transfer_state
                            .update_transfer_status(&active_transfer_id, TransferStatus::Completed)
                            .await;
                        let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                    }
                    let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                        status: SendSessionStatus::Completed,
                        message: Some("发送完成".to_string()),
                    });
                    return;
                }
                let mut v2_upload_failed = false;
                for (file_id, token) in &v2_response.files {
                    if cancel_flag.load(Ordering::Acquire) {
                        let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                            status: SendSessionStatus::CancelledByUser,
                            message: Some("已取消发送".to_string()),
                        });
                        if let Some(active_transfer_id) = transfer_id.clone() {
                            transfer_state
                                .update_transfer_status(
                                    &active_transfer_id,
                                    TransferStatus::Cancelled,
                                )
                                .await;
                            let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                        }
                        return;
                    }
                    if let Some(entry) = v2_entry_map.remove(file_id) {
                        if let Some(active_transfer_id) = transfer_id.clone() {
                            transfer_state
                                .mark_file_in_progress(&active_transfer_id, file_id)
                                .await;
                            let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                        }
                        let content_type = match &entry {
                            SendFileEntry::Text { .. } => "text/plain".to_string(),
                            SendFileEntry::File { file_type, .. } => file_type.clone(),
                        };
                        let content_length = match &entry {
                            SendFileEntry::Text { content } => content.as_bytes().len() as u64,
                            SendFileEntry::File { size, .. } => *size,
                        };
                        let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
                        let transfer_state_for_feed = transfer_state.clone();
                        let notify_tx_for_feed = notify_tx.clone();
                        let active_transfer_id_for_feed = transfer_id.clone();
                        let file_id_for_feed = file_id.clone();
                        let cancel_flag_for_feed = cancel_flag.clone();
                        tokio::task::spawn(async move {
                            let start = std::time::Instant::now();
                            match entry {
                                SendFileEntry::Text { content } => {
                                    let bytes = content.into_bytes();
                                    let bytes_len = bytes.len() as u64;
                                    if tx.send(bytes).await.is_ok() {
                                        if let Some(active_transfer_id) =
                                            active_transfer_id_for_feed.as_ref()
                                        {
                                            let elapsed = start.elapsed().as_secs_f64().max(0.001);
                                            let speed = (bytes_len as f64 / elapsed) as u64;
                                            transfer_state_for_feed
                                                .update_file_progress(
                                                    active_transfer_id,
                                                    &file_id_for_feed,
                                                    bytes_len,
                                                    speed,
                                                )
                                                .await;
                                            let _ = notify_tx_for_feed
                                                .send(SendUiMessage::RefreshProgress);
                                        }
                                    }
                                }
                                SendFileEntry::File { path, .. } => {
                                    let mut file = match tokio::fs::File::open(&path).await {
                                        Ok(file) => file,
                                        Err(e) => {
                                            log::error!("Failed to open file {:?}: {}", path, e);
                                            return;
                                        }
                                    };
                                    let mut sent = 0u64;
                                    let mut buf = vec![0u8; 64 * 1024];
                                    loop {
                                        if cancel_flag_for_feed.load(Ordering::Acquire) {
                                            break;
                                        }
                                        match tokio::io::AsyncReadExt::read(&mut file, &mut buf)
                                            .await
                                        {
                                            Ok(0) => break,
                                            Ok(n) => {
                                                sent += n as u64;
                                                if tx.send(buf[..n].to_vec()).await.is_err() {
                                                    break;
                                                }
                                                if let Some(active_transfer_id) =
                                                    active_transfer_id_for_feed.as_ref()
                                                {
                                                    let elapsed =
                                                        start.elapsed().as_secs_f64().max(0.001);
                                                    let speed = (sent as f64 / elapsed) as u64;
                                                    transfer_state_for_feed
                                                        .update_file_progress(
                                                            active_transfer_id,
                                                            &file_id_for_feed,
                                                            sent,
                                                            speed,
                                                        )
                                                        .await;
                                                    let _ = notify_tx_for_feed
                                                        .send(SendUiMessage::RefreshProgress);
                                                }
                                            }
                                            Err(e) => {
                                                log::error!(
                                                    "Failed to read file {:?}: {}",
                                                    path,
                                                    e
                                                );
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        });

                        let upload_url =
                            format!("{}://{}:{}/api/localsend/v2/upload", scheme, ip, port);
                        let upload_result = v2_http_client
                            .post(&upload_url)
                            .query(&[
                                ("sessionId", v2_response.session_id.as_str()),
                                ("fileId", file_id.as_str()),
                                ("token", token.as_str()),
                            ])
                            .header(reqwest::header::CONTENT_TYPE, content_type)
                            .header(reqwest::header::CONTENT_LENGTH, content_length.to_string())
                            .body({
                                let stream =
                                    futures_util::stream::unfold(rx, |mut rx| async move {
                                        rx.recv()
                                            .await
                                            .map(|chunk| (Ok::<Vec<u8>, anyhow::Error>(chunk), rx))
                                    });
                                reqwest::Body::wrap_stream(stream)
                            })
                            .send()
                            .await;

                        match upload_result {
                            Ok(res) if res.status().is_success() => {
                                log::info!("v2 upload succeeded for file_id={}", file_id);
                                if let Some(active_transfer_id) = transfer_id.clone() {
                                    transfer_state
                                        .mark_file_completed(&active_transfer_id, file_id)
                                        .await;
                                    let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                                }
                            }
                            Ok(res) => {
                                log::error!(
                                    "v2 upload failed for file_id={} status={}",
                                    file_id,
                                    res.status()
                                );
                                let _ = notify_tx.send(SendUiMessage::Notice(format!(
                                    "发送失败：文件上传状态异常（{}）。",
                                    res.status()
                                )));
                                let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                                    status: SendSessionStatus::Failed,
                                    message: Some(format!("发送失败（上传状态 {}）", res.status())),
                                });
                                if let Some(active_transfer_id) = transfer_id.clone() {
                                    transfer_state
                                        .mark_file_failed(&active_transfer_id, file_id)
                                        .await;
                                    transfer_state
                                        .update_transfer_status(
                                            &active_transfer_id,
                                            TransferStatus::Failed,
                                        )
                                        .await;
                                    let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                                }
                                v2_upload_failed = true;
                                break;
                            }
                            Err(e) => {
                                log::error!("v2 upload failed for file_id={}: {}", file_id, e);
                                let _ = notify_tx.send(SendUiMessage::Notice(
                                    "发送失败：上传过程中发生网络错误。".to_string(),
                                ));
                                let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                                    status: SendSessionStatus::Failed,
                                    message: Some("发送失败（上传网络错误）".to_string()),
                                });
                                if let Some(active_transfer_id) = transfer_id.clone() {
                                    transfer_state
                                        .mark_file_failed(&active_transfer_id, file_id)
                                        .await;
                                    transfer_state
                                        .update_transfer_status(
                                            &active_transfer_id,
                                            TransferStatus::Failed,
                                        )
                                        .await;
                                    let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                                }
                                v2_upload_failed = true;
                                break;
                            }
                        }
                    }
                }
                if v2_upload_failed {
                    return;
                }

                log::info!(
                    "v2 transfer complete, session_id={}",
                    v2_response.session_id
                );
                if let Some(active_transfer_id) = transfer_id.clone() {
                    transfer_state
                        .update_transfer_status(&active_transfer_id, TransferStatus::Completed)
                        .await;
                    let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                }
                if let Some(peer) = remembered_peer.clone() {
                    remember_peer(peer);
                }
                let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                    status: SendSessionStatus::Completed,
                    message: Some("发送完成".to_string()),
                });
                return;
            } else {
                log::warn!(
                    "v2 send path failed for {}:{}; fallback to legacy core client. last_error={}",
                    ip,
                    port,
                    v2_last_err.unwrap_or_else(|| "unknown".to_string())
                );
            }

            // ===== Legacy fallback path: localsend/core client =====
            // Create a fresh LsHttpClient for this transfer
            let cert = match cert {
                Some(c) => c,
                None => {
                    log::error!("No TLS cert available, cannot create transfer client");
                    let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                        status: SendSessionStatus::Failed,
                        message: Some("发送失败（缺少证书）".to_string()),
                    });
                    let _ = notify_tx.send(SendUiMessage::Notice(
                        "发送失败：本端证书不可用。".to_string(),
                    ));
                    if let Some(active_transfer_id) = transfer_id.clone() {
                        transfer_state
                            .update_transfer_status(&active_transfer_id, TransferStatus::Failed)
                            .await;
                        let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                    }
                    return;
                }
            };
            let client = match localsend::http::client::LsHttpClient::try_new(
                &cert.private_key_pem,
                &cert.cert_pem,
            ) {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Failed to create transfer client: {}", e);
                    let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                        status: SendSessionStatus::Failed,
                        message: Some("发送失败（初始化失败）".to_string()),
                    });
                    let _ = notify_tx.send(SendUiMessage::Notice(
                        "发送失败：无法初始化传输客户端。".to_string(),
                    ));
                    if let Some(active_transfer_id) = transfer_id.clone() {
                        transfer_state
                            .update_transfer_status(&active_transfer_id, TransferStatus::Failed)
                            .await;
                        let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                    }
                    return;
                }
            };

            // Build FileDto map
            let mut file_map = std::collections::HashMap::new();
            let mut entry_map = std::collections::HashMap::new();
            for (file_id, entry) in prepared_entries {
                let dto = match &entry {
                    SendFileEntry::Text { content } => localsend::model::transfer::FileDto {
                        id: file_id.clone(),
                        file_name: "message.txt".to_string(),
                        size: content.as_bytes().len() as u64,
                        file_type: "text/plain".to_string(),
                        sha256: None,
                        preview: if is_single_text_message {
                            Some(content.clone())
                        } else {
                            None
                        },
                        metadata: None,
                    },
                    SendFileEntry::File {
                        name,
                        size,
                        file_type,
                        ..
                    } => localsend::model::transfer::FileDto {
                        id: file_id.clone(),
                        file_name: name.clone(),
                        size: *size,
                        file_type: file_type.clone(),
                        sha256: None,
                        preview: None,
                        metadata: None,
                    },
                };
                file_map.insert(file_id.clone(), dto);
                entry_map.insert(file_id, entry);
            }

            let payload = localsend::http::dto::PrepareUploadRequestDto {
                info: our_info,
                files: file_map,
            };

            // Target client protocol can be HTTP or HTTPS.
            // Try HTTP first for backward compatibility, then HTTPS fallback.
            let protocols = [ProtocolType::Http, ProtocolType::Https];
            let mut selected_protocol: Option<ProtocolType> = None;
            let mut response: Option<localsend::http::dto::PrepareUploadResponseDto> = None;
            let mut last_err: Option<String> = None;
            let legacy_http_client = {
                let identity = {
                    let pem = &[
                        cert.cert_pem.as_bytes(),
                        "\n".as_bytes(),
                        cert.private_key_pem.as_bytes(),
                    ]
                    .concat();
                    reqwest::Identity::from_pem(pem)
                };
                match identity.and_then(|id| {
                    reqwest::Client::builder()
                        .use_rustls_tls()
                        .danger_accept_invalid_certs(true)
                        .identity(id)
                        .build()
                }) {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("failed to create legacy reqwest client: {}", e);
                        let _ = notify_tx.send(SendUiMessage::Notice(
                            "发送失败：无法初始化网络客户端。".to_string(),
                        ));
                        let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                            status: SendSessionStatus::Failed,
                            message: Some("发送失败（客户端初始化失败）".to_string()),
                        });
                        if let Some(active_transfer_id) = transfer_id.clone() {
                            transfer_state
                                .update_transfer_status(&active_transfer_id, TransferStatus::Failed)
                                .await;
                            let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                        }
                        return;
                    }
                }
            };

            for protocol in protocols {
                log::info!("Trying protocol={} for {}:{}", protocol.as_str(), ip, port);

                let _public_key = match client
                    .register(&protocol, &ip, port, payload.info.clone())
                    .await
                {
                    Ok(register_result) => {
                        remembered_peer = Some(crate::core::discovery::DiscoveredDevice {
                            info: ClientInfo {
                                alias: register_result.body.alias,
                                version: register_result.body.version,
                                device_model: register_result.body.device_model,
                                device_type: register_result.body.device_type,
                                token: register_result.body.token,
                            },
                            ip: ip.clone(),
                            port,
                            https: matches!(protocol, ProtocolType::Https),
                        });
                        register_result.public_key
                    }
                    Err(e) => {
                        // Some peers may reject/ignore register variants. Keep compatibility by still trying prepare-upload.
                        log::warn!(
                            "register failed via {} for {}:{}: {}",
                            protocol.as_str(),
                            ip,
                            port,
                            e
                        );
                        None
                    }
                };

                let prepare_url = format!(
                    "{}://{}:{}/api/localsend/v3/prepare-upload",
                    protocol.as_str(),
                    ip,
                    port
                );
                let mut pin: Option<String> = None;
                let mut pin_first_attempt = true;
                loop {
                    let mut req = legacy_http_client.post(&prepare_url).json(&payload);
                    if let Some(pin_value) = pin.as_ref() {
                        req = req.query(&[("pin", pin_value)]);
                    }
                    match req.send().await {
                        Ok(res) => {
                            if !res.status().is_success() {
                                if res.status() == reqwest::StatusCode::UNAUTHORIZED {
                                    let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                                        status: SendSessionStatus::PinRequired,
                                        message: Some("需要输入接收端 PIN".to_string()),
                                    });
                                    let (pin_tx, pin_rx) = oneshot::channel::<Option<String>>();
                                    let _ = notify_tx.send(SendUiMessage::RequestPin {
                                        show_invalid_pin: !pin_first_attempt,
                                        responder: pin_tx,
                                    });
                                    pin_first_attempt = false;
                                    pin = match pin_rx.await {
                                        Ok(value) => value,
                                        Err(_) => None,
                                    };
                                    if pin.is_none() {
                                        let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                                            status: SendSessionStatus::CancelledByUser,
                                            message: Some("已取消输入 PIN".to_string()),
                                        });
                                        let _ = notify_tx.send(SendUiMessage::Notice(
                                            "已取消输入 PIN。".to_string(),
                                        ));
                                        return;
                                    }
                                    continue;
                                }

                                if matches!(
                                    res.status(),
                                    reqwest::StatusCode::FORBIDDEN
                                        | reqwest::StatusCode::CONFLICT
                                        | reqwest::StatusCode::TOO_MANY_REQUESTS
                                ) {
                                    let status = res.status();
                                    log::warn!(
                                        "prepare-upload terminal status via {} for {}:{}: {}",
                                        protocol.as_str(),
                                        ip,
                                        port,
                                        status
                                    );
                                    let _ = notify_tx.send(SendUiMessage::Notice(match status {
                                        reqwest::StatusCode::FORBIDDEN => {
                                            "对方已拒绝本次传输请求。".to_string()
                                        }
                                        reqwest::StatusCode::CONFLICT => {
                                            "对方当前正忙，请稍后重试。".to_string()
                                        }
                                        reqwest::StatusCode::TOO_MANY_REQUESTS => {
                                            "PIN 尝试次数过多，请稍后再试。".to_string()
                                        }
                                        _ => "发送失败，请稍后重试。".to_string(),
                                    }));
                                    let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                                        status: match status {
                                            reqwest::StatusCode::FORBIDDEN => {
                                                SendSessionStatus::Declined
                                            }
                                            reqwest::StatusCode::CONFLICT => {
                                                SendSessionStatus::RecipientBusy
                                            }
                                            reqwest::StatusCode::TOO_MANY_REQUESTS => {
                                                SendSessionStatus::TooManyAttempts
                                            }
                                            _ => SendSessionStatus::Failed,
                                        },
                                        message: Some(format!("发送失败（{}）", status)),
                                    });
                                    if let Some(active_transfer_id) = transfer_id.clone() {
                                        transfer_state
                                            .update_transfer_status(
                                                &active_transfer_id,
                                                TransferStatus::Failed,
                                            )
                                            .await;
                                        let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                                    }
                                    return;
                                }

                                let msg = format!("status={}", res.status());
                                log::warn!(
                                    "prepare-upload failed via {} for {}:{}: {}",
                                    protocol.as_str(),
                                    ip,
                                    port,
                                    msg
                                );
                                last_err = Some(msg);
                                break;
                            }

                            match res
                                .json::<localsend::http::dto::PrepareUploadResponseDto>()
                                .await
                            {
                                Ok(parsed) => {
                                    selected_protocol = Some(protocol);
                                    response = Some(parsed);
                                    break;
                                }
                                Err(e) => {
                                    let msg =
                                        format!("decode prepare-upload response failed: {}", e);
                                    log::warn!(
                                        "prepare-upload decode failed via {} for {}:{}: {}",
                                        protocol.as_str(),
                                        ip,
                                        port,
                                        msg
                                    );
                                    last_err = Some(msg);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            log::warn!(
                                "prepare-upload failed via {} for {}:{}: {}",
                                protocol.as_str(),
                                ip,
                                port,
                                msg
                            );
                            last_err = Some(msg);
                            break;
                        }
                    }
                }
                if response.is_some() {
                    break;
                }
            }

            let (protocol, response) = match (selected_protocol, response) {
                (Some(protocol), Some(response)) => (protocol, response),
                _ => {
                    log::error!(
                        "prepare-upload failed for {}:{} after trying HTTP+HTTPS. last_error={}",
                        ip,
                        port,
                        last_err.unwrap_or_else(|| "unknown".to_string())
                    );
                    let _ = notify_tx.send(SendUiMessage::Notice(
                        "发送失败，请检查对端状态后重试。".to_string(),
                    ));
                    let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                        status: SendSessionStatus::Failed,
                        message: Some("发送失败（准备阶段）".to_string()),
                    });
                    if let Some(active_transfer_id) = transfer_id.clone() {
                        transfer_state
                            .update_transfer_status(&active_transfer_id, TransferStatus::Failed)
                            .await;
                        let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                    }
                    return;
                }
            };

            let session_id = response.session_id.clone();
            if let Ok(mut ctx) = active_context.lock() {
                ctx.scheme = Some(protocol.as_str().to_string());
                ctx.session_id = Some(session_id.clone());
            }
            let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                status: SendSessionStatus::Sending,
                message: Some("正在发送...".to_string()),
            });
            if let Some(active_transfer_id) = transfer_id.clone() {
                transfer_state
                    .update_transfer_status(&active_transfer_id, TransferStatus::InProgress)
                    .await;
                let _ = notify_tx.send(SendUiMessage::RefreshProgress);
            }
            log::info!(
                "Got session_id: {}, protocol: {}, accepted files: {}",
                session_id,
                protocol.as_str(),
                response.files.len()
            );

            // Upload each accepted file
            let mut upload_failed = false;
            for (file_id, token) in &response.files {
                if cancel_flag.load(Ordering::Acquire) {
                    let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                        status: SendSessionStatus::CancelledByUser,
                        message: Some("已取消发送".to_string()),
                    });
                    if let Some(active_transfer_id) = transfer_id.clone() {
                        transfer_state
                            .update_transfer_status(&active_transfer_id, TransferStatus::Cancelled)
                            .await;
                        let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                    }
                    return;
                }
                if let Some(entry) = entry_map.remove(file_id) {
                    if let Some(active_transfer_id) = transfer_id.clone() {
                        transfer_state
                            .mark_file_in_progress(&active_transfer_id, file_id)
                            .await;
                        let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                    }
                    let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
                    let transfer_state_for_feed = transfer_state.clone();
                    let notify_tx_for_feed = notify_tx.clone();
                    let active_transfer_id_for_feed = transfer_id.clone();
                    let file_id_for_feed = file_id.clone();
                    let cancel_flag_for_feed = cancel_flag.clone();

                    // Feed data into the channel
                    tokio::task::spawn(async move {
                        let start = std::time::Instant::now();
                        match entry {
                            SendFileEntry::Text { content } => {
                                let bytes = content.into_bytes();
                                let bytes_len = bytes.len() as u64;
                                if tx.send(bytes).await.is_ok() {
                                    if let Some(active_transfer_id) =
                                        active_transfer_id_for_feed.as_ref()
                                    {
                                        let elapsed = start.elapsed().as_secs_f64().max(0.001);
                                        let speed = (bytes_len as f64 / elapsed) as u64;
                                        transfer_state_for_feed
                                            .update_file_progress(
                                                active_transfer_id,
                                                &file_id_for_feed,
                                                bytes_len,
                                                speed,
                                            )
                                            .await;
                                        let _ =
                                            notify_tx_for_feed.send(SendUiMessage::RefreshProgress);
                                    }
                                }
                            }
                            SendFileEntry::File { path, .. } => {
                                let mut file = match tokio::fs::File::open(&path).await {
                                    Ok(file) => file,
                                    Err(e) => {
                                        log::error!("Failed to open file {:?}: {}", path, e);
                                        return;
                                    }
                                };
                                let mut sent = 0u64;
                                let mut buf = vec![0u8; 64 * 1024];
                                loop {
                                    if cancel_flag_for_feed.load(Ordering::Acquire) {
                                        break;
                                    }
                                    match tokio::io::AsyncReadExt::read(&mut file, &mut buf).await {
                                        Ok(0) => break,
                                        Ok(n) => {
                                            sent += n as u64;
                                            if tx.send(buf[..n].to_vec()).await.is_err() {
                                                break;
                                            }
                                            if let Some(active_transfer_id) =
                                                active_transfer_id_for_feed.as_ref()
                                            {
                                                let elapsed =
                                                    start.elapsed().as_secs_f64().max(0.001);
                                                let speed = (sent as f64 / elapsed) as u64;
                                                transfer_state_for_feed
                                                    .update_file_progress(
                                                        active_transfer_id,
                                                        &file_id_for_feed,
                                                        sent,
                                                        speed,
                                                    )
                                                    .await;
                                                let _ = notify_tx_for_feed
                                                    .send(SendUiMessage::RefreshProgress);
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Failed to read file {:?}: {}", path, e);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    });

                    log::info!("Uploading file_id={}", file_id);
                    if let Err(e) = client
                        .upload(
                            &protocol,
                            &ip,
                            port,
                            session_id.clone(),
                            file_id.clone(),
                            token.clone(),
                            rx,
                        )
                        .await
                    {
                        log::error!("Upload failed for file_id={}: {}", file_id, e);
                        let _ = notify_tx.send(SendUiMessage::Notice(
                            "发送失败：上传过程中发生错误。".to_string(),
                        ));
                        let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                            status: SendSessionStatus::Failed,
                            message: Some("发送失败（上传错误）".to_string()),
                        });
                        if let Some(active_transfer_id) = transfer_id.clone() {
                            transfer_state
                                .mark_file_failed(&active_transfer_id, file_id)
                                .await;
                            transfer_state
                                .update_transfer_status(&active_transfer_id, TransferStatus::Failed)
                                .await;
                            let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                        }
                        upload_failed = true;
                        break;
                    } else if let Some(active_transfer_id) = transfer_id.clone() {
                        transfer_state
                            .mark_file_completed(&active_transfer_id, file_id)
                            .await;
                        let _ = notify_tx.send(SendUiMessage::RefreshProgress);
                    }
                }
            }
            if upload_failed {
                return;
            }

            log::info!("Transfer complete, session_id={}", session_id);
            if let Some(active_transfer_id) = transfer_id.clone() {
                transfer_state
                    .update_transfer_status(&active_transfer_id, TransferStatus::Completed)
                    .await;
                let _ = notify_tx.send(SendUiMessage::RefreshProgress);
            }
            if let Some(peer) = remembered_peer {
                remember_peer(peer);
            }
            let _ = notify_tx.send(SendUiMessage::UpdateStatus {
                status: SendSessionStatus::Completed,
                message: Some("发送完成".to_string()),
            });
        });

        cx.spawn(async move |_this, cx| {
            while let Some(message) = notify_rx.recv().await {
                match message {
                    SendUiMessage::Notice(text) => {
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = home_entity.update(cx, |this, cx| {
                                this.open_simple_notice_dialog(&text, window, cx);
                            });
                        });
                    }
                    SendUiMessage::UpdateStatus { status, message } => {
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = home_entity.update(cx, |this, cx| {
                                if this.send_state.session_status
                                    == SendSessionStatus::CancelledByUser
                                    && status != SendSessionStatus::CancelledByUser
                                {
                                    return;
                                }
                                this.send_state.session_status = status;
                                this.send_state.session_status_text = message.clone();
                                this.send_state.pending_send = !matches!(
                                    status,
                                    SendSessionStatus::Idle
                                        | SendSessionStatus::Completed
                                        | SendSessionStatus::Declined
                                        | SendSessionStatus::RecipientBusy
                                        | SendSessionStatus::TooManyAttempts
                                        | SendSessionStatus::CancelledByUser
                                        | SendSessionStatus::Failed
                                );
                                if matches!(
                                    status,
                                    SendSessionStatus::Completed
                                        | SendSessionStatus::Declined
                                        | SendSessionStatus::RecipientBusy
                                        | SendSessionStatus::TooManyAttempts
                                        | SendSessionStatus::CancelledByUser
                                        | SendSessionStatus::Failed
                                ) {
                                    this.send_state.active_send_cancel_flag = None;
                                    this.send_state.active_send_context = None;
                                    this.send_state.active_transfer_id = None;
                                }

                                if status == SendSessionStatus::Completed {
                                    let timestamp = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .map(|d| d.as_secs())
                                        .unwrap_or(0);
                                    for file in &sent_files_for_history {
                                        let is_text_item = file.text_content.is_some();
                                        let message_text = if is_text_item {
                                            file.text_content
                                                .clone()
                                                .filter(|t| !t.trim().is_empty())
                                                .unwrap_or_else(|| file.name.clone())
                                        } else {
                                            file.name.clone()
                                        };
                                        let entry = HistoryEntry {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            file_name: message_text.clone(),
                                            file_size: file.size,
                                            file_path: if is_text_item {
                                                std::path::PathBuf::new()
                                            } else {
                                                file.path.clone()
                                            },
                                            file_uri: file.source_uri.clone(),
                                            kind: if is_text_item {
                                                HistoryEntryKind::Text
                                            } else {
                                                HistoryEntryKind::File
                                            },
                                            text_content: if is_text_item {
                                                Some(message_text)
                                            } else {
                                                None
                                            },
                                            direction: TransferDirection::Send,
                                            device_name: target_device_name.clone(),
                                            timestamp,
                                            status: TransferStatus::Completed,
                                        };
                                        let _ = history_state.update(cx, |state, state_cx| {
                                            state.add_entry(entry);
                                            state_cx.notify();
                                        });
                                    }
                                    if matches!(this.send_state.send_mode, SendMode::Single) {
                                        this.send_selection_state.update(cx, |state, state_cx| {
                                            state.clear();
                                            state_cx.notify();
                                        });
                                        this.sync_selected_files_from_shared(cx);
                                    }
                                }
                                cx.notify();
                            });
                            window.refresh();
                        });
                    }
                    SendUiMessage::RequestPin {
                        show_invalid_pin,
                        responder,
                    } => {
                        let _ = window_handle.update(cx, |_, window, cx| {
                            let _ = home_entity.update(cx, |this, cx| {
                                this.open_send_pin_dialog(show_invalid_pin, responder, window, cx);
                            });
                        });
                    }
                    SendUiMessage::RefreshProgress => {
                        let _ = window_handle.update(cx, |_, window, _cx| {
                            window.refresh();
                        });
                    }
                }
            }
        })
        .detach();
    }

    fn validate_before_send(&self, ip: &str, port: u16, file_count: usize) -> Result<(), String> {
        if self.send_state.pending_send {
            return Err("正在发送中，请稍后再试。".to_string());
        }
        if file_count == 0 {
            return Err("请先选择要发送的内容。".to_string());
        }
        if ip.trim().is_empty() {
            return Err("目标地址不能为空。".to_string());
        }
        if port == 0 {
            return Err("目标端口无效。".to_string());
        }
        Ok(())
    }

    pub(crate) fn poll_send_retry_event(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !crate::core::send_retry_events::take_send_retry_requested() {
            return;
        }

        if self.send_state.pending_send {
            self.open_simple_notice_dialog("当前发送未结束，暂时不能重试。", window, cx);
            return;
        }

        let Some(ip) = self.send_state.last_send_ip.clone() else {
            self.open_simple_notice_dialog("没有可重试的发送目标。", window, cx);
            return;
        };
        let Some(port) = self.send_state.last_send_port else {
            self.open_simple_notice_dialog("没有可重试的发送目标端口。", window, cx);
            return;
        };

        if self.send_state.selected_files.is_empty() {
            self.open_simple_notice_dialog("没有可重试的发送内容，请重新选择。", window, cx);
            return;
        }

        self.execute_send(ip, port, window, cx);
    }

    pub(crate) fn poll_send_cancel_event(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !crate::core::send_cancel_events::take_send_cancel_requested() {
            return;
        }

        if let Some(flag) = self.send_state.active_send_cancel_flag.as_ref() {
            flag.store(true, Ordering::Release);
        }

        self.send_state.pending_send = false;
        self.send_state.session_status = SendSessionStatus::CancelledByUser;
        self.send_state.session_status_text = Some("已取消发送".to_string());

        if let Some(transfer_id) = self.send_state.active_transfer_id.clone() {
            let transfer_state = self.transfer_state.read(cx).clone();
            self.app_state.read(cx).tokio_handle.spawn(async move {
                transfer_state
                    .update_transfer_status(&transfer_id, TransferStatus::Cancelled)
                    .await;
            });
        }

        if let Some(context) = self.send_state.active_send_context.clone() {
            self.app_state.read(cx).tokio_handle.spawn(async move {
                let snapshot = {
                    let Ok(guard) = context.lock() else {
                        return;
                    };
                    guard.clone()
                };
                let schemes: Vec<String> = if let Some(scheme) = snapshot.scheme.clone() {
                    vec![scheme]
                } else {
                    vec!["http".to_string(), "https".to_string()]
                };
                let client = reqwest::Client::new();
                for scheme in schemes {
                    let mut req = client.post(format!(
                        "{}://{}:{}/api/localsend/v2/cancel",
                        scheme, snapshot.ip, snapshot.port
                    ));
                    if let Some(session_id) = snapshot.session_id.as_ref() {
                        req = req.query(&[("sessionId", session_id)]);
                    }
                    let _ = req.send().await;
                }
            });
        }
        self.send_state.active_send_cancel_flag = None;
        self.send_state.active_send_context = None;
        self.send_state.active_transfer_id = None;
        window.refresh();
    }
}
