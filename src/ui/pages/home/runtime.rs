//! Home page runtime lifecycle, service bootstrap, and discovery wiring.

use super::*;

impl HomePage {
    #[allow(dead_code)]
    pub(super) fn send_mode_label(mode: SendMode) -> &'static str {
        match mode {
            SendMode::Single => "单设备",
            SendMode::Multiple => "多设备",
            SendMode::Link => "链接分享",
        }
    }

    #[allow(dead_code)]
    pub(super) fn send_mode_setting_label(mode: SendModeSetting) -> &'static str {
        match mode {
            SendModeSetting::Single => "单设备",
            SendModeSetting::Multiple => "多设备",
            SendModeSetting::Link => "链接分享",
        }
    }

    pub(super) fn apply_send_mode_current(&mut self, mode: SendMode) {
        self.send_state.send_mode = mode;
    }

    pub(super) fn apply_send_mode_default(&mut self, mode: SendMode) {
        self.settings_state.send_mode_default = match mode {
            SendMode::Single => SendModeSetting::Single,
            SendMode::Multiple => SendModeSetting::Multiple,
            SendMode::Link => SendModeSetting::Link,
        };
        self.persist_settings();
    }

    pub(crate) fn poll_incoming_events(&mut self, cx: &mut Context<Self>) {
        let events = crate::core::receive_events::drain_incoming_events();
        if events.is_empty() {
            return;
        }
        log::info!("poll_incoming_events: received {} events", events.len());

        let mut should_open_receive_page = false;
        let mut should_auto_finish_receive = false;
        let mut history_entries = Vec::new();
        let settings_quick_save = self.settings_state.quick_save;
        let settings_quick_save_favorites = self.settings_state.quick_save_favorites;
        let save_to_history = self.settings_state.save_to_history;
        let auto_finish = self.settings_state.auto_finish;
        let favorite_tokens = self.send_state.favorite_tokens.clone();

        self.receive_inbox_state.update(cx, |state, _| {
            for event in events {
                match &event {
                    crate::core::receive_events::IncomingTransferEvent::Prepared {
                        session_id,
                        sender_fingerprint: _sender_fingerprint,
                        files,
                        ..
                    } => {
                        let is_message_only = Self::is_message_like_prepared(files);
                        let is_favorite = favorite_tokens.contains(_sender_fingerprint);
                        if is_message_only {
                            // Align with LocalSend:
                            // Message requests (single text file with preview) are displayed in UI
                            // and are not quick-saved automatically.
                            should_open_receive_page = true;
                        }
                        let quick_save = !is_message_only
                            && (settings_quick_save
                                || (settings_quick_save_favorites && is_favorite));
                        if quick_save {
                            crate::core::receive_events::submit_incoming_decision(
                                session_id.clone(),
                                crate::core::receive_events::IncomingTransferDecision::AcceptAll,
                            );
                        } else {
                            should_open_receive_page = true;
                        }
                    }
                    crate::core::receive_events::IncomingTransferEvent::FileReceived { .. } => {
                        if let Some(active) = state.active.as_ref() {
                            let is_favorite = favorite_tokens.contains(&active.sender_fingerprint);
                            let quick_save = !active.is_message_only
                                && (settings_quick_save
                                    || (settings_quick_save_favorites && is_favorite));
                            if !quick_save {
                                should_open_receive_page = true;
                            }
                        }

                        if let crate::core::receive_events::IncomingTransferEvent::FileReceived {
                            session_id,
                            file_id,
                            saved_path,
                            saved_uri,
                            ..
                        } = &event
                        {
                            if let Some(active) = state.active.as_ref() {
                                if active.session_id == *session_id {
                                    if let Some(item) =
                                        active.items.iter().find(|x| x.file_id == *file_id)
                                    {
                                        let file_path = saved_path
                                            .as_ref()
                                            .map(std::path::PathBuf::from)
                                            .unwrap_or_default();
                                        let timestamp = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .map(|d| d.as_secs())
                                            .unwrap_or(0);
                                        if save_to_history {
                                            let is_text_item = item.file_type.starts_with("text/");
                                            let message_text = if is_text_item {
                                                item.text_content
                                                    .clone()
                                                    .filter(|t| !t.trim().is_empty())
                                                    .unwrap_or_else(|| item.file_name.clone())
                                            } else {
                                                item.file_name.clone()
                                            };
                                            history_entries.push(HistoryEntry {
                                                id: uuid::Uuid::new_v4().to_string(),
                                                file_name: message_text.clone(),
                                                file_size: item.size,
                                                file_path,
                                                file_uri: saved_uri.clone(),
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
                                                direction: TransferDirection::Receive,
                                                device_name: active.sender_alias.clone(),
                                                timestamp,
                                                status: TransferStatus::Completed,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    crate::core::receive_events::IncomingTransferEvent::Completed { .. } => {
                        let is_message_only = state
                            .active
                            .as_ref()
                            .map(|active| active.is_message_only)
                            .unwrap_or(false);
                        if auto_finish && !is_message_only {
                            should_auto_finish_receive = true;
                        }
                    }
                    _ => {}
                }
                state.apply_event(event);
            }
        });

        if should_auto_finish_receive {
            self.receive_inbox_state
                .update(cx, |state, _| state.clear());
            if RouterState::global(cx).location.pathname == routes::RECEIVE_INCOMING {
                self.navigate_to(routes::HOME, cx);
                cx.notify();
            }
        }

        for entry in history_entries {
            let _ = self.history_state.update(cx, |state, _| {
                state.add_entry(entry);
            });
        }

        if should_open_receive_page {
            log::info!(
                "poll_incoming_events: navigate to {}",
                routes::RECEIVE_INCOMING
            );
            self.navigate_to(routes::RECEIVE_INCOMING, cx);
            cx.notify();
        }
    }

    pub(super) fn sync_selected_files_from_shared(&mut self, cx: &mut Context<Self>) {
        let items = self.send_selection_state.read(cx).items().to_vec();
        let total = self.send_selection_state.read(cx).total_size();
        self.send_state.selected_files = items
            .into_iter()
            .map(|item| send_state::SelectedFileInfo {
                path: item.path,
                source_uri: item.source_uri,
                name: item.name,
                size: item.size,
                file_type: item.file_type,
                text_content: item.text_content,
            })
            .collect();
        self.send_state.selected_files_total_size = total;
    }

    /// Start the HTTP server and discovery service.
    pub(super) fn start_services(&mut self, cx: &mut Context<Self>) {
        let local_ips = detect_local_ipv4s(&self.settings_state);
        log::info!("near-send local ipv4 candidates: {:?}", local_ips);
        self.receive_state.server_ips = local_ips.clone();
        self.send_state.local_ips = local_ips;

        let fingerprint = self
            .app_state
            .read(cx)
            .cert
            .as_ref()
            .map(|cert| cert.certificate_fingerprint.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let alias = self.receive_state.server_alias.clone();
        let port = self.receive_state.server_port;
        let use_https = self.settings_state.encryption;
        let multicast_group = self.settings_state.multicast_group.clone();
        let device_model = normalized_device_model(&self.settings_state);
        let device_type = parse_device_type(&self.settings_state.device_type);
        let client_info = ClientInfo {
            alias: alias.clone(),
            version: "2.1".to_string(),
            device_model: device_model.clone(),
            device_type,
            token: fingerprint,
        };

        // Store client_info in AppState
        let app_state_entity = self.app_state.clone();
        app_state_entity.update(cx, |state, _cx| {
            state.client_info = Some(client_info.clone());
        });

        self.start_local_server(cx);

        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();
        tokio_handle.spawn(async move {
            if let Err(err) = crate::core::multicast::start_multicast_service(
                multicast_group,
                client_info.alias,
                client_info.token,
                port,
                use_https,
                device_model,
                client_info.device_type,
            )
            .await
            {
                log::warn!("multicast service failed: {}", err);
            }
        });
    }

    pub(super) fn start_local_server(&mut self, cx: &mut Context<Self>) {
        self.sync_server_config_to_runtime(cx);
        let fallback_token = self
            .app_state
            .read(cx)
            .cert
            .as_ref()
            .map(|cert| cert.certificate_fingerprint.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let client_info = self
            .app_state
            .read(cx)
            .client_info
            .clone()
            .unwrap_or_else(|| ClientInfo {
                alias: self.settings_state.server_alias.clone(),
                version: "2.1".to_string(),
                device_model: normalized_device_model(&self.settings_state),
                device_type: parse_device_type(&self.settings_state.device_type),
                token: fallback_token,
            });

        let server_entity = self.app_state.read(cx).server.clone();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();
        let cert = self.app_state.read(cx).cert.clone();
        let use_https = self.settings_state.encryption;
        match server_entity.update(cx, |server, _cx| {
            server.set_port(self.settings_state.server_port);
            server.start(client_info, use_https, cert, &tokio_handle)
        }) {
            Ok(()) => {
                self.receive_state.server_running = true;
                self.settings_state.server_running = true;
                self.settings_state.server_paused = false;
            }
            Err(e) => {
                log::error!("Failed to start server: {}", e);
                self.receive_state.server_running = false;
                self.settings_state.server_running = false;
            }
        }
    }

    pub(super) fn trigger_server_refresh_feedback(&mut self, cx: &mut Context<Self>) {
        self.server_refreshing = true;
        self.server_refresh_op_id = self.server_refresh_op_id.wrapping_add(1);
        let refresh_op_id = self.server_refresh_op_id;
        cx.notify();

        let handle = self.app_state.read(cx).tokio_handle.clone();
        let wait = handle.spawn(async move {
            tokio::time::sleep(Duration::from_millis(900)).await;
        });

        cx.spawn(async move |this, cx| {
            let _ = wait.await;
            let _ = this.update(cx, |this, cx| {
                if this.server_refresh_op_id == refresh_op_id {
                    this.server_refreshing = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn stop_local_server(&mut self, cx: &mut Context<Self>) {
        let server_entity = self.app_state.read(cx).server.clone();
        server_entity.update(cx, |server, _cx| {
            server.stop();
        });
        self.receive_state.server_running = false;
        self.settings_state.server_running = false;
    }

    pub(super) fn sync_server_config_to_runtime(&mut self, cx: &mut Context<Self>) {
        let local_ips = detect_local_ipv4s(&self.settings_state);
        self.receive_state.server_ips = local_ips.clone();
        self.send_state.local_ips = local_ips;
        self.settings_state.network_filtered = is_network_filter_active(&self.settings_state);
        self.receive_state.server_alias = self.settings_state.server_alias.clone();
        self.receive_state.server_port = self.settings_state.server_port;
        let require_pin = self.settings_state.require_pin;
        let receive_pin = self.settings_state.receive_pin.clone();
        let server_entity = self.app_state.read(cx).server.clone();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();
        let default_save_directory = self
            .settings_state
            .destination
            .as_ref()
            .map(std::path::PathBuf::from);
        server_entity.update(cx, |server, _| {
            server.set_receive_pin_config(require_pin, receive_pin, &tokio_handle);
            server.set_default_save_directory(default_save_directory, &tokio_handle);
        });

        let alias = self.settings_state.server_alias.clone();
        let device_model = normalized_device_model(&self.settings_state);
        let device_type = parse_device_type(&self.settings_state.device_type);
        let fallback_token = self
            .app_state
            .read(cx)
            .cert
            .as_ref()
            .map(|cert| cert.certificate_fingerprint.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let app_state_entity = self.app_state.clone();
        app_state_entity.update(cx, |state, _| {
            if let Some(info) = state.client_info.as_mut() {
                info.alias = alias.clone();
                info.device_model = device_model.clone();
                info.device_type = device_type;
            } else {
                state.client_info = Some(ClientInfo {
                    alias: alias.clone(),
                    version: "2.1".to_string(),
                    device_model,
                    device_type,
                    token: fallback_token.clone(),
                });
            }
        });
    }

    pub(crate) fn restart_local_server_with_current_config(&mut self, cx: &mut Context<Self>) {
        self.sync_server_config_to_runtime(cx);
        self.stop_local_server(cx);
        self.start_local_server(cx);
    }

    pub(super) fn pause_local_server(&mut self, cx: &mut Context<Self>) {
        self.stop_local_server(cx);
        self.settings_state.server_paused = true;
    }

    pub(super) fn resume_local_server(&mut self, cx: &mut Context<Self>) {
        self.sync_server_config_to_runtime(cx);
        self.start_local_server(cx);
    }

    pub(super) fn hydrate_nearby_devices_from_cache(&mut self, cx: &mut Context<Self>) {
        let own = self
            .app_state
            .read(cx)
            .client_info
            .as_ref()
            .map(|c| c.token.clone())
            .unwrap_or_default();
        let cached = crate::core::discovery::list_passive_devices(Some(&own));
        if cached.is_empty() {
            return;
        }

        let mut endpoint_map = self.send_state.nearby_endpoints.clone();
        for d in &cached {
            let key = Self::normalized_peer_token(&d.info, &d.ip, d.port);
            endpoint_map.insert(
                key,
                send_state::DeviceEndpoint {
                    ip: d.ip.clone(),
                    port: d.port,
                    https: d.https,
                },
            );
        }
        self.send_state.nearby_endpoints = endpoint_map;

        let mut info_map = std::collections::HashMap::<String, ClientInfo>::new();
        for info in self.send_state.nearby_devices.iter().cloned() {
            let endpoint_ip = self
                .send_state
                .nearby_endpoints
                .get(&info.token)
                .map(|endpoint| endpoint.ip.as_str());
            if Self::should_display_peer(&info, endpoint_ip) {
                info_map.insert(info.token.clone(), info);
            }
        }
        let mut favorites_changed = false;
        for d in cached {
            let normalized = Self::normalize_peer_info(d.info, &d.ip, d.port);
            if !Self::should_display_peer(&normalized, Some(&d.ip)) {
                continue;
            }
            favorites_changed |= self.send_state.sync_favorite_from_discovered(
                &normalized.token,
                &normalized.alias,
                &d.ip,
                d.port,
                d.https,
            );
            info_map.insert(normalized.token.clone(), normalized);
        }
        if favorites_changed {
            self.send_state.persist_favorites_if_dirty();
        }
        self.send_state.nearby_devices = info_map.into_values().collect();
        self.send_state
            .nearby_devices
            .sort_by(|a, b| a.alias.cmp(&b.alias));
    }

    pub(super) fn start_discovery_scan(&mut self, force_refresh: bool, cx: &mut Context<Self>) {
        if self.send_state.scanning {
            return;
        }
        self.send_state.has_scanned_once = true;
        self.send_state.scanning = true;
        if force_refresh {
            self.send_state.nearby_devices.clear();
            self.send_state.nearby_endpoints.clear();
            crate::core::discovery::clear_passive_devices();
        }

        let port = self.receive_state.server_port;
        let timeout_ms = self.settings_state.discovery_timeout.max(200) as u64;
        let self_fingerprint = self
            .app_state
            .read(cx)
            .client_info
            .as_ref()
            .map(|c| c.token.clone());
        let announce_info = self.app_state.read(cx).client_info.clone();
        let use_https = self.settings_state.encryption;
        let multicast_group = self.settings_state.multicast_group.clone();
        let discovery_target_subnets = self.settings_state.discovery_target_subnets.clone();
        let handle = self.app_state.read(cx).tokio_handle.clone();
        let join = handle.spawn(async move {
            if let Some(info) = announce_info {
                if let Err(err) = crate::core::multicast::send_multicast_announcement(
                    multicast_group,
                    info.alias,
                    info.token,
                    port,
                    use_https,
                    info.device_model,
                    info.device_type,
                )
                .await
                {
                    log::debug!("send multicast announcement failed: {}", err);
                }
            }
            crate::core::discovery::scan_local_network(
                port,
                use_https,
                Duration::from_millis(timeout_ms),
                self_fingerprint,
                discovery_target_subnets,
            )
            .await
        });

        cx.spawn(async move |this, cx| {
            let discovered = match join.await {
                Ok(items) => items,
                Err(err) => {
                    log::error!("discovery scan task failed: {}", err);
                    Vec::new()
                }
            };

            let _ = this.update(cx, |this, cx| {
                let mut endpoint_map = this.send_state.nearby_endpoints.clone();
                for d in &discovered {
                    let key = Self::normalized_peer_token(&d.info, &d.ip, d.port);
                    endpoint_map.insert(
                        key,
                        send_state::DeviceEndpoint {
                            ip: d.ip.clone(),
                            port: d.port,
                            https: d.https,
                        },
                    );
                }
                this.send_state.nearby_endpoints = endpoint_map;

                let mut info_map = std::collections::HashMap::<String, ClientInfo>::new();
                for info in this.send_state.nearby_devices.iter().cloned() {
                    let endpoint_ip = this
                        .send_state
                        .nearby_endpoints
                        .get(&info.token)
                        .map(|endpoint| endpoint.ip.as_str());
                    if Self::should_display_peer(&info, endpoint_ip) {
                        info_map.insert(info.token.clone(), info);
                    }
                }
                let mut favorites_changed = false;
                for d in &discovered {
                    let normalized = Self::normalize_peer_info(d.info.clone(), &d.ip, d.port);
                    if !Self::should_display_peer(&normalized, Some(&d.ip)) {
                        continue;
                    }
                    favorites_changed |= this.send_state.sync_favorite_from_discovered(
                        &normalized.token,
                        &normalized.alias,
                        &d.ip,
                        d.port,
                        d.https,
                    );
                    info_map.insert(normalized.token.clone(), normalized);
                }
                if favorites_changed {
                    this.send_state.persist_favorites_if_dirty();
                }
                this.send_state.nearby_devices = info_map.into_values().collect();
                this.send_state
                    .nearby_devices
                    .sort_by(|a, b| a.alias.cmp(&b.alias));

                for d in discovered.iter() {
                    crate::core::discovery::register_passive_device(
                        crate::core::discovery::DiscoveredDevice {
                            info: d.info.clone(),
                            ip: d.ip.clone(),
                            port: d.port,
                            https: d.https,
                        },
                    );
                }
                this.send_state.scanning = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn ensure_has_selected_files(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.send_state.selected_files.is_empty() {
            self.open_simple_notice_dialog("请先选择要发送的文件或文本", window, cx);
            false
        } else {
            true
        }
    }

    pub(super) fn open_simple_notice_dialog(
        &self,
        message: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let msg = message.to_string();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(dialog_title("提示"))
                .overlay(true)
                .w(px(320.))
                .child(div().w_full().text_sm().child(msg.clone()))
                .footer(Self::build_alert_dialog_footer("simple-notice", "确定"))
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("确定"))
        });
    }

    #[allow(dead_code)]
    pub(super) fn show_copy_success_toast(&self, window: &mut Window, cx: &mut Context<Self>) {
        struct CopySuccessToast;
        window.push_notification(
            Notification::new()
                .id::<CopySuccessToast>()
                .autohide(false)
                .content(|_, _, _| {
                    div()
                        .w_full()
                        .text_xs()
                        .text_center()
                        .child("复制成功")
                        .into_any_element()
                })
                .w(px(92.))
                .py(px(4.))
                .px(px(10.))
                .rounded_full()
                .shadow_none()
                .border_color(hsla(0.0, 0.0, 0.0, 0.0))
                .bg(hsla(0.0, 0.0, 0.12, 0.92))
                .text_color(hsla(0.0, 0.0, 1.0, 0.96)),
            cx,
        );
        let window_handle = window.window_handle();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();
        let dismiss = tokio_handle.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        });
        cx.spawn(async move |_this, cx| {
            let _ = dismiss.await;
            let _ = window_handle.update(cx, |_, window, cx| {
                window.remove_notification::<CopySuccessToast>(cx);
            });
        })
        .detach();
    }

    pub(super) fn show_clipboard_empty_toast(&self, window: &mut Window, cx: &mut Context<Self>) {
        struct ClipboardEmptyToast;
        window.push_notification(
            Notification::new()
                .id::<ClipboardEmptyToast>()
                .autohide(false)
                .content(|_, _, _| {
                    div()
                        .w_full()
                        .text_xs()
                        .text_center()
                        .child("当前剪贴板无内容")
                        .into_any_element()
                })
                .w(px(158.))
                .py(px(4.))
                .px(px(10.))
                .mb(px(22.))
                .rounded_full()
                .shadow_none()
                .border_color(hsla(0.0, 0.0, 0.0, 0.0))
                .bg(hsla(0.0, 0.0, 0.12, 0.92))
                .text_color(hsla(0.0, 0.0, 1.0, 0.96)),
            cx,
        );
        let window_handle = window.window_handle();
        let tokio_handle = self.app_state.read(cx).tokio_handle.clone();
        let dismiss = tokio_handle.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        });
        cx.spawn(async move |_this, cx| {
            let _ = dismiss.await;
            let _ = window_handle.update(cx, |_, window, cx| {
                window.remove_notification::<ClipboardEmptyToast>(cx);
            });
        })
        .detach();
    }
}
