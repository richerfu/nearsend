use crate::core::receive_events::IncomingTransferEvent;
use crate::state::transfer_state::TransferDirection;

#[derive(Clone, Debug, Default)]
pub struct ReceiveItem {
    pub file_id: String,
    pub file_name: String,
    pub file_type: String,
    pub size: u64,
    pub bytes_received: u64,
    pub speed_bytes_per_sec: u64,
    pub saved_path: Option<String>,
    pub saved_uri: Option<String>,
    pub text_content: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ReceiveSession {
    pub session_id: String,
    pub sender_alias: String,
    pub sender_device_model: Option<String>,
    pub sender_fingerprint: String,
    pub direction: TransferDirection,
    pub items: Vec<ReceiveItem>,
    pub completed: bool,
    pub cancelled: bool,
    pub decision_submitted: bool,
    pub is_message_only: bool,
    pub selected_file_ids: Vec<String>,
}

impl Default for ReceiveSession {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            sender_alias: String::new(),
            sender_device_model: None,
            sender_fingerprint: String::new(),
            direction: TransferDirection::Receive,
            items: Vec::new(),
            completed: false,
            cancelled: false,
            decision_submitted: false,
            is_message_only: false,
            selected_file_ids: Vec::new(),
        }
    }
}

#[derive(Default)]
pub struct ReceiveInboxState {
    pub active: Option<ReceiveSession>,
}

impl ReceiveInboxState {
    fn is_message_like(files: &[crate::core::receive_events::IncomingFileMeta]) -> bool {
        files.len() == 1
            && files
                .first()
                .map(|f| f.file_type.starts_with("text/") && f.preview.is_some())
                .unwrap_or(false)
    }

    pub fn apply_event(&mut self, event: IncomingTransferEvent) {
        match event {
            IncomingTransferEvent::Prepared {
                session_id,
                sender_alias,
                sender_device_model,
                sender_fingerprint,
                files,
            } => {
                let is_message_only = Self::is_message_like(&files);
                let items: Vec<ReceiveItem> = files
                    .into_iter()
                    .map(|f| ReceiveItem {
                        file_id: f.file_id,
                        file_name: f.file_name,
                        file_type: f.file_type,
                        size: f.size,
                        bytes_received: 0,
                        speed_bytes_per_sec: 0,
                        saved_path: None,
                        saved_uri: None,
                        text_content: f.preview,
                    })
                    .collect();
                self.active = Some(ReceiveSession {
                    selected_file_ids: items.iter().map(|i| i.file_id.clone()).collect(),
                    session_id,
                    sender_alias,
                    sender_device_model,
                    sender_fingerprint,
                    direction: TransferDirection::Receive,
                    items,
                    completed: false,
                    cancelled: false,
                    decision_submitted: false,
                    is_message_only,
                });
            }
            IncomingTransferEvent::FileReceived {
                session_id,
                file_id,
                saved_path,
                saved_uri,
                text_content,
            } => {
                if let Some(active) = self.active.as_mut() {
                    if active.session_id != session_id {
                        return;
                    }
                    active.decision_submitted = true;
                    if let Some(item) = active.items.iter_mut().find(|x| x.file_id == file_id) {
                        item.bytes_received = item.size;
                        item.speed_bytes_per_sec = 0;
                        item.saved_path = saved_path;
                        item.saved_uri = saved_uri;
                        if text_content.is_some() {
                            item.text_content = text_content;
                        }
                    }
                }
            }
            IncomingTransferEvent::FileProgress {
                session_id,
                file_id,
                bytes_received,
                total_bytes,
                speed_bytes_per_sec,
            } => {
                if let Some(active) = self.active.as_mut() {
                    if active.session_id != session_id {
                        return;
                    }
                    active.decision_submitted = true;
                    if let Some(item) = active.items.iter_mut().find(|x| x.file_id == file_id) {
                        if total_bytes > 0 {
                            item.size = total_bytes;
                        }
                        item.bytes_received = bytes_received.min(item.size);
                        item.speed_bytes_per_sec = speed_bytes_per_sec;
                    }
                }
            }
            IncomingTransferEvent::Completed { session_id } => {
                if let Some(active) = self.active.as_mut() {
                    if active.session_id == session_id {
                        active.decision_submitted = true;
                        active.completed = true;
                        for item in &mut active.items {
                            item.bytes_received = item.size;
                            item.speed_bytes_per_sec = 0;
                        }
                    }
                }
            }
            IncomingTransferEvent::Cancelled { session_id, .. } => {
                if let Some(active) = self.active.as_mut() {
                    if active.session_id == session_id {
                        active.cancelled = true;
                    }
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.active = None;
    }

    pub fn toggle_file_selected(&mut self, file_id: &str) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if active.decision_submitted {
            return;
        }
        if let Some(idx) = active.selected_file_ids.iter().position(|id| id == file_id) {
            active.selected_file_ids.remove(idx);
        } else if active.items.iter().any(|item| item.file_id == file_id) {
            active.selected_file_ids.push(file_id.to_string());
        }
    }

    pub fn mark_decision_submitted(&mut self) {
        if let Some(active) = self.active.as_mut() {
            active.decision_submitted = true;
        }
    }

    #[allow(dead_code)]
    pub fn selected_file_ids(&self) -> Vec<String> {
        self.active
            .as_ref()
            .map(|s| s.selected_file_ids.clone())
            .unwrap_or_default()
    }
}
