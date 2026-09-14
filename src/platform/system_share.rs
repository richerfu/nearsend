use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use napi_derive_ohos::napi;
use tokio::sync::Notify;

/// Content handed to NearSend by the system share panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemShareRequest {
    pub uris: Vec<String>,
    pub texts: Vec<String>,
}

static REQUEST_QUEUE: OnceLock<Mutex<VecDeque<SystemShareRequest>>> = OnceLock::new();
static REQUEST_NOTIFY: OnceLock<Notify> = OnceLock::new();

fn queue() -> &'static Mutex<VecDeque<SystemShareRequest>> {
    REQUEST_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn request_notify() -> &'static Notify {
    REQUEST_NOTIFY.get_or_init(Notify::new)
}

fn normalize_request(uris: Vec<String>, texts: Vec<String>) -> Option<SystemShareRequest> {
    fn push_unique(values: Vec<String>, normalized: &mut Vec<String>) {
        for value in values {
            let value = value.trim();
            if !value.is_empty() && !normalized.iter().any(|existing| existing == value) {
                normalized.push(value.to_string());
            }
        }
    }

    let mut normalized_uris = Vec::new();
    push_unique(uris, &mut normalized_uris);
    let mut normalized_texts = Vec::new();
    push_unique(texts, &mut normalized_texts);

    if normalized_uris.is_empty() && normalized_texts.is_empty() {
        None
    } else {
        Some(SystemShareRequest {
            uris: normalized_uris,
            texts: normalized_texts,
        })
    }
}

/// Native entry point used by `EntryAbility` after Share Kit parses a share Want.
///
/// This can run before GPUI has created `AppRoot`, so requests are retained in a
/// process-local queue and consumed once the first window is ready.
#[napi]
pub fn enqueue_system_share(uris: Vec<String>, texts: Vec<String>) {
    let Some(request) = normalize_request(uris, texts) else {
        return;
    };
    if let Ok(mut requests) = queue().lock() {
        requests.push_back(request);
    } else {
        log::error!("failed to lock the pending system-share queue");
        return;
    }
    request_notify().notify_one();
}

pub fn drain_system_shares() -> Vec<SystemShareRequest> {
    if let Ok(mut requests) = queue().lock() {
        requests.drain(..).collect()
    } else {
        log::error!("failed to lock the pending system-share queue");
        Vec::new()
    }
}

pub async fn wait_for_system_share() {
    request_notify().notified().await;
}

#[cfg(test)]
mod tests {
    use super::normalize_request;

    #[test]
    fn normalizes_and_deduplicates_share_content() {
        let request = normalize_request(
            vec![
                " file://bundle/image.png ".to_string(),
                "file://bundle/image.png".to_string(),
                String::new(),
            ],
            vec![" hello ".to_string(), "hello".to_string()],
        )
        .expect("request should contain normalized content");

        assert_eq!(request.uris, ["file://bundle/image.png"]);
        assert_eq!(request.texts, ["hello"]);
    }

    #[test]
    fn drops_an_empty_share_request() {
        assert!(normalize_request(vec!["  ".to_string()], vec![String::new()]).is_none());
    }
}
