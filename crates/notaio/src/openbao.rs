//! The production signer (ADR-0004): signatures from a non-exportable ed25519 key in OpenBao
//! transit. notaio never holds the key: it logs in with its ServiceAccount (Kubernetes auth,
//! projected token, audience `openbao`) and asks for one signature at a time.
//!
//! The signer is chosen from the environment by [`choose_signer`]:
//!
//! | Variable | Meaning | Default |
//! | --- | --- | --- |
//! | `NOTAIO_OPENBAO_ADDR` | OpenBao's address, `https://` only | none: OpenBao signer off |
//! | `NOTAIO_OPENBAO_CA_FILE` | CA that signed OpenBao's certificate, the only root trusted | required with the address |
//! | `NOTAIO_OPENBAO_ROLE` | Kubernetes auth role | `notaio` |
//! | `NOTAIO_OPENBAO_KEY` | transit key | `notaio` |
//! | `NOTAIO_OPENBAO_AUTH_PATH` | Kubernetes auth mount | `kubernetes` |
//! | `NOTAIO_OPENBAO_TOKEN_FILE` | projected ServiceAccount token | `/var/run/secrets/openbao/token` |
//! | `NOTAIO_SIGNING_KEY_FILE`, `NOTAIO_KEY_ID` | development signer, for kind and tests only | none, `dev` |
//!
//! Setting both the OpenBao address and the development key is refused.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use bytes::Bytes;
use http::{Method, Request, StatusCode};
use http_body_util::{BodyExt as _, Full};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::CertificateDer;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::time::Instant;

pub const ENV_ADDR: &str = "NOTAIO_OPENBAO_ADDR";
pub const ENV_CA_FILE: &str = "NOTAIO_OPENBAO_CA_FILE";
pub const ENV_ROLE: &str = "NOTAIO_OPENBAO_ROLE";
pub const ENV_KEY: &str = "NOTAIO_OPENBAO_KEY";
pub const ENV_AUTH_PATH: &str = "NOTAIO_OPENBAO_AUTH_PATH";
pub const ENV_TOKEN_FILE: &str = "NOTAIO_OPENBAO_TOKEN_FILE";
pub const ENV_DEV_KEY_FILE: &str = "NOTAIO_SIGNING_KEY_FILE";
pub const ENV_DEV_KEY_ID: &str = "NOTAIO_KEY_ID";

const DEFAULT_ROLE: &str = "notaio";
const DEFAULT_KEY: &str = "notaio";
const DEFAULT_AUTH_PATH: &str = "kubernetes";
const DEFAULT_TOKEN_FILE: &str = "/var/run/secrets/openbao/token";
const DEFAULT_DEV_KEY_ID: &str = "dev";

/// Length of an ed25519 signature.
const ED25519_SIGNATURE_LEN: usize = 64;
/// Attempts per signature, for transport errors and 5xx (a sealed OpenBao answers 503).
const MAX_ATTEMPTS: u32 = 3;
/// First retry delay; doubles each time.
const FIRST_BACKOFF: Duration = Duration::from_millis(500);
/// Bound on one request, so a hung OpenBao cannot stall a reconcile.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Log in again once this share of the token's lease has passed (four fifths).
const REFRESH_NUMERATOR: u64 = 4;
const REFRESH_DENOMINATOR: u64 = 5;

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("signer configuration: {0}")]
    Config(String),
    #[error("cannot reach OpenBao: {0}")]
    Transport(String),
    #[error("OpenBao answered {status}: {body}")]
    Status { status: u16, body: String },
    #[error("OpenBao refused the token")]
    Forbidden,
    #[error("unexpected response from OpenBao: {0}")]
    Response(String),
}

impl SignError {
    /// Worth another attempt: OpenBao unreachable, sealed or overloaded, not a refusal.
    fn retryable(&self) -> bool {
        match self {
            SignError::Transport(_) => true,
            SignError::Status { status, .. } => {
                *status >= StatusCode::INTERNAL_SERVER_ERROR.as_u16()
                    || *status == StatusCode::TOO_MANY_REQUESTS.as_u16()
            }
            _ => false,
        }
    }
}

/// How to reach the transit key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitConfig {
    pub addr: String,
    pub ca_file: PathBuf,
    pub role: String,
    pub key: String,
    pub auth_path: String,
    pub token_file: PathBuf,
}

/// Which signer the environment asks for.
#[derive(Debug)]
pub enum SignerChoice {
    /// None configured: notaio validates and reports, and publishes nothing.
    None,
    /// A seed on disk, for kind and tests only (ADR-0004, decision 5).
    Development { key_file: PathBuf, key_id: String },
    /// OpenBao transit, the production signer.
    OpenBao(TransitConfig),
}

