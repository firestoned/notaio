use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::Signer as _;
use serde::{Deserialize, Serialize};

use crate::{BundleError, BundlePayload};

pub const PAYLOAD_TYPE: &str = "application/vnd.firestoned.sandboxpolicy.bundle.v2+json";

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
/// memory or on its disk (T8.2); ADR-0004 backs it with a non-exportable OpenBao transit key.
/// `Ed25519Signer` is for development and tests.
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

/// What a signer signs for one bundle, and the payload bytes the envelope will carry.
pub struct SigningInput {
    /// The serialised payload, exactly as it goes into the envelope.
    pub payload: Vec<u8>,
    /// The DSSE pre-authentication encoding of `payload`: the bytes the signature covers.
    pub message: Vec<u8>,
}

/// The bytes to sign for `payload`. For a signer that signs out of band, such as OpenBao transit
/// (ADR-0004): sign `message`, then call [`envelope`].
///
/// # Errors
/// `BundleError::Malformed` if the payload cannot be serialised.
pub fn signing_input(payload: &BundlePayload) -> Result<SigningInput, BundleError> {
    let bytes = serde_json::to_vec(payload).map_err(|e| BundleError::Malformed(e.to_string()))?;
    let message = pae(PAYLOAD_TYPE, &bytes);
    Ok(SigningInput {
        payload: bytes,
        message,
    })
}

/// The envelope for `payload` (from [`signing_input`]) with one signature by `key_id`.
pub fn envelope(payload: &[u8], key_id: &str, signature: &[u8]) -> Envelope {
    Envelope {
        payload_type: PAYLOAD_TYPE.to_string(),
        payload: STANDARD.encode(payload),
        signatures: vec![Signature {
            keyid: key_id.to_string(),
            sig: STANDARD.encode(signature),
        }],
    }
}

/// Signs `payload` with an in-process signer.
///
/// # Errors
/// From [`signing_input`] and from the signer.
pub fn sign_bundle(payload: &BundlePayload, signer: &dyn Signer) -> Result<Envelope, BundleError> {
    let input = signing_input(payload)?;
    let sig = signer.sign(&input.message)?;
    Ok(envelope(&input.payload, signer.key_id(), &sig))
}
