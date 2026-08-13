use crate::core::cert::CertPair;
use crate::core::receive_events::{
    clear_incoming_cancel, is_incoming_cancel_requested, push_incoming_event,
    request_incoming_cancel, wait_incoming_decision, IncomingFileMeta, IncomingTransferDecision,
    IncomingTransferEvent,
};
use base64::Engine as _;
use bytes::Bytes;
use futures_util::TryStreamExt;
use http_body_util::{combinators::BoxBody, BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use localsend::http::dto::{ErrorResponse, NonceRequest, NonceResponse, PrepareUploadResponseDto};
use localsend::http::state::ClientInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::runtime::Handle;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{timeout, Duration};
use tokio_rustls::TlsAcceptor;
use tokio_util::io::ReaderStream;

type BoxError = Box<dyn std::error::Error + Send + Sync>;
type ResponseBody = BoxBody<Bytes, BoxError>;

const MAX_CAPTURED_TEXT_BYTES: u64 = 256 * 1024;

#[derive(Clone)]
enum TlsMode {
    Disabled,
    Required(TlsAcceptor),
    Opportunistic(TlsAcceptor),
}

#[derive(Clone)]
struct ServerState {
    info: Arc<Mutex<ClientInfo>>,
    sessions: Arc<Mutex<HashMap<String, IncomingSession>>>,
    pin_config: Arc<Mutex<ReceivePinConfig>>,
    pin_attempts: Arc<Mutex<HashMap<IpAddr, PinAttemptInfo>>>,
    #[allow(dead_code)]
    default_save_directory: Arc<Mutex<Option<PathBuf>>>,
}

#[derive(Clone, Debug)]
struct ReceivePinConfig {
    enabled: bool,
    pin: String,
}

#[derive(Clone, Debug)]
struct PinAttemptInfo {
    attempts: u8,
    last_failed_at: Instant,
}

#[derive(Clone, Debug)]
struct IncomingSessionFile {
    file_name: String,
    file_type: String,
    size: u64,
    token: Option<String>,
    received: bool,
}

#[derive(Clone, Debug)]
struct IncomingSession {
    status: IncomingSessionStatus,
    sender_ip: IpAddr,
    sender_alias: String,
    sender_device_model: Option<String>,
    sender_fingerprint: String,
    files: HashMap<String, IncomingSessionFile>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IncomingSessionStatus {
    Waiting,
    Sending,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WirePeerInfo {
    alias: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    device_model: Option<String>,
    #[serde(default)]
    device_type: Option<serde_json::Value>,
    #[serde(default)]
    fingerprint: Option<String>,
    #[serde(default)]
    token: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireFileDto {
    #[allow(dead_code)]
    id: String,
    file_name: String,
    size: u64,
    file_type: String,
    #[serde(default)]
    preview: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WirePrepareUploadRequest {
    info: WirePeerInfo,
    files: HashMap<String, WireFileDto>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRegisterRequest {
    alias: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    device_model: Option<String>,
    #[serde(default)]
    device_type: Option<String>,
    #[serde(default)]
    fingerprint: Option<String>,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    protocol: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompatInfoResponse {
    alias: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_type: Option<String>,
    fingerprint: String,
    download: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompatRegisterResponse {
    alias: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_type: Option<String>,
    fingerprint: String,
    token: String,
    download: bool,
    has_web_interface: bool,
}

/// Server manager owned by NearSend.
pub struct ServerManager {
    port: u16,
    stop_tx: Option<oneshot::Sender<()>>,
    pin_config: Arc<Mutex<ReceivePinConfig>>,
    default_save_directory: Arc<Mutex<Option<PathBuf>>>,
}

impl ServerManager {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            stop_tx: None,
            pin_config: Arc::new(Mutex::new(ReceivePinConfig {
                enabled: false,
                pin: "123456".to_string(),
            })),
            default_save_directory: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(
        &mut self,
        client_info: ClientInfo,
        use_https: bool,
        cert: Option<CertPair>,
        handle: &Handle,
    ) -> anyhow::Result<()> {
        if self.is_running() {
            log::warn!("Server already running on port {}", self.port);
            return Ok(());
        }

        let tls_mode = if use_https {
            let cert = cert.ok_or_else(|| anyhow::anyhow!("HTTPS requires certificate"))?;
            TlsMode::Required(build_tls_acceptor(&cert)?)
        } else if let Some(cert) = cert.as_ref() {
            // Official LocalSend desktop falls back to a single-scheme HTTP scan.
            // Accept TLS probes on the same port even in HTTP mode to keep discovery symmetric.
            TlsMode::Opportunistic(build_tls_acceptor(cert)?)
        } else {
            TlsMode::Disabled
        };

        let (stop_tx, stop_rx) = oneshot::channel();
        self.stop_tx = Some(stop_tx);

        let port = self.port;
        let pin_config = self.pin_config.clone();
        let default_save_directory = self.default_save_directory.clone();
        handle.spawn(async move {
            if let Err(e) = start_with_port(
                port,
                client_info,
                tls_mode,
                pin_config,
                default_save_directory,
                stop_rx,
            )
            .await
            {
                log::error!("Server error: {}", e);
            }
        });

        log::info!(
            "Starting NearSend server on port {} (https={})",
            self.port,
            use_https
        );
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
            log::info!("Stopping NearSend server");
        }
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    pub fn set_receive_pin_config(&mut self, enabled: bool, pin: String, handle: &Handle) {
        let config = self.pin_config.clone();
        handle.spawn(async move {
            let mut cfg = config.lock().await;
            cfg.enabled = enabled;
            cfg.pin = pin;
        });
    }

    pub fn set_default_save_directory(&mut self, dir: Option<PathBuf>, handle: &Handle) {
        let config = self.default_save_directory.clone();
        handle.spawn(async move {
            let mut current = config.lock().await;
            *current = dir;
        });
    }

    pub fn is_running(&self) -> bool {
        self.stop_tx.is_some()
    }
}

fn build_tls_acceptor(cert: &CertPair) -> anyhow::Result<TlsAcceptor> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};

    let _ = rustls::crypto::ring::default_provider().install_default();
    let cert_chain = vec![CertificateDer::from_pem_slice(cert.cert_pem.as_bytes())?];
    let private_key = PrivateKeyDer::from_pem_slice(cert.private_key_pem.as_bytes())?;
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}

async fn start_with_port(
    port: u16,
    info: ClientInfo,
    tls_mode: TlsMode,
    pin_config: Arc<Mutex<ReceivePinConfig>>,
    default_save_directory: Arc<Mutex<Option<PathBuf>>>,
    mut stop_rx: oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    let listener_v4 = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await?;
    let listener_v6 = match tokio::net::TcpListener::bind((Ipv6Addr::UNSPECIFIED, port)).await {
        Ok(listener) => Some(listener),
        Err(err) => {
            log::warn!("ipv6 listener disabled on port {}: {}", port, err);
            None
        }
    };

    let state = ServerState {
        info: Arc::new(Mutex::new(info)),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        pin_config,
        pin_attempts: Arc::new(Mutex::new(HashMap::new())),
        default_save_directory,
    };

    loop {
        tokio::select! {
            _ = &mut stop_rx => {
                break;
            }
            accepted = listener_v4.accept() => {
                let (stream, remote_addr) = accepted?;
                let state = state.clone();
                let tls_mode = tls_mode.clone();
                tokio::spawn(async move {
                    if let Err(err) =
                        serve_tcp_connection(stream, remote_addr.ip(), state, tls_mode).await
                    {
                        log::warn!("serve connection failed: {}", err);
                    }
                });
            }
            accepted = async {
                match &listener_v6 {
                    Some(listener) => listener.accept().await,
                    None => std::future::pending::<std::io::Result<(tokio::net::TcpStream, std::net::SocketAddr)>>().await,
                }
            } => {
                let (stream, remote_addr) = accepted?;
                let state = state.clone();
                let tls_mode = tls_mode.clone();
                tokio::spawn(async move {
                    if let Err(err) =
                        serve_tcp_connection(stream, remote_addr.ip(), state, tls_mode).await
                    {
                        log::warn!("serve connection failed: {}", err);
                    }
                });
            }
        }
    }

    Ok(())
}

async fn serve_tcp_connection(
    stream: tokio::net::TcpStream,
    remote_ip: IpAddr,
    state: ServerState,
    tls_mode: TlsMode,
) -> anyhow::Result<()> {
    let builder = Builder::new(TokioExecutor::new());
    match tls_mode {
        TlsMode::Disabled => {
            let svc = service_fn(move |req| handle_request(req, state.clone(), remote_ip));
            builder
                .serve_connection(TokioIo::new(stream), svc)
                .await
                .map_err(|err| anyhow::anyhow!("serve plain connection failed: {}", err))?;
        }
        TlsMode::Required(acceptor) => {
            let tls_stream = acceptor
                .accept(stream)
                .await
                .map_err(|err| anyhow::anyhow!("tls handshake failed: {}", err))?;
            let svc = service_fn(move |req| handle_request(req, state.clone(), remote_ip));
            builder
                .serve_connection(TokioIo::new(tls_stream), svc)
                .await
                .map_err(|err| anyhow::anyhow!("serve tls connection failed: {}", err))?;
        }
        TlsMode::Opportunistic(acceptor) => {
            if looks_like_tls_client_hello(&stream).await? {
                let tls_stream = acceptor
                    .accept(stream)
                    .await
                    .map_err(|err| anyhow::anyhow!("tls handshake failed: {}", err))?;
                let svc = service_fn(move |req| handle_request(req, state.clone(), remote_ip));
                builder
                    .serve_connection(TokioIo::new(tls_stream), svc)
                    .await
                    .map_err(|err| anyhow::anyhow!("serve tls connection failed: {}", err))?;
            } else {
                let svc = service_fn(move |req| handle_request(req, state.clone(), remote_ip));
                builder
                    .serve_connection(TokioIo::new(stream), svc)
                    .await
                    .map_err(|err| anyhow::anyhow!("serve plain connection failed: {}", err))?;
            }
        }
    }
    Ok(())
}

async fn looks_like_tls_client_hello(stream: &tokio::net::TcpStream) -> anyhow::Result<bool> {
    let mut buf = [0u8; 3];
    let read = stream.peek(&mut buf).await?;
    if read < 3 {
        return Ok(false);
    }

    Ok(buf[0] == 0x16 && buf[1] == 0x03)
}

async fn handle_request(
    req: Request<Incoming>,
    state: ServerState,
    remote_ip: IpAddr,
) -> Result<Response<ResponseBody>, Infallible> {
    let response = match handle_request_inner(req, state, remote_ip).await {
        Ok(resp) => resp,
        Err((status, message)) => json_response(status, &ErrorResponse { message }),
    };
    Ok(response)
}

async fn handle_request_inner(
    req: Request<Incoming>,
    state: ServerState,
    remote_ip: IpAddr,
) -> Result<Response<ResponseBody>, (StatusCode, String)> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    match (method, path.as_str()) {
        (Method::GET, "/api/localsend/v1/info")
        | (Method::GET, "/api/localsend/v2/info")
        | (Method::GET, "/api/localsend/v3/info") => handle_info(req, state, remote_ip).await,
        (Method::POST, "/api/localsend/v1/register")
        | (Method::POST, "/api/localsend/v2/register")
        | (Method::POST, "/api/localsend/v3/register") => {
            handle_register(req, state, remote_ip).await
        }
        (Method::POST, "/api/localsend/v3/nonce") => handle_nonce(req).await,
        (Method::POST, "/api/localsend/v1/send-request")
        | (Method::POST, "/api/localsend/v1/prepare-upload") => {
            handle_prepare_upload(req, state, remote_ip, false).await
        }
        (Method::POST, "/api/localsend/v2/prepare-upload")
        | (Method::POST, "/api/localsend/v2/send-request") => {
            handle_prepare_upload(req, state, remote_ip, true).await
        }
        (Method::POST, "/api/localsend/v1/send") | (Method::POST, "/api/localsend/v1/upload") => {
            handle_upload(req, state, remote_ip, false).await
        }
        (Method::POST, "/api/localsend/v2/upload") | (Method::POST, "/api/localsend/v2/send") => {
            handle_upload(req, state, remote_ip, true).await
        }
        (Method::POST, "/api/localsend/v1/cancel") => handle_cancel(req, state, false).await,
        (Method::POST, "/api/localsend/v2/cancel") => handle_cancel(req, state, true).await,
        (Method::POST, "/api/localsend/v1/show") | (Method::POST, "/api/localsend/v2/show") => {
            Ok(json_response(StatusCode::OK, &serde_json::json!({})))
        }
        (Method::GET, p) if p.starts_with("/share/") => {
            handle_share_link(req, state, remote_ip).await
        }
        _ => Ok(json_response(
            StatusCode::NOT_FOUND,
            &ErrorResponse {
                message: "Not Found".to_string(),
            },
        )),
    }
}

async fn handle_info(
    req: Request<Incoming>,
    state: ServerState,
    remote_ip: IpAddr,
) -> Result<Response<ResponseBody>, (StatusCode, String)> {
    let info = state.info.lock().await.clone();
    let own_fingerprint = info.token.clone();
    let query = parse_query(req.uri().query().unwrap_or_default());
    let sender_fingerprint = query
        .get("fingerprint")
        .cloned()
        .or_else(|| query.get("token").cloned())
        .unwrap_or_default();

    if !sender_fingerprint.is_empty() && sender_fingerprint == own_fingerprint {
        return Ok(json_response(
            StatusCode::PRECONDITION_FAILED,
            &ErrorResponse {
                message: "Self-discovered".to_string(),
            },
        ));
    }

    if !sender_fingerprint.is_empty()
        && !crate::core::discovery::has_passive_device_token(&sender_fingerprint)
    {
        let remote_ip_text = remote_ip.to_string();
        let own_fingerprint = own_fingerprint.clone();
        let probe_port = query_first(&query, &["port"])
            .and_then(|v| v.parse::<u16>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(53317);
        let prefer_https = query_first(&query, &["protocol", "scheme"])
            .map(|v| v.eq_ignore_ascii_case("https"))
            .unwrap_or(true);
        tokio::spawn(async move {
            match crate::core::discovery::probe_device(
                &remote_ip_text,
                probe_port,
                prefer_https,
                Some(own_fingerprint),
            )
            .await
            {
                Some(device) => {
                    log::info!(
                        "reverse probe discovered peer alias={} ip={} port={} https={}",
                        device.info.alias,
                        device.ip,
                        device.port,
                        device.https
                    );
                    crate::core::discovery::register_passive_device(device);
                }
                None => {
                    log::debug!(
                        "reverse probe did not resolve peer details for {}:{}",
                        remote_ip_text,
                        probe_port
                    );
                }
            }
        });
    }

    Ok(json_response(
        StatusCode::OK,
        &CompatInfoResponse {
            alias: info.alias,
            version: info.version,
            device_model: info.device_model,
            device_type: map_device_type_to_wire(info.device_type.as_ref()),
            fingerprint: own_fingerprint,
            download: false,
        },
    ))
}

async fn handle_register(
    req: Request<Incoming>,
    state: ServerState,
    remote_ip: IpAddr,
) -> Result<Response<ResponseBody>, (StatusCode, String)> {
    match parse_json_body::<WireRegisterRequest>(req.into_body()).await {
        Ok(peer) => {
            let token = peer.fingerprint.or(peer.token).unwrap_or_default();
            let https = peer
                .protocol
                .as_deref()
                .map(|p| p.eq_ignore_ascii_case("https"))
                .unwrap_or(false);
            let port = peer.port.unwrap_or(53317);
            log::info!(
                "register discovered peer alias={} ip={} port={} https={} token_empty={}",
                peer.alias,
                remote_ip,
                port,
                https,
                token.is_empty()
            );
            crate::core::discovery::register_passive_device(
                crate::core::discovery::DiscoveredDevice {
                    info: ClientInfo {
                        alias: peer.alias,
                        version: peer.version.unwrap_or_else(|| "2.1".to_string()),
                        device_model: peer.device_model,
                        device_type: map_wire_device_type(peer.device_type.as_deref()),
                        token,
                    },
                    ip: remote_ip.to_string(),
                    port,
                    https,
                },
            );
        }
        Err((_, err)) => {
            log::debug!("register parse failed from {}: {}", remote_ip, err);
        }
    }

    let info = state.info.lock().await.clone();
    Ok(json_response(
        StatusCode::OK,
        &CompatRegisterResponse {
            alias: info.alias,
            version: info.version,
            device_model: info.device_model,
            device_type: map_device_type_to_wire(info.device_type.as_ref()),
            fingerprint: info.token.clone(),
            token: info.token,
            download: false,
            has_web_interface: false,
        },
    ))
}

async fn handle_nonce(
    req: Request<Incoming>,
) -> Result<Response<ResponseBody>, (StatusCode, String)> {
    let _payload: NonceRequest = parse_json_body(req.into_body()).await?;
    let nonce = base64::engine::general_purpose::STANDARD.encode(uuid::Uuid::new_v4().as_bytes());
    Ok(json_response(StatusCode::OK, &NonceResponse { nonce }))
}

async fn handle_prepare_upload(
    req: Request<Incoming>,
    state: ServerState,
    remote_ip: IpAddr,
    v2: bool,
) -> Result<Response<ResponseBody>, (StatusCode, String)> {
    const DECISION_WAIT_TIMEOUT_SECS: u64 = 300;
    let query = parse_query(req.uri().query().unwrap_or_default());

    // Align with LocalSend: block incoming requests when already in a session.
    if !state.sessions.lock().await.is_empty() {
        return Err((
            StatusCode::CONFLICT,
            "Blocked by another session".to_string(),
        ));
    }
    check_pin(&state, remote_ip, query.get("pin").map(|v| v.as_str())).await?;

    let payload: WirePrepareUploadRequest = parse_json_body(req.into_body()).await?;
    if payload.files.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Request must contain at least one file".to_string(),
        ));
    }

    // Register sender as a discovered device as soon as transfer request arrives.
    // This must happen regardless of whether user accepts/rejects/cancels the transfer.
    {
        let token = payload
            .info
            .fingerprint
            .clone()
            .or(payload.info.token.clone())
            .unwrap_or_default();
        let peer = crate::core::discovery::DiscoveredDevice {
            info: ClientInfo {
                alias: payload.info.alias.clone(),
                version: payload
                    .info
                    .version
                    .clone()
                    .unwrap_or_else(|| "1.0".to_string()),
                device_model: payload.info.device_model.clone(),
                device_type: map_wire_device_type(
                    payload.info.device_type.as_ref().and_then(|v| v.as_str()),
                ),
                token,
            },
            ip: remote_ip.to_string(),
            // prepare-upload payload does not include sender port; use LocalSend default.
            port: 53317,
            https: false,
        };
        log::info!(
            "prepare-upload discovered peer alias={} ip={} token_empty={}",
            peer.info.alias,
            peer.ip,
            peer.info.token.is_empty()
        );
        crate::core::discovery::register_passive_device(peer);
    }

    let sender_fingerprint = payload
        .info
        .fingerprint
        .clone()
        .or(payload.info.token.clone())
        .unwrap_or_default();
    let session_id = uuid::Uuid::new_v4().to_string();
    let sender_alias = payload.info.alias.clone();
    let sender_device_model = payload.info.device_model.clone();

    let mut metas = Vec::new();
    for (file_id, f) in &payload.files {
        metas.push(IncomingFileMeta {
            file_id: file_id.clone(),
            file_name: f.file_name.clone(),
            file_type: normalize_file_type(&f.file_type),
            size: f.size,
            preview: f.preview.clone(),
        });
    }
    push_incoming_event(IncomingTransferEvent::Prepared {
        session_id: session_id.clone(),
        sender_alias: sender_alias.clone(),
        sender_device_model: sender_device_model.clone(),
        sender_fingerprint: sender_fingerprint.clone(),
        files: metas,
    });

    let is_single_text_message = payload.files.len() == 1
        && payload
            .files
            .values()
            .next()
            .map(|f| is_text_type(&f.file_type) && f.preview.is_some())
            .unwrap_or(false);
    if is_single_text_message {
        if let Some((file_id, f)) = payload.files.iter().next() {
            push_incoming_event(IncomingTransferEvent::FileReceived {
                session_id: session_id.clone(),
                file_id: file_id.clone(),
                saved_path: None,
                saved_uri: None,
                text_content: f.preview.clone(),
            });
        }
        push_incoming_event(IncomingTransferEvent::Completed { session_id });
        return Ok(no_content());
    }

    let mut accepted_files = HashMap::new();
    let mut session_files = HashMap::new();
    for (file_id, f) in payload.files {
        session_files.insert(
            file_id,
            IncomingSessionFile {
                file_name: f.file_name,
                file_type: normalize_file_type(&f.file_type),
                size: f.size,
                token: None,
                received: false,
            },
        );
    }

    state.sessions.lock().await.insert(
        session_id.clone(),
        IncomingSession {
            status: IncomingSessionStatus::Waiting,
            sender_ip: remote_ip,
            sender_alias,
            sender_device_model,
            sender_fingerprint,
            files: session_files,
        },
    );

    let decision = match timeout(
        Duration::from_secs(DECISION_WAIT_TIMEOUT_SECS),
        wait_incoming_decision(&session_id),
    )
    .await
    {
        Ok(decision) => decision,
        Err(_) => IncomingTransferDecision::Decline,
    };

    match decision {
        IncomingTransferDecision::Decline => {
            state.sessions.lock().await.remove(&session_id);
            clear_incoming_cancel(&session_id);
            push_incoming_event(IncomingTransferEvent::Cancelled {
                session_id: session_id.clone(),
                reason: Some("declined by recipient".to_string()),
            });
            return Err((
                StatusCode::FORBIDDEN,
                "File request declined by recipient".to_string(),
            ));
        }
        IncomingTransferDecision::AcceptAll => {
            let mut sessions = state.sessions.lock().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.status = IncomingSessionStatus::Sending;
                for (file_id, file) in session.files.iter_mut() {
                    let token = uuid::Uuid::new_v4().to_string();
                    accepted_files.insert(file_id.clone(), token.clone());
                    file.token = Some(token);
                }
            } else {
                return Err((StatusCode::CONFLICT, "No session".to_string()));
            }
        }
        IncomingTransferDecision::AcceptSelected(selected_ids) => {
            let selected_set: std::collections::HashSet<String> =
                selected_ids.into_iter().collect();
            if selected_set.is_empty() {
                state.sessions.lock().await.remove(&session_id);
                clear_incoming_cancel(&session_id);
                push_incoming_event(IncomingTransferEvent::Cancelled {
                    session_id: session_id.clone(),
                    reason: Some("accepted with empty selection".to_string()),
                });
                return Ok(no_content());
            }
            let mut sessions = state.sessions.lock().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.status = IncomingSessionStatus::Sending;
                for (file_id, file) in session.files.iter_mut() {
                    if selected_set.contains(file_id) {
                        let token = uuid::Uuid::new_v4().to_string();
                        accepted_files.insert(file_id.clone(), token.clone());
                        file.token = Some(token);
                    }
                }
                session
                    .files
                    .retain(|file_id, _| selected_set.contains(file_id));
            } else {
                return Err((StatusCode::CONFLICT, "No session".to_string()));
            }
        }
    }

    if v2 {
        Ok(json_response(
            StatusCode::OK,
            &PrepareUploadResponseDto {
                session_id,
                files: accepted_files,
            },
        ))
    } else {
        Ok(json_response(StatusCode::OK, &accepted_files))
    }
}

async fn handle_upload(
    req: Request<Incoming>,
    state: ServerState,
    remote_ip: IpAddr,
    v2: bool,
) -> Result<Response<ResponseBody>, (StatusCode, String)> {
    let query = parse_query(req.uri().query().unwrap_or_default());
    let file_id = query_first(&query, &["fileId", "fileID", "file_id", "id"])
        .cloned()
        .ok_or((StatusCode::BAD_REQUEST, "missing fileId".to_string()))?;
    let token = query_first(&query, &["token", "fileToken"])
        .cloned()
        .ok_or((StatusCode::BAD_REQUEST, "missing token".to_string()))?;
    let session_id_query = query_first(&query, &["sessionId", "session_id", "sid"]).cloned();
    if v2 && session_id_query.is_none() {
        return Err((StatusCode::BAD_REQUEST, "missing sessionId".to_string()));
    }

    let content_length = req
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    let (session_id, file_name_for_save, file_type, expected_size) = {
        let mut sessions = state.sessions.lock().await;
        let sid = if let Some(sid) = session_id_query.clone() {
            sid
        } else {
            sessions
                .keys()
                .next()
                .cloned()
                .ok_or((StatusCode::CONFLICT, "No session".to_string()))?
        };
        let session = sessions
            .get_mut(&sid)
            .ok_or((StatusCode::FORBIDDEN, "Invalid session id".to_string()))?;
        if session.sender_ip != remote_ip {
            return Err((StatusCode::FORBIDDEN, "Invalid IP address".to_string()));
        }
        if session.status != IncomingSessionStatus::Sending {
            return Err((
                StatusCode::CONFLICT,
                "Recipient is in wrong state".to_string(),
            ));
        }
        let file = session
            .files
            .get_mut(&file_id)
            .ok_or((StatusCode::FORBIDDEN, "Invalid token".to_string()))?;
        if file.token.as_deref() != Some(token.as_str()) {
            return Err((StatusCode::FORBIDDEN, "Invalid token".to_string()));
        }
        (
            sid.clone(),
            file.file_name.clone(),
            file.file_type.clone(),
            content_length.unwrap_or(file.size),
        )
    };

    if is_incoming_cancel_requested(&session_id) {
        state.sessions.lock().await.remove(&session_id);
        push_incoming_event(IncomingTransferEvent::Cancelled {
            session_id: session_id.clone(),
            reason: Some("cancelled by recipient".to_string()),
        });
        return Err((StatusCode::CONFLICT, "Transfer cancelled".to_string()));
    }

    let (saved_location, mut output_file) =
        match crate::platform::save_file::create_incoming_file(&session_id, &file_name_for_save)
            .await
        {
            Ok(target) => target,
            Err(err) => {
                let msg = format!("save incoming file failed: {}", err);
                cancel_incoming_session(&state, &session_id, msg.clone()).await;
                return Err((StatusCode::INTERNAL_SERVER_ERROR, msg));
            }
        };

    let mut body = req.into_body();
    let mut captured_text =
        (is_text_type(&file_type) && expected_size <= MAX_CAPTURED_TEXT_BYTES).then(Vec::new);
    let started_at = Instant::now();
    let mut last_emit_at = Instant::now();
    let mut bytes_received = 0_u64;

    push_incoming_event(IncomingTransferEvent::FileProgress {
        session_id: session_id.clone(),
        file_id: file_id.clone(),
        bytes_received,
        total_bytes: expected_size,
        speed_bytes_per_sec: 0,
    });

    while let Some(frame) = match body.frame().await {
        _ if is_incoming_cancel_requested(&session_id) => {
            let _ = tokio::fs::remove_file(&saved_location.native_path).await;
            state.sessions.lock().await.remove(&session_id);
            push_incoming_event(IncomingTransferEvent::Cancelled {
                session_id: session_id.clone(),
                reason: Some("cancelled by recipient".to_string()),
            });
            return Err((StatusCode::CONFLICT, "Transfer cancelled".to_string()));
        }
        Some(Ok(frame)) => Some(frame),
        Some(Err(err)) => {
            let msg = format!("read body failed: {}", err);
            let _ = tokio::fs::remove_file(&saved_location.native_path).await;
            cancel_incoming_session(&state, &session_id, msg.clone()).await;
            return Err((StatusCode::BAD_REQUEST, msg));
        }
        None => None,
    } {
        let Ok(chunk) = frame.into_data() else {
            continue;
        };
        bytes_received = bytes_received.saturating_add(chunk.len() as u64);

        if let Some(buffer) = captured_text.as_mut() {
            let remaining = MAX_CAPTURED_TEXT_BYTES.saturating_sub(buffer.len() as u64) as usize;
            if remaining > 0 {
                let take = remaining.min(chunk.len());
                buffer.extend_from_slice(&chunk[..take]);
            }
        }

        if let Err(err) = output_file.write_all(&chunk).await {
            let msg = format!("save incoming file failed: {}", err);
            let _ = tokio::fs::remove_file(&saved_location.native_path).await;
            cancel_incoming_session(&state, &session_id, msg.clone()).await;
            return Err((StatusCode::INTERNAL_SERVER_ERROR, msg));
        }

        if is_incoming_cancel_requested(&session_id) {
            let _ = tokio::fs::remove_file(&saved_location.native_path).await;
            state.sessions.lock().await.remove(&session_id);
            push_incoming_event(IncomingTransferEvent::Cancelled {
                session_id: session_id.clone(),
                reason: Some("cancelled by recipient".to_string()),
            });
            return Err((StatusCode::CONFLICT, "Transfer cancelled".to_string()));
        }

        if last_emit_at.elapsed() >= StdDuration::from_millis(100)
            || (expected_size > 0 && bytes_received >= expected_size)
        {
            push_incoming_event(IncomingTransferEvent::FileProgress {
                session_id: session_id.clone(),
                file_id: file_id.clone(),
                bytes_received,
                total_bytes: expected_size,
                speed_bytes_per_sec: average_speed(bytes_received, started_at),
            });
            last_emit_at = Instant::now();
        }
    }

    push_incoming_event(IncomingTransferEvent::FileProgress {
        session_id: session_id.clone(),
        file_id: file_id.clone(),
        bytes_received,
        total_bytes: expected_size,
        speed_bytes_per_sec: average_speed(bytes_received, started_at),
    });

    if let Err(err) = output_file.flush().await {
        let msg = format!("save incoming file failed: {}", err);
        let _ = tokio::fs::remove_file(&saved_location.native_path).await;
        cancel_incoming_session(&state, &session_id, msg.clone()).await;
        return Err((StatusCode::INTERNAL_SERVER_ERROR, msg));
    }

    let text_content = captured_text.and_then(|bytes| {
        if bytes.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&bytes).into_owned())
        }
    });

    let (saved_path, saved_uri, completed_now) = {
        let mut sessions = state.sessions.lock().await;
        let session = sessions
            .get_mut(&session_id)
            .ok_or((StatusCode::FORBIDDEN, "Invalid session id".to_string()))?;
        let file = session
            .files
            .get_mut(&file_id)
            .ok_or((StatusCode::FORBIDDEN, "Invalid token".to_string()))?;
        file.received = true;
        let completed_now = session.files.values().all(|f| f.received);
        if completed_now {
            session.status = IncomingSessionStatus::Completed;
        }
        Ok((
            Some(saved_location.native_path.display().to_string()),
            saved_location.original_uri.clone(),
            completed_now,
        ))
    }?;

    push_incoming_event(IncomingTransferEvent::FileReceived {
        session_id: session_id.clone(),
        file_id,
        saved_path,
        saved_uri,
        text_content,
    });
    if completed_now {
        push_incoming_event(IncomingTransferEvent::Completed {
            session_id: session_id.clone(),
        });
        state.sessions.lock().await.remove(&session_id);
        clear_incoming_cancel(&session_id);
    }

    Ok(json_response(StatusCode::OK, &serde_json::json!({})))
}