/// The signer `lookup` (the process environment, or a map in tests) configures.
///
/// # Errors
/// `SignError::Config` when both signers are configured, when the OpenBao address is not
/// `https://`, or when it has no CA file.
pub fn choose_signer(lookup: impl Fn(&str) -> Option<String>) -> Result<SignerChoice, SignError> {
    let dev = lookup(ENV_DEV_KEY_FILE);
    let addr = lookup(ENV_ADDR);
    let or = |name: &str, default: &str| lookup(name).unwrap_or_else(|| default.to_string());
    match (dev, addr) {
        (None, None) => Ok(SignerChoice::None),
        (Some(_), Some(_)) => Err(SignError::Config(format!(
            "both {ENV_DEV_KEY_FILE} and {ENV_ADDR} are set; configure one signer"
        ))),
        (Some(key_file), None) => Ok(SignerChoice::Development {
            key_file: key_file.into(),
            key_id: or(ENV_DEV_KEY_ID, DEFAULT_DEV_KEY_ID),
        }),
        (None, Some(addr)) => {
            if !addr.starts_with("https://") {
                return Err(SignError::Config(format!(
                    "{ENV_ADDR} must be https://, got {addr:?}"
                )));
            }
            let Some(ca_file) = lookup(ENV_CA_FILE) else {
                return Err(SignError::Config(format!(
                    "{ENV_CA_FILE} is required with {ENV_ADDR}"
                )));
            };
            Ok(SignerChoice::OpenBao(TransitConfig {
                addr: addr.trim_end_matches('/').to_string(),
                ca_file: ca_file.into(),
                role: or(ENV_ROLE, DEFAULT_ROLE),
                key: or(ENV_KEY, DEFAULT_KEY),
                auth_path: or(ENV_AUTH_PATH, DEFAULT_AUTH_PATH),
                token_file: or(ENV_TOKEN_FILE, DEFAULT_TOKEN_FILE).into(),
            }))
        }
    }
}

/// A transit signature, `vault:v<N>:<base64>`, as the key id `<key>:v<N>` (ADR-0004, decision 3)
/// and the raw signature.
///
/// # Errors
/// `SignError::Response` for anything else, including a signature of the wrong length.
pub fn parse_signature(key: &str, raw: &str) -> Result<(String, Vec<u8>), SignError> {
    let bad = |why: &str| SignError::Response(format!("transit signature {why}"));
    let rest = raw
        .strip_prefix("vault:v")
        .ok_or_else(|| bad("lacks the vault:v<N>: prefix"))?;
    let (version, b64) = rest.split_once(':').ok_or_else(|| bad("lacks a version"))?;
    let version: u32 = version.parse().map_err(|_| bad("has a bad version"))?;
    let sig = STANDARD.decode(b64).map_err(|_| bad("is not base64"))?;
    if sig.len() != ED25519_SIGNATURE_LEN {
        return Err(bad("is not an ed25519 signature"));
    }
    Ok((format!("{key}:v{version}"), sig))
}

/// The client token and its lease in seconds, from a Kubernetes auth login response.
///
/// # Errors
/// `SignError::Response` for a response without a token.
pub fn parse_login(body: &[u8]) -> Result<(String, u64), SignError> {
    #[derive(Deserialize)]
    struct Login {
        auth: Option<Auth>,
    }
    #[derive(Deserialize)]
    struct Auth {
        client_token: String,
        lease_duration: u64,
    }
    let login: Login =
        serde_json::from_slice(body).map_err(|e| SignError::Response(format!("login: {e}")))?;
    match login.auth {
        Some(a) if !a.client_token.is_empty() => Ok((a.client_token, a.lease_duration)),
        _ => Err(SignError::Response("login returned no token".into())),
    }
}

struct Token {
    value: String,
    refresh_at: Instant,
}

/// Signs through OpenBao transit.
pub struct TransitSigner {
    cfg: TransitConfig,
    http: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
    token: Mutex<Option<Token>>,
}

