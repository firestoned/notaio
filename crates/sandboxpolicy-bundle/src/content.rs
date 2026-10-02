use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sandboxpolicy_types::{AudienceGrant, Budgets, EgressRule, KnobId, Lease, Step, ToolRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCHEMA: &str = "sandbox.firestoned.io/bundle/v1";

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

/// Everything that decides what a sandbox may do. Field order here is part of the canonical form:
/// do not reorder fields without a schema bump.
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
    /// Canonical bytes of the content.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        // Serialising plain structs and BTreeMaps to JSON cannot fail.
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// `sha256:<hex>` over the canonical bytes.
    pub fn digest(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.canonical_bytes());
        format!("sha256:{}", hex_lower(&h.finalize()))
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
    pub fn new(meta: BundleMeta, content: BundleContent) -> Self {
        let content_digest = content.digest();
        BundlePayload {
            schema: SCHEMA.to_string(),
            meta,
            content,
            content_digest,
        }
    }
}