fn average_speed(bytes_received: u64, started_at: Instant) -> u64 {
    let elapsed = started_at.elapsed().as_secs_f64();
    if elapsed > 0.0 {
        (bytes_received as f64 / elapsed) as u64
    } else {
        0
    }
}

async fn handle_cancel(
    req: Request<Incoming>,
    state: ServerState,
    v2: bool,
) -> Result<Response<ResponseBody>, (StatusCode, String)> {
    let query = parse_query(req.uri().query().unwrap_or_default());
    let session_id_query = query_first(&query, &["sessionId", "session_id", "sid"]).cloned();
    let session_id = if let Some(session_id) = session_id_query {
        session_id
    } else {
        let sessions = state.sessions.lock().await;
        if !v2 {
            sessions.keys().next().cloned().unwrap_or_default()
        } else if sessions.len() == 1 {
            let Some((id, session)) = sessions.iter().next() else {
                return Err((StatusCode::BAD_REQUEST, "missing sessionId".to_string()));
            };
            // Align with LocalSend: in waiting stage v2 cancel may omit sessionId.
            let waiting = session.files.values().all(|f| f.token.is_none());
            if waiting {
                id.clone()
            } else {
                return Err((StatusCode::BAD_REQUEST, "missing sessionId".to_string()));
            }
        } else {
            return Err((StatusCode::BAD_REQUEST, "missing sessionId".to_string()));
        }
    };

    if !session_id.is_empty() {
        if let Some(mut session) = state.sessions.lock().await.remove(&session_id) {
            session.status = IncomingSessionStatus::Cancelled;
            log::info!(
                "cancelled session {} from {} ({:?}), files={}",
                session_id,
                session.sender_alias,
                session.sender_device_model,
                session.files.len()
            );
            push_incoming_event(IncomingTransferEvent::Cancelled {
                session_id: session_id.clone(),
                reason: Some(format!("cancelled by {}", session.sender_fingerprint)),
            });
            request_incoming_cancel(session_id);
        }
    }
    Ok(json_response(StatusCode::OK, &serde_json::json!({})))
}