impl TransitSigner {
    /// A signer for `cfg`. TLS 1.3 only, and only `cfg.ca_file` is trusted.
    ///
    /// # Errors
    /// `SignError::Config` when the CA file cannot be read or holds no certificate.
    pub fn new(cfg: TransitConfig) -> Result<Self, SignError> {
        let ca = cfg.ca_file.display().to_string();
        let config = |why: String| SignError::Config(why);
        let pem = std::fs::read(&cfg.ca_file).map_err(|e| config(format!("{ca}: {e}")))?;
        let mut roots = rustls::RootCertStore::empty();
        for cert in CertificateDer::pem_slice_iter(&pem) {
            let cert = cert.map_err(|e| config(format!("{ca}: {e}")))?;
            roots.add(cert).map_err(|e| config(format!("{ca}: {e}")))?;
        }
        if roots.is_empty() {
            return Err(config(format!("{ca}: no certificate")));
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| config(e.to_string()))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls)
            .https_only()
            .enable_http1()
            .build();
        Ok(TransitSigner {
            cfg,
            http: Client::builder(TokioExecutor::new()).build(https),
            token: Mutex::new(None),
        })
    }

    /// The transit key this signer uses.
    pub fn key(&self) -> &str {
        &self.cfg.key
    }

    /// The key id and the raw ed25519 signature of `message`. Retries an unreachable, sealed or
    /// overloaded OpenBao with backoff, and logs in again once if the token is refused.
    ///
    /// # Errors
    /// The last `SignError` when every attempt fails. Nothing is ever signed any other way.
    pub async fn sign(&self, message: &[u8]) -> Result<(String, Vec<u8>), SignError> {
        let mut backoff = FIRST_BACKOFF;
        let mut fresh_login = false;
        let mut attempt = 1;
        loop {
            match self.sign_once(message, fresh_login).await {
                Ok(signed) => return Ok(signed),
                // The token may have expired or been revoked early: log in again, once.
                Err(SignError::Forbidden) if !fresh_login => fresh_login = true,
                Err(e) if e.retryable() && attempt < MAX_ATTEMPTS => {
                    tracing::warn!(error = %e, attempt, "OpenBao signing failed, retrying");
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn sign_once(
        &self,
        message: &[u8],
        fresh_login: bool,
    ) -> Result<(String, Vec<u8>), SignError> {
        #[derive(Deserialize)]
        struct Signed {
            data: SignedData,
        }
        #[derive(Deserialize)]
        struct SignedData {
            signature: String,
        }
        let token = self.token(fresh_login).await?;
        let path = format!("transit/sign/{}", self.cfg.key);
        let body = json!({ "input": STANDARD.encode(message) });
        let (status, bytes) = self.post(&path, Some(&token), &body).await?;
        if status == StatusCode::FORBIDDEN {
            return Err(SignError::Forbidden);
        }
        if !status.is_success() {
            return Err(status_error(status, &bytes));
        }
        let signed: Signed = serde_json::from_slice(&bytes)
            .map_err(|e| SignError::Response(format!("sign: {e}")))?;
        parse_signature(&self.cfg.key, &signed.data.signature)
    }

    /// A client token, from the cache unless it is due for refresh or `fresh` is set.
    async fn token(&self, fresh: bool) -> Result<String, SignError> {
        let mut cached = self.token.lock().await;
        if let Some(t) = cached
            .as_ref()
            .filter(|t| !fresh && Instant::now() < t.refresh_at)
        {
            return Ok(t.value.clone());
        }
        let t = self.login().await?;
        let value = t.value.clone();
        *cached = Some(t);
        Ok(value)
    }

    async fn login(&self) -> Result<Token, SignError> {
        // Read every time: the kubelet rotates the projected token.
        let jwt = tokio::fs::read_to_string(&self.cfg.token_file)
            .await
            .map_err(|e| SignError::Config(format!("{}: {e}", self.cfg.token_file.display())))?;
        let path = format!("auth/{}/login", self.cfg.auth_path);
        let body = json!({ "role": self.cfg.role, "jwt": jwt.trim() });
        let (status, bytes) = self.post(&path, None, &body).await?;
        if !status.is_success() {
            return Err(status_error(status, &bytes));
        }
        let (value, lease) = parse_login(&bytes)?;
        let refresh_in = Duration::from_secs(lease * REFRESH_NUMERATOR / REFRESH_DENOMINATOR);
        Ok(Token {
            value,
            refresh_at: Instant::now() + refresh_in,
        })
    }

    async fn post(
        &self,
        path: &str,
        token: Option<&str>,
        body: &Value,
    ) -> Result<(StatusCode, Bytes), SignError> {
        let mut req = Request::builder()
            .method(Method::POST)
            .uri(format!("{}/v1/{path}", self.cfg.addr))
            .header(http::header::CONTENT_TYPE, "application/json");
        if let Some(t) = token {
            req = req.header("X-Vault-Token", t);
        }
        let payload = serde_json::to_vec(body).map_err(|e| SignError::Response(e.to_string()))?;
        let req = req
            .body(Full::new(Bytes::from(payload)))
            .map_err(|e| SignError::Config(e.to_string()))?;
        let resp = tokio::time::timeout(REQUEST_TIMEOUT, self.http.request(req))
            .await
            .map_err(|_| SignError::Transport(format!("no answer within {REQUEST_TIMEOUT:?}")))?
            .map_err(|e| SignError::Transport(e.to_string()))?;
        let status = resp.status();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| SignError::Transport(e.to_string()))?
            .to_bytes();
        Ok((status, bytes))
    }
}

/// A non-2xx answer. The body is OpenBao's error list, never a secret.
fn status_error(status: StatusCode, body: &[u8]) -> SignError {
    SignError::Status {
        status: status.as_u16(),
        body: String::from_utf8_lossy(body).trim().to_string(),
    }
}

#[cfg(test)]
#[path = "openbao_tests.rs"]
mod openbao_tests;
