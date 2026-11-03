use rcgen::{CertificateParams, KeyPair};
use rustls::pki_types::{pem::PemObject as _, CertificateDer, PrivateKeyDer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::{self, File},
    io::{self, BufReader},
    path::PathBuf,
};

/// The same identity is used for discovery, transfer metadata and TLS clients.
#[derive(Clone)]
pub struct CertPair {
    pub cert_pem: String,
    pub private_key_pem: String,
    pub certificate_fingerprint: String,
}

impl fmt::Debug for CertPair {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CertPair")
            .field("certificate_fingerprint", &self.certificate_fingerprint)
            .finish_non_exhaustive()
    }
}

impl CertPair {
    fn generate() -> Result<Self, IdentityError> {
        let key_pair = KeyPair::generate()?;
        let params = CertificateParams::new(vec!["localsend".to_string()])?;
        let cert = params.self_signed(&key_pair)?;
        Ok(Self {
            cert_pem: cert.pem(),
            private_key_pem: key_pair.serialize_pem(),
            certificate_fingerprint: hash_certificate_der(cert.der().as_ref()),
        })
    }

    fn from_pem(cert_pem: String, private_key_pem: String) -> Result<Self, IdentityError> {
        let cert = CertificateDer::from_pem_slice(cert_pem.as_bytes())?;
        let fingerprint = hash_certificate_der(cert.as_ref());
        let key = PrivateKeyDer::from_pem_slice(private_key_pem.as_bytes())?;
        let certified_key = rustls::sign::CertifiedKey::from_der(
            vec![cert],
            key,
            &rustls::crypto::ring::default_provider(),
        )?;
        certified_key.keys_match()?;
        Ok(Self {
            cert_pem,
            private_key_pem,
            certificate_fingerprint: fingerprint,
        })
    }

    /// LocalSend HTTPS peers identify the sender by its client certificate.
    pub fn http_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let pem = [
            self.cert_pem.as_bytes(),
            b"\n",
            self.private_key_pem.as_bytes(),
        ]
        .concat();
        let identity = reqwest::Identity::from_pem(&pem)?;
        reqwest::Client::builder()
            .use_rustls_tls()
            .danger_accept_invalid_certs(true)
            .identity(identity)
            .build()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("cannot access the saved device identity: {0}")]
    Io(#[from] io::Error),
    #[error("the saved device identity has invalid JSON")]
    Json(#[from] serde_json::Error),
    #[error("the saved device identity has invalid PEM data")]
    Pem(#[from] rustls::pki_types::pem::Error),
    #[error("the certificate and private key do not form a valid TLS identity: {0}")]
    Tls(#[from] rustls::Error),
    #[error("cannot generate a TLS identity: {0}")]
    Generate(#[from] rcgen::Error),
}

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    cert_pem: String,
    private_key_pem: String,
}

/// Persist the certificate and key together so favorites survive app restarts.
pub struct IdentityStore {
    directory: PathBuf,
}

impl IdentityStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn load_or_create(&self) -> Result<CertPair, IdentityError> {
        match self.load() {
            Err(IdentityError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                self.create()
            }
            result => result,
        }
    }

    fn path(&self) -> PathBuf {
        self.directory.join("identity.json")
    }

    fn load(&self) -> Result<CertPair, IdentityError> {
        let file = File::open(self.path())?;
        let stored: StoredIdentity = serde_json::from_reader(BufReader::new(file))?;
        CertPair::from_pem(stored.cert_pem, stored.private_key_pem)
    }

    fn create(&self) -> Result<CertPair, IdentityError> {
        fs::create_dir_all(&self.directory)?;
        let cert = CertPair::generate()?;
        // NamedTempFile creates private files on Unix. Publish the complete pair
        // atomically, without replacing an identity another window has created.
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
        serde_json::to_writer_pretty(
            temporary.as_file_mut(),
            &StoredIdentity {
                cert_pem: cert.cert_pem.clone(),
                private_key_pem: cert.private_key_pem.clone(),
            },
        )?;
        temporary.as_file().sync_all()?;
        match temporary.persist_noclobber(self.path()) {
            Ok(_) => Ok(cert),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => self.load(),
            Err(error) => Err(IdentityError::Io(error.error)),
        }
    }
}

fn hash_certificate_der(der: &[u8]) -> String {
    let digest = Sha256::digest(der);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02X}");
    }
    out
}

#[cfg(test)]
#[path = "cert/tests.rs"]
mod tests;