async fn handle_share_link(
    req: Request<Incoming>,
    state: ServerState,
    remote_ip: IpAddr,
) -> Result<Response<ResponseBody>, (StatusCode, String)> {
    let path = req.uri().path().trim_matches('/');
    let query = parse_query(req.uri().query().unwrap_or_default());
    check_pin(&state, remote_ip, query.get("pin").map(|v| v.as_str())).await?;
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() < 2 || segments.first() != Some(&"share") {
        return Err((StatusCode::NOT_FOUND, "Not Found".to_string()));
    }
    let share_id = segments[1];
    let Some(record) = crate::core::share_links::get_share(share_id) else {
        return Err((StatusCode::NOT_FOUND, "Share link not found".to_string()));
    };

    if segments.len() == 2 {
        let mut html = String::new();
        html.push_str("<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
        html.push_str("<title>NearSend Share</title><style>body{font-family:-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,sans-serif;padding:20px;max-width:680px;margin:auto;}h1{font-size:20px;}li{margin:10px 0;}a{color:#0b65d8;text-decoration:none;}a:hover{text-decoration:underline;}small{color:#777;}</style></head><body>");
        html.push_str("<h1>NearSend Shared Files</h1><ul>");
        for (idx, entry) in record.entries.iter().enumerate() {
            let name = match entry {
                crate::core::share_links::SharedEntry::File { name, .. } => name,
                crate::core::share_links::SharedEntry::Text { name, .. } => name,
            };
            let safe_name = html_escape(name);
            html.push_str(&format!(
                "<li><a href=\"/share/{}/download/{}\">{}</a></li>",
                record.id, idx, safe_name
            ));
        }
        html.push_str("</ul><small>Powered by NearSend</small></body></html>");
        return Ok(html_response(StatusCode::OK, html));
    }

    if segments.len() == 4 && segments[2] == "download" {
        let Ok(index) = segments[3].parse::<usize>() else {
            return Err((StatusCode::BAD_REQUEST, "Invalid file index".to_string()));
        };
        let Some(entry) = record.entries.get(index) else {
            return Err((StatusCode::NOT_FOUND, "File not found".to_string()));
        };
        match entry {
            crate::core::share_links::SharedEntry::Text { name, content } => {
                return Ok(binary_response(
                    StatusCode::OK,
                    "text/plain; charset=utf-8",
                    name,
                    Bytes::from(content.clone()),
                ));
            }
            crate::core::share_links::SharedEntry::File {
                name,
                path,
                file_type,
            } => {
                let file = tokio::fs::File::open(path)
                    .await
                    .map_err(|_| (StatusCode::NOT_FOUND, "File no longer exists".to_string()))?;
                let content_length = file.metadata().await.ok().map(|meta| meta.len());
                return Ok(stream_file_response(
                    StatusCode::OK,
                    file_type,
                    name,
                    file,
                    content_length,
                ));
            }
        }
    }

    Err((StatusCode::NOT_FOUND, "Not Found".to_string()))
}

async fn parse_json_body<T: serde::de::DeserializeOwned>(
    body: Incoming,
) -> Result<T, (StatusCode, String)> {
    let bytes = body
        .collect()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("read body failed: {}", e)))?
        .to_bytes();
    serde_json::from_slice(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid json: {}", e)))
}

