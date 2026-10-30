// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sandboxpolicy_types::{AudienceGrant, Budgets, EgressRule, KnobId, Lease, Step, ToolRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{jcs, BundleError};

pub const SCHEMA: &str = "sandbox.firestoned.io/bundle/v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRef {
    pub namespace: String,
    pub name: String,
    pub uid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRef {
    pub name: String,
    pub digest: String,
}

/// Everything that decides what a sandbox may do. The canonical form is RFC 8785 (ADR-0005), which
/// sorts members, so field order here does not affect the digest; adding, removing, renaming or
/// retyping a field still does, and is a schema change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleContent {
    pub policy: PolicyRef,
    pub profile: ProfileRef,
    /// Effective knob steps after applying active grants. Every registry knob is present.
    pub knobs: BTreeMap<KnobId, Step>,
    pub audiences: Vec<AudienceGrant>,
    pub egress: Vec<EgressRule>,
    pub tools: Vec<ToolRef>,
    pub lease: Lease,
    pub budgets: Budgets,
}

impl BundleContent {
    /// RFC 8785 canonical bytes of the content (ADR-0005).
    ///
    /// # Errors
    /// `BundleError::NonCanonical` when the content holds a number RFC 8785 cannot represent
    /// exactly: a float, or an integer outside the I-JSON safe range. Callers fail closed.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, BundleError> {
        let value =
            serde_json::to_value(self).map_err(|e| BundleError::NonCanonical(e.to_string()))?;
        jcs::canonicalize(&value)
    }

    /// `sha256:<hex>` over the canonical bytes.
    ///
    /// # Errors
    /// As [`BundleContent::canonical_bytes`].
    pub fn digest(&self) -> Result<String, BundleError> {
        let mut h = Sha256::new();
        h.update(self.canonical_bytes()?);
        Ok(format!("sha256:{}", hex_lower(&h.finalize())))
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleMeta {
    /// Monotonic. Bumps only when `contentDigest` changes.
    pub version: u64,
    pub issued_at: DateTime<Utc>,
    /// The bundle is worthless after this instant. Bounded by the earliest active grant expiry, so a
    /// relaxation lapses without any revocation mechanism.
    pub not_after: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundlePayload {
    pub schema: String,
    pub meta: BundleMeta,
    pub content: BundleContent,
    pub content_digest: String,
}

impl BundlePayload {
    /// A payload for `content`, with its digest.
    ///
    /// # Errors
    /// As [`BundleContent::canonical_bytes`]: content without a canonical form is never wrapped.
    pub fn new(meta: BundleMeta, content: BundleContent) -> Result<Self, BundleError> {
        let content_digest = content.digest()?;
        Ok(BundlePayload {
            schema: SCHEMA.to_string(),
            meta,
            content,
            content_digest,
        })
    }
}
