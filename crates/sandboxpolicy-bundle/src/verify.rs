// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::Verifier as _;
use jiff::{SignedDuration, Timestamp};

use crate::dsse::pae;
use crate::{BundleError, BundleMeta, BundlePayload, Envelope, PAYLOAD_TYPE, SCHEMA};

/// The set of public keys a consumer trusts, by key id.
#[derive(Default)]
pub struct TrustedKeys(BTreeMap<String, ed25519_dalek::VerifyingKey>);

impl TrustedKeys {
    pub fn new() -> Self {
        Self::default()
    }

    /// Trust `public` under `key_id`.
    ///
    /// # Errors
    /// `BundleError::Key` when `public` is not an Ed25519 public key, or when `key_id` is already
    /// trusted: a second key under the same id would silently replace the first, and which one a
    /// consumer trusts would depend on the order its configuration lists them.
    pub fn insert(&mut self, key_id: &str, public: [u8; 32]) -> Result<(), BundleError> {
        if self.0.contains_key(key_id) {
            return Err(BundleError::Key(format!(
                "key id {key_id} is already trusted"
            )));
        }
        let key = ed25519_dalek::VerifyingKey::from_bytes(&public)
            .map_err(|e| BundleError::Key(e.to_string()))?;
        self.0.insert(key_id.to_string(), key);
        Ok(())
    }

    /// The trusted key ids, in order.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

/// The key ids only: what a log line needs, and nothing a reader could mistake for the keys.
impl std::fmt::Debug for TrustedKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.ids()).finish()
    }
}

/// The longest bundle lifetime a consumer accepts by default: one day, the upper bound of the
/// controller's own `NOTAIO_BUNDLE_LIFETIME_SECONDS` (ADR-0018), so every bundle notaio can sign
/// passes, and a bundle signed to outlive that is refused (ADR-0002, decision 5).
pub const MAX_BUNDLE_LIFETIME_SECONDS: i64 = 86_400;

/// How far a bundle's `issuedAt` may run ahead of the consumer's clock by default.
const DEFAULT_MAX_CLOCK_SKEW_SECONDS: i64 = 60;

pub struct VerifyOptions {
    pub now: Timestamp,
    /// Reject anything older than this version. Consumers persist the highest version they have
    /// accepted, which is what stops a downgrade to a looser bundle.
    pub min_version: Option<u64>,
    pub max_clock_skew: SignedDuration,
    /// Reject a bundle whose `notAfter` is further than this from its `issuedAt`. Expiry is what
    /// ends a relaxation on its own (ADR-0002, decision 4); a bundle signed with a far `notAfter`,
    /// by a compromised or misconfigured signer, would otherwise stay valid for as long as it says.
    /// A consumer may lower it, never usefully raise it past what notaio signs.
    pub max_lifetime: SignedDuration,
}

impl VerifyOptions {
    pub fn at(now: Timestamp) -> Self {
        VerifyOptions {
            now,
            min_version: None,
            max_clock_skew: SignedDuration::from_secs(DEFAULT_MAX_CLOCK_SKEW_SECONDS),
            max_lifetime: SignedDuration::from_secs(MAX_BUNDLE_LIFETIME_SECONDS),
        }
    }
}

/// Verify an envelope and return the payload. Fails closed on every check.
///
/// # Errors
/// `BundleError` for a wrong payload type, no signature from a trusted key, a wrong schema, a
/// digest that does not match the content, a lifetime over `opts.max_lifetime`, a bundle not yet
/// valid or expired, or a version below `opts.min_version` (ADR-0002, decision 5; T5.4).
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
    check_signature(env, &bytes, keys)?;

    let payload: BundlePayload =
        serde_json::from_slice(&bytes).map_err(|e| BundleError::Malformed(e.to_string()))?;
    if payload.schema != SCHEMA {
        return Err(BundleError::Schema(payload.schema));
    }
    if payload.content.digest()? != payload.content_digest {
        return Err(BundleError::DigestMismatch);
    }
    check_window(&payload.meta, opts)?;
    Ok(payload)
}

/// At least one signature over the DSSE encoding of `bytes`, under the envelope's payload type,
/// from a trusted key.
pub(crate) fn check_signature(
    env: &Envelope,
    bytes: &[u8],
    keys: &TrustedKeys,
) -> Result<(), BundleError> {
    let message = pae(&env.payload_type, bytes);
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
    Ok(())
}

/// The lifetime, the validity window and the minimum version, shared by every bundle kind.
pub(crate) fn check_window(meta: &BundleMeta, opts: &VerifyOptions) -> Result<(), BundleError> {
    let lifetime = meta.not_after.duration_since(meta.issued_at);
    if lifetime > opts.max_lifetime {
        return Err(BundleError::LifetimeTooLong {
            lifetime_seconds: lifetime.as_secs(),
            max_seconds: opts.max_lifetime.as_secs(),
        });
    }
    if meta.issued_at > opts.now + opts.max_clock_skew {
        return Err(BundleError::NotYetValid(meta.issued_at.to_string()));
    }
    if meta.not_after <= opts.now {
        return Err(BundleError::Expired(meta.not_after.to_string()));
    }
    if let Some(min) = opts.min_version {
        if meta.version < min {
            return Err(BundleError::Downgrade {
                got: meta.version,
                min,
            });
        }
    }
    Ok(())
}
