use super::*;
use std::sync::{Arc, Barrier};

#[test]
fn fingerprint_uses_localsend_uppercase_sha256() {
    assert_eq!(
        hash_certificate_der(b"abc"),
        "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
    );
}

#[test]
fn restarting_reuses_the_saved_certificate_and_fingerprint() {
    let directory = tempfile::tempdir().unwrap();
    let first = IdentityStore::new(directory.path().to_path_buf())
        .load_or_create()
        .unwrap();
    let restarted = IdentityStore::new(directory.path().to_path_buf())
        .load_or_create()
        .unwrap();
    assert_eq!(first.cert_pem, restarted.cert_pem);
    assert_eq!(first.private_key_pem, restarted.private_key_pem);
    assert_eq!(
        first.certificate_fingerprint,
        restarted.certificate_fingerprint
    );
}

#[test]
fn concurrent_startup_publishes_one_complete_identity() {
    let directory = tempfile::tempdir().unwrap();
    let barrier = Arc::new(Barrier::new(4));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let directory = directory.path().to_path_buf();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                IdentityStore::new(directory)
                    .load_or_create()
                    .unwrap()
                    .certificate_fingerprint
            })
        })
        .collect();
    let fingerprints: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert!(fingerprints.iter().all(|value| value == &fingerprints[0]));
}

#[test]
fn corrupt_identity_is_reported_without_replacing_it() {
    let directory = tempfile::tempdir().unwrap();
    let store = IdentityStore::new(directory.path().to_path_buf());
    fs::write(store.path(), b"incomplete identity").unwrap();
    assert!(matches!(
        store.load_or_create(),
        Err(IdentityError::Json(_))
    ));
    assert_eq!(fs::read(store.path()).unwrap(), b"incomplete identity");
}

#[test]
fn a_mismatched_saved_private_key_is_not_silently_regenerated() {
    let directory = tempfile::tempdir().unwrap();
    let store = IdentityStore::new(directory.path().to_path_buf());
    let first = CertPair::generate().unwrap();
    let second = CertPair::generate().unwrap();
    let bytes = serde_json::to_vec(&StoredIdentity {
        cert_pem: first.cert_pem,
        private_key_pem: second.private_key_pem,
    })
    .unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert!(matches!(store.load_or_create(), Err(IdentityError::Tls(_))));
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn the_saved_private_key_is_accessible_only_to_its_owner() {
    use std::os::unix::fs::PermissionsExt as _;
    let directory = tempfile::tempdir().unwrap();
    let store = IdentityStore::new(directory.path().to_path_buf());
    store.load_or_create().unwrap();
    assert_eq!(
        fs::metadata(store.path()).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[tokio::test]
async fn https_favorite_identity_matches_metadata_even_after_restart() {
    use bytes::Bytes;
    use http_body_util::{BodyExt as _, Full};
    use hyper::{server::conn::http1, service::service_fn, Response, StatusCode};
    use hyper_util::rt::TokioIo;
    use rustls::{server::WebPkiClientVerifier, RootCertStore, ServerConfig};
    use std::{convert::Infallible, time::Duration};
    use tokio::net::TcpListener;
    use tokio_rustls::TlsAcceptor;

    let directory = tempfile::tempdir().unwrap();
    let favorite = IdentityStore::new(directory.path().to_path_buf())
        .load_or_create()
        .unwrap();
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from_pem_slice(favorite.cert_pem.as_bytes()).unwrap())
        .unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider.clone())
        .build()
        .unwrap();
    let server_identity = CertPair::generate().unwrap();
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_client_cert_verifier(verifier)
        .with_single_cert(
            vec![CertificateDer::from_pem_slice(server_identity.cert_pem.as_bytes()).unwrap()],
            PrivateKeyDer::from_pem_slice(server_identity.private_key_pem.as_bytes()).unwrap(),
        )
        .unwrap();
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let expected = favorite.certificate_fingerprint.clone();
    let server = tokio::spawn(async move {
        let acceptor = TlsAcceptor::from(Arc::new(config));
        for request in 0..3 {
            let (stream, _) = listener.accept().await.unwrap();
            let tls = match acceptor.accept(stream).await {
                Ok(tls) => tls,
                Err(_) if request == 2 => return,
                Err(error) => panic!("authenticated client handshake failed: {error}"),
            };
            assert_ne!(request, 2, "an anonymous client must not pass mutual TLS");
            let certificate = &tls.get_ref().1.peer_certificates().unwrap()[0];
            let actual_fingerprint = hash_certificate_der(certificate.as_ref());
            let expected = expected.clone();
            let service =
                service_fn(move |request: hyper::Request<hyper::body::Incoming>| {
                    let expected = expected.clone();
                    let actual_fingerprint = actual_fingerprint.clone();
                    async move {
                        assert_eq!(request.uri().path(), "/api/localsend/v2/prepare-upload");
                        let body = request.into_body().collect().await.unwrap().to_bytes();
                        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
                        // Official LocalSend matches HTTPS favorites against the
                        // authenticated certificate, not a self-reported payload ID.
                        let accepted = actual_fingerprint == expected
                            && payload["info"]["fingerprint"] == expected;
                        Ok::<_, Infallible>(
                        Response::builder()
                            .status(if accepted { StatusCode::OK } else { StatusCode::FORBIDDEN })
                            .body(Full::new(Bytes::from_static(
                                br#"{"sessionId":"test-session","files":{"file":"upload-token"}}"#,
                            )))
                            .unwrap(),
                    )
                    }
                });
            http1::Builder::new()
                .keep_alive(false)
                .serve_connection(TokioIo::new(tls), service)
                .await
                .unwrap();
        }
        panic!("the anonymous negative case was not exercised");
    });

    let url = format!("https://{address}/api/localsend/v2/prepare-upload");
    for _ in 0..2 {
        let restarted = IdentityStore::new(directory.path().to_path_buf())
            .load_or_create()
            .unwrap();
        let payload = serde_json::json!({
            "info": {"fingerprint": restarted.certificate_fingerprint},
            "files": {"file": {"fileName": "test.txt", "size": 1, "fileType": "text/plain"}}
        });
        let response = restarted
            .http_client()
            .unwrap()
            .post(&url)
            .json(&payload)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .expect("a stored identity must pass the receiver's mutual TLS");
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["files"]["file"], "upload-token");
    }
    let anonymous = reqwest::Client::builder()
        .use_rustls_tls()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    assert!(anonymous
        .post(&url)
        .json(&serde_json::json!({}))
        .send()
        .await
        .is_err());
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
}
