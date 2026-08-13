use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};
use tokio::sync::Notify;

#[derive(Clone, Debug)]
pub struct IncomingFileMeta {
    pub file_id: String,
    pub file_name: String,
    pub file_type: String,
    pub size: u64,
    pub preview: Option<String>,
}

#[derive(Clone, Debug)]
pub enum IncomingTransferEvent {
    Prepared {
        session_id: String,
        sender_alias: String,
        sender_device_model: Option<String>,
        sender_fingerprint: String,
        files: Vec<IncomingFileMeta>,
    },
    FileReceived {
        session_id: String,
        file_id: String,
        saved_path: Option<String>,
        saved_uri: Option<String>,
        text_content: Option<String>,
    },
    FileProgress {
        session_id: String,
        file_id: String,
        bytes_received: u64,
        total_bytes: u64,
        speed_bytes_per_sec: u64,
    },
    Completed {
        session_id: String,
    },
    Cancelled {
        session_id: String,
        #[allow(dead_code)]
        reason: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IncomingTransferDecision {
    AcceptAll,
    AcceptSelected(Vec<String>),
    Decline,
}

static EVENT_QUEUE: OnceLock<Mutex<VecDeque<IncomingTransferEvent>>> = OnceLock::new();
static DECISION_MAP: OnceLock<Mutex<HashMap<String, IncomingTransferDecision>>> = OnceLock::new();
static CANCELLED_SESSIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static DECISION_NOTIFY: OnceLock<Notify> = OnceLock::new();
static EVENT_NOTIFY: OnceLock<Notify> = OnceLock::new();

fn queue() -> &'static Mutex<VecDeque<IncomingTransferEvent>> {
    EVENT_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn decision_map() -> &'static Mutex<HashMap<String, IncomingTransferDecision>> {
    DECISION_MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cancelled_sessions() -> &'static Mutex<HashSet<String>> {
    CANCELLED_SESSIONS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn decision_notify() -> &'static Notify {
    DECISION_NOTIFY.get_or_init(Notify::new)
}

fn event_notify() -> &'static Notify {
    EVENT_NOTIFY.get_or_init(Notify::new)
}

pub fn push_incoming_event(event: IncomingTransferEvent) {
    if let Ok(mut q) = queue().lock() {
        q.push_back(event);
    }
    event_notify().notify_one();
}

pub fn drain_incoming_events() -> Vec<IncomingTransferEvent> {
    if let Ok(mut q) = queue().lock() {
        q.drain(..).collect()
    } else {
        Vec::new()
    }
}

pub fn submit_incoming_decision(session_id: impl Into<String>, decision: IncomingTransferDecision) {
    if let Ok(mut map) = decision_map().lock() {
        map.insert(session_id.into(), decision);
    }
    decision_notify().notify_one();
}

pub fn request_incoming_cancel(session_id: impl Into<String>) {
    let session_id = session_id.into();
    if session_id.is_empty() {
        return;
    }
    if let Ok(mut sessions) = cancelled_sessions().lock() {
        sessions.insert(session_id.clone());
    }
    submit_incoming_decision(session_id, IncomingTransferDecision::Decline);
}

pub fn is_incoming_cancel_requested(session_id: &str) -> bool {
    if let Ok(sessions) = cancelled_sessions().lock() {
        sessions.contains(session_id)
    } else {
        false
    }
}

pub fn clear_incoming_cancel(session_id: &str) {
    if let Ok(mut sessions) = cancelled_sessions().lock() {
        sessions.remove(session_id);
    }
}

pub async fn wait_incoming_decision(session_id: &str) -> IncomingTransferDecision {
    loop {
        if let Ok(mut map) = decision_map().lock() {
            if let Some(decision) = map.remove(session_id) {
                return decision;
            }
        }
        decision_notify().notified().await;
    }
}

pub async fn wait_for_incoming_event() {
    event_notify().notified().await;
}