async fn check_pin(
    state: &ServerState,
    remote_ip: IpAddr,
    request_pin: Option<&str>,
) -> Result<(), (StatusCode, String)> {
    const PIN_MAX_ATTEMPTS: u8 = 3;
    const PIN_ATTEMPT_WINDOW_SECS: u64 = 300;

    let (enabled, configured_pin) = {
        let cfg = state.pin_config.lock().await;
        (cfg.enabled, cfg.pin.clone())
    };
    if !enabled || configured_pin.trim().is_empty() {
        return Ok(());
    }

    let now = Instant::now();
    let window = StdDuration::from_secs(PIN_ATTEMPT_WINDOW_SECS);
    let mut attempts_guard = state.pin_attempts.lock().await;
    attempts_guard.retain(|_, entry| now.duration_since(entry.last_failed_at) <= window);

    let attempts = attempts_guard
        .get(&remote_ip)
        .map(|entry| entry.attempts)
        .unwrap_or(0);
    if attempts >= PIN_MAX_ATTEMPTS {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many attempts.".to_string(),
        ));
    }

    if request_pin != Some(configured_pin.as_str()) {
        if request_pin.map(|v| !v.is_empty()).unwrap_or(false) {
            let entry = attempts_guard.entry(remote_ip).or_insert(PinAttemptInfo {
                attempts: 0,
                last_failed_at: now,
            });
            entry.attempts = entry.attempts.saturating_add(1);
            entry.last_failed_at = now;
            if entry.attempts >= PIN_MAX_ATTEMPTS {
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    "Too many attempts.".to_string(),
                ));
            }
        }
        return Err((StatusCode::UNAUTHORIZED, "Invalid pin.".to_string()));
    }

    attempts_guard.remove(&remote_ip);
    Ok(())
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or_default();
        let v = it.next().unwrap_or_default();
        map.insert(k.to_string(), v.to_string());
    }
    map
}

