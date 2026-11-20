// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! The signed audience bundle (ADR-0020, decision 9): one `SandboxAudience`'s definition, wrapped
//! like a policy bundle but under its own DSSE payload type and schema. DSSE signs the payload
//! type, so a policy bundle can never verify as an audience bundle, nor the reverse.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use sandboxpolicy_types::AudienceDefinition;
use serde::{Deserialize, Serialize};

use crate::dsse::{pae, signed_envelope};
use crate::verify::{check_signature, check_window};
use crate::{
    content::sha256_digest, jcs, BundleError, BundleMeta, Envelope, Signer, SigningInput,
    TrustedKeys, VerifyOptions,
};

/// The schema of an audience bundle's payload.
pub const AUDIENCE_SCHEMA: &str = "sandbox.firestoned.io/audience-bundle/v1";

/// The DSSE payload type of an audience bundle.
pub const AUDIENCE_PAYLOAD_TYPE: &str = "application/vnd.firestoned.sandboxaudience.bundle.v1+json";

/// Which `SandboxAudience` the bundle defines. The namespace decides whether it is a platform or a
/// team audience, and the consumer derives that from its own configuration, never from a field
/// here (ADR-0020, decision 2).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudienceRef {
    pub namespace: String,
    pub name: String,
    /// The incarnation, for the version floor (ADR-0008).
    pub uid: String,
}

/// Everything the audience bundle asserts. RFC 8785 canonical form (ADR-0005); adding, removing,
/// renaming or retyping a field is a schema change.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudienceBundleContent {
    pub audience: AudienceRef,
    pub definition: AudienceDefinition,
}

impl AudienceBundleContent {
    /// RFC 8785 canonical bytes of the content (ADR-0005).
    ///
    /// # Errors
    /// `BundleError::NonCanonical` for content RFC 8785 cannot represent exactly.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, BundleError> {
        let value =
            serde_json::to_value(self).map_err(|e| BundleError::NonCanonical(e.to_string()))?;
        jcs::canonicalize(&value)
    }

    /// `sha256:<hex>` over the canonical bytes.
    ///
    /// # Errors
    /// As [`AudienceBundleContent::canonical_bytes`].
    pub fn digest(&self) -> Result<String, BundleError> {
        Ok(sha256_digest(&self.canonical_bytes()?))
    }
}

/// The payload of an audience bundle: the same meta as a policy bundle (version, `issuedAt`,
/// `notAfter`), its own schema and content.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudienceBundlePayload {
    pub schema: String,
    pub meta: BundleMeta,
    pub content: AudienceBundleContent,
    pub content_digest: String,
}

impl AudienceBundlePayload {
    /// A payload for `content`, with its digest.
    ///
    /// # Errors
    /// As [`AudienceBundleContent::canonical_bytes`].
    pub fn new(meta: BundleMeta, content: AudienceBundleContent) -> Result<Self, BundleError> {
        let content_digest = content.digest()?;
        Ok(AudienceBundlePayload {
            schema: AUDIENCE_SCHEMA.to_string(),
            meta,
            content,
            content_digest,
        })
    }
}

/// The bytes to sign for an audience `payload`, for a signer that signs out of band (OpenBao
/// transit, ADR-0004): sign `message`, then call [`audience_envelope`].
///
/// # Errors
/// `BundleError::Malformed` if the payload cannot be serialised.
pub fn audience_signing_input(
    payload: &AudienceBundlePayload,
) -> Result<SigningInput, BundleError> {
    let bytes = serde_json::to_vec(payload).map_err(|e| BundleError::Malformed(e.to_string()))?;
    let message = pae(AUDIENCE_PAYLOAD_TYPE, &bytes);
    Ok(SigningInput {
        payload: bytes,
        message,
    })
}

/// The audience envelope for `payload` (from [`audience_signing_input`]) with one signature.
pub fn audience_envelope(payload: &[u8], key_id: &str, signature: &[u8]) -> Envelope {
    signed_envelope(AUDIENCE_PAYLOAD_TYPE, payload, key_id, signature)
}

/// Signs an audience `payload` with an in-process signer.
///
/// # Errors
/// From [`audience_signing_input`] and from the signer.
pub fn sign_audience_bundle(
    payload: &AudienceBundlePayload,
    signer: &dyn Signer,
) -> Result<Envelope, BundleError> {
    let input = audience_signing_input(payload)?;
    let sig = signer.sign(&input.message)?;
    Ok(audience_envelope(&input.payload, signer.key_id(), &sig))
}

/// Verify an audience envelope and return the payload. Fails closed on every check, in the same
/// order as [`crate::verify_bundle`]: payload type, signature, schema, digest, validity window,
/// minimum version.
///
/// # Errors
/// `BundleError::PayloadType` for anything but [`AUDIENCE_PAYLOAD_TYPE`] (a policy bundle
/// included), and otherwise as [`crate::verify_bundle`].
pub fn verify_audience_bundle(
    env: &Envelope,
    keys: &TrustedKeys,
    opts: &VerifyOptions,
) -> Result<AudienceBundlePayload, BundleError> {
    if env.payload_type != AUDIENCE_PAYLOAD_TYPE {
        return Err(BundleError::PayloadType(env.payload_type.clone()));
    }
    let bytes = STANDARD
        .decode(&env.payload)
        .map_err(|e| BundleError::Malformed(e.to_string()))?;
    check_signature(env, &bytes, keys)?;
    let payload: AudienceBundlePayload =
        serde_json::from_slice(&bytes).map_err(|e| BundleError::Malformed(e.to_string()))?;
    if payload.schema != AUDIENCE_SCHEMA {
        return Err(BundleError::Schema(payload.schema));
    }
    if payload.content.digest()? != payload.content_digest {
        return Err(BundleError::DigestMismatch);
    }
    check_window(&payload.meta, opts)?;
    Ok(payload)
}
