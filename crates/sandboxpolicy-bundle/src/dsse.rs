use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::Signer as _;
use serde::{Deserialize, Serialize};

use crate::{BundleError, BundlePayload};

pub const PAYLOAD_TYPE: &str = "application/vnd.firestoned.sandboxpolicy.bundle.v1+json";

/// DSSE envelope, https://github.com/secure-systems-lab/dsse
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub payload_type: String,
    /// Standard base64 of the payload bytes.
    pub payload: String,
    pub signatures: Vec<Signature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub keyid: String,
    /// Standard base64 of the raw 64 byte signature.
    pub sig: String,
}

/// DSSE pre-authentication encoding. Signatures cover this, never the bare payload.
pub(crate) fn pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"DSSEv1 ");
    out.extend_from_slice(payload_type.len().to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(payload_type.as_bytes());
    out.push(b' ');
    out.extend_from_slice(payload.len().to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(payload);
    out
}

/// Something that can sign. The production implementation must not hold the key in the controller's
/// memory or on its disk (T8.2): back it with a KMS, HSM or TPM. `Ed25519Signer` is for development
/// and tests.
pub trait Signer {
    fn key_id(&self) -> &str;
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, BundleError>;
}

pub struct Ed25519Signer {
    key_id: String,
    key: ed25519_dalek::SigningKey,
}

impl Ed25519Signer {
    pub fn from_seed(key_id: &str, seed: [u8; 32]) -> Self {
        Ed25519Signer {
            key_id: key_id.to_string(),
            key: ed25519_dalek::SigningKey::from_bytes(&seed),
        }
    }

    /// Seed as standard base64 of 32 bytes, for example the content of a mounted file.
    pub fn from_seed_b64(key_id: &str, b64: &str) -> Result<Self, BundleError> {
        let raw = STANDARD
            .decode(b64.trim())
            .map_err(|e| BundleError::Key(e.to_string()))?;
        let seed: [u8; 32] = raw
            .try_into()
            .map_err(|_| BundleError::Key("seed must be 32 bytes".to_string()))?;
        Ok(Self::from_seed(key_id, seed))
    }

    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
}

impl Signer for Ed25519Signer {
    fn key_id(&self) -> &str {
        &self.key_id
    }

    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, BundleError> {
        Ok(self.key.sign(message).to_bytes().to_vec())
    }
}

pub fn sign_bundle(payload: &BundlePayload, signer: &dyn Signer) -> Result<Envelope, BundleError> {
    let bytes = serde_json::to_vec(payload).map_err(|e| BundleError::Malformed(e.to_string()))?;
    let sig = signer.sign(&pae(PAYLOAD_TYPE, &bytes))?;
    Ok(Envelope {
        payload_type: PAYLOAD_TYPE.to_string(),
        payload: STANDARD.encode(&bytes),
        signatures: vec![Signature {
            keyid: signer.key_id().to_string(),
            sig: STANDARD.encode(sig),
        }],
    })
}