fn query_first<'a>(query: &'a HashMap<String, String>, keys: &[&str]) -> Option<&'a String> {
    for key in keys {
        if let Some(value) = query.get(*key) {
            return Some(value);
        }
    }
    None
}

fn normalize_file_type(ft: &str) -> String {
    if ft.contains('/') {
        ft.to_string()
    } else {
        match ft.to_ascii_lowercase().as_str() {
            "text" => "text/plain".to_string(),
            "image" => "image/*".to_string(),
            "video" => "video/*".to_string(),
            "pdf" => "application/pdf".to_string(),
            "apk" => "application/vnd.android.package-archive".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }
}

fn is_text_type(ft: &str) -> bool {
    let t = ft.to_ascii_lowercase();
    t.starts_with("text/") || t == "text"
}

fn map_device_type_to_wire(
    device_type: Option<&localsend::model::discovery::DeviceType>,
) -> Option<String> {
    match device_type {
        Some(localsend::model::discovery::DeviceType::Mobile) => Some("mobile".to_string()),
        Some(localsend::model::discovery::DeviceType::Desktop) => Some("desktop".to_string()),
        Some(localsend::model::discovery::DeviceType::Web) => Some("web".to_string()),
        Some(localsend::model::discovery::DeviceType::Headless) => Some("headless".to_string()),
        Some(localsend::model::discovery::DeviceType::Server) => Some("server".to_string()),
        None => None,
    }
}

fn map_wire_device_type(value: Option<&str>) -> Option<localsend::model::discovery::DeviceType> {
    match value {
        Some("mobile") | Some("MOBILE") => Some(localsend::model::discovery::DeviceType::Mobile),
        Some("desktop") | Some("DESKTOP") => Some(localsend::model::discovery::DeviceType::Desktop),
        Some("web") | Some("WEB") => Some(localsend::model::discovery::DeviceType::Web),
        Some("headless") | Some("HEADLESS") => {
            Some(localsend::model::discovery::DeviceType::Headless)
        }
        Some("server") | Some("SERVER") => Some(localsend::model::discovery::DeviceType::Server),
        _ => None,
    }
}

fn json_response<T: serde::Serialize>(status: StatusCode, body: &T) -> Response<ResponseBody> {
    let mut response = Response::new(full_body(Bytes::new()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        hyper::http::HeaderValue::from_static("application/json"),
    );
    let payload = serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec());
    *response.body_mut() = full_body(Bytes::from(payload));
    response
}

fn html_response(status: StatusCode, html: String) -> Response<ResponseBody> {
    let mut response = Response::new(full_body(Bytes::new()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        hyper::http::HeaderValue::from_static("text/html; charset=utf-8"),
    );
    *response.body_mut() = full_body(Bytes::from(html));
    response
}

fn binary_response(
    status: StatusCode,
    content_type: &str,
    file_name: &str,
    bytes: Bytes,
) -> Response<ResponseBody> {
    let mut response = Response::new(full_body(Bytes::new()));
    *response.status_mut() = status;
    if let Ok(v) = hyper::http::HeaderValue::from_str(content_type) {
        response.headers_mut().insert(CONTENT_TYPE, v);
    } else {
        response.headers_mut().insert(
            CONTENT_TYPE,
            hyper::http::HeaderValue::from_static("application/octet-stream"),
        );
    }
    let safe = file_name.replace('"', "");
    if let Ok(v) = hyper::http::HeaderValue::from_str(&format!("attachment; filename=\"{}\"", safe))
    {
        response.headers_mut().insert(CONTENT_DISPOSITION, v);
    }
    if let Ok(v) = hyper::http::HeaderValue::from_str(&bytes.len().to_string()) {
        response.headers_mut().insert(CONTENT_LENGTH, v);
    }
    *response.body_mut() = full_body(bytes);
    response
}

fn stream_file_response(
    status: StatusCode,
    content_type: &str,
    file_name: &str,
    file: tokio::fs::File,
    content_length: Option<u64>,
) -> Response<ResponseBody> {
    let stream = ReaderStream::new(file).map_ok(Frame::data);
    let body =
        BodyExt::map_err(StreamBody::new(stream), |err| -> BoxError { Box::new(err) }).boxed();
    let mut response = Response::new(body);
    *response.status_mut() = status;
    if let Ok(v) = hyper::http::HeaderValue::from_str(content_type) {
        response.headers_mut().insert(CONTENT_TYPE, v);
    } else {
        response.headers_mut().insert(
            CONTENT_TYPE,
            hyper::http::HeaderValue::from_static("application/octet-stream"),
        );
    }
    let safe = file_name.replace('"', "");
    if let Ok(v) = hyper::http::HeaderValue::from_str(&format!("attachment; filename=\"{}\"", safe))
    {
        response.headers_mut().insert(CONTENT_DISPOSITION, v);
    }
    if let Some(len) = content_length {
        if let Ok(v) = hyper::http::HeaderValue::from_str(&len.to_string()) {
            response.headers_mut().insert(CONTENT_LENGTH, v);
        }
    }
    response
}

fn no_content() -> Response<ResponseBody> {
    let mut response = Response::new(full_body(Bytes::new()));
    *response.status_mut() = StatusCode::NO_CONTENT;
    response
}

fn full_body(bytes: Bytes) -> ResponseBody {
    Full::new(bytes).map_err(|never| match never {}).boxed()
}

async fn cancel_incoming_session(state: &ServerState, session_id: &str, reason: String) {
    log::warn!("{}", reason);
    if let Some(mut session) = state.sessions.lock().await.remove(session_id) {
        session.status = IncomingSessionStatus::Cancelled;
        push_incoming_event(IncomingTransferEvent::Cancelled {
            session_id: session_id.to_string(),
            reason: Some(reason),
        });
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
