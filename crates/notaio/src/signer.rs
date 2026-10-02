use std::sync::Arc;

use anyhow::Context as _;
use sandboxpolicy_bundle::{Ed25519Signer, Signer};

pub type SharedSigner = Arc<dyn Signer + Send + Sync>;

/// Loads the development signer from `NOTAIO_SIGNING_KEY_FILE` (base64 of a 32 byte seed) and
/// `NOTAIO_KEY_ID`. Returns `None` when no key is configured: the controller then validates and
/// reports but never publishes a bundle. Unsigned bundles are never published.
///
/// This is deliberately a development path. The threat model (T8.2) says the signing key must not be
/// held by whoever administers the cluster, so the production signer is a KMS, HSM or TPM backed
/// implementation of `Signer`. See ADR-0002, open decision 2.
pub fn from_env() -> anyhow::Result<Option<SharedSigner>> {
    let Ok(path) = std::env::var("NOTAIO_SIGNING_KEY_FILE") else {
        return Ok(None);
    };
    let key_id = std::env::var("NOTAIO_KEY_ID").unwrap_or_else(|_| "dev".to_string());
    let raw = std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))?;
    let signer = Ed25519Signer::from_seed_b64(&key_id, &raw)?;
    Ok(Some(Arc::new(signer)))
}
