use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::Verifier as _;

use crate::dsse::pae;
use crate::{BundleError, BundlePayload, Envelope, PAYLOAD_TYPE, SCHEMA};

/// The set of public keys a consumer trusts, by key id.
#[derive(Default)]
pub struct TrustedKeys(BTreeMap<String, ed25519_dalek::VerifyingKey>);

impl TrustedKeys {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key_id: &str, public: [u8; 32]) -> Result<(), BundleError> {
        let key = ed25519_dalek::VerifyingKey::from_bytes(&public)
            .map_err(|e| BundleError::Key(e.to_string()))?;
        self.0.insert(key_id.to_string(), key);
        Ok(())
    }
}

pub struct VerifyOptions {
    pub now: DateTime<Utc>,
    /// Reject anything older than this version. Consumers persist the highest version they have
    /// accepted, which is what stops a downgrade to a looser bundle.
    pub min_version: Option<u64>,
    pub max_clock_skew: Duration,
}

impl VerifyOptions {
    pub fn at(now: DateTime<Utc>) -> Self {
        VerifyOptions {
            now,
            min_version: None,
            max_clock_skew: Duration::seconds(60),
        }
    }
}

/// Verify an envelope and return the payload. Fails closed on every check.
pub fn verify_bundle(
    env: &Envelope,
    keys: &TrustedKeys,
    opts: &VerifyOptions,
) -> Result<BundlePayload, BundleError> {
    if env.payload_type != PAYLOAD_TYPE {
        return Err(BundleError::PayloadType(env.payload_type.clone()));
    }
    let bytes = STANDARD
        .decode(&env.payload)
        .map_err(|e| BundleError::Malformed(e.to_string()))?;
    let message = pae(&env.payload_type, &bytes);

    let verified = env.signatures.iter().any(|s| {
        let Some(key) = keys.0.get(&s.keyid) else {
            return false;
        };
        let Ok(raw) = STANDARD.decode(&s.sig) else {
            return false;
        };
        let Ok(sig) = ed25519_dalek::Signature::from_slice(&raw) else {
            return false;
        };
        key.verify(&message, &sig).is_ok()
    });
    if !verified {
        return Err(BundleError::NoValidSignature);
    }

    let payload: BundlePayload =
        serde_json::from_slice(&bytes).map_err(|e| BundleError::Malformed(e.to_string()))?;
    if payload.schema != SCHEMA {
        return Err(BundleError::Schema(payload.schema));
    }
    if payload.content.digest() != payload.content_digest {
        return Err(BundleError::DigestMismatch);
    }
    if payload.meta.issued_at > opts.now + opts.max_clock_skew {
        return Err(BundleError::NotYetValid(
            payload.meta.issued_at.to_rfc3339(),
        ));
    }
    if payload.meta.not_after <= opts.now {
        return Err(BundleError::Expired(payload.meta.not_after.to_rfc3339()));
    }
    if let Some(min) = opts.min_version {
        if payload.meta.version < min {
            return Err(BundleError::Downgrade {
                got: payload.meta.version,
                min,
            });
        }
    }
    Ok(payload)
}
