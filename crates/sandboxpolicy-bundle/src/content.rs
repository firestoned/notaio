// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use std::collections::BTreeMap;

use jiff::Timestamp;
use sandboxpolicy_types::{Budgets, KnobId, Lease, Step};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    jcs, BundleAudience, BundleEgress, BundleError, BundleTool, IncludeRef, Resources, Sources,
    Subjects, PAYLOAD_TYPE_V3, PAYLOAD_TYPE_V4,
};

/// Bundle schema v3 (ADR-0013).
pub const SCHEMA_V3: &str = "sandbox.firestoned.io/bundle/v3";
/// Bundle schema v4 (ADR-0026): v3 plus `subjects`, `includes`, a per-entry `from`, and places for
/// `resources` and `sources`.
pub const SCHEMA_V4: &str = "sandbox.firestoned.io/bundle/v4";
/// The schema [`BundlePayload::new`] writes. v3 until the release that drops it (ADR-0026 decision
/// 9); a producer that writes v4 calls [`BundlePayload::new_v4`].
pub const SCHEMA: &str = SCHEMA_V3;

/// A bundle schema this crate reads. Ordered: v3 is older than v4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SchemaVersion {
    V3,
    V4,
}

impl SchemaVersion {
    /// The payload's `schema` string.
    pub fn schema(self) -> &'static str {
        match self {
            SchemaVersion::V3 => SCHEMA_V3,
            SchemaVersion::V4 => SCHEMA_V4,
        }
    }

    /// The DSSE payload type that pairs with this schema (ADR-0026 decision 3).
    pub fn payload_type(self) -> &'static str {
        match self {
            SchemaVersion::V3 => PAYLOAD_TYPE_V3,
            SchemaVersion::V4 => PAYLOAD_TYPE_V4,
        }
    }

    pub fn from_schema(schema: &str) -> Option<Self> {
        [SchemaVersion::V3, SchemaVersion::V4]
            .into_iter()
            .find(|v| v.schema() == schema)
    }

    pub fn from_payload_type(payload_type: &str) -> Option<Self> {
        [SchemaVersion::V3, SchemaVersion::V4]
            .into_iter()
            .find(|v| v.payload_type() == payload_type)
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRef {
    pub namespace: String,
    pub name: String,
    pub uid: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRef {
    pub name: String,
    pub digest: String,
}

/// Everything that decides what a sandbox may do. The canonical form is RFC 8785 (ADR-0005), which
/// sorts members, so field order here does not affect the digest; adding, removing, renaming or
/// retyping a field still does, and is a schema change.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleContent {
    pub policy: PolicyRef,
    pub profile: ProfileRef,
    /// Effective knob steps after applying active grants. Every registry knob is present.
    pub knobs: BTreeMap<KnobId, Step>,
    pub audiences: Vec<BundleAudience>,
    pub egress: Vec<BundleEgress>,
    pub tools: Vec<BundleTool>,
    pub lease: Lease,
    pub budgets: Budgets,
    /// v4 only, and always there in v4 (ADR-0024).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subjects: Option<Subjects>,
    /// v4 only: a composite's includes, sorted by name (ADR-0023 decision 7).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub includes: Vec<IncludeRef>,
    /// v4 only, once ADR-0019 is built.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,
    /// v4 only, once ADR-0016 is built.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Sources>,
}

impl BundleContent {
    /// Whether any member or entry field that only v4 has is set. A v3 bundle carrying one is
    /// refused (ADR-0026 decision 4), so v3 is never read as v4.
    pub fn has_v4_members(&self) -> bool {
        self.subjects.is_some()
            || !self.includes.is_empty()
            || self.resources.is_some()
            || self.sources.is_some()
            || self.audiences.iter().any(|a| a.from.is_some())
            || self.egress.iter().any(|e| e.from.is_some())
            || self.tools.iter().any(|t| t.from.is_some())
    }

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
        Ok(sha256_digest(&self.canonical_bytes()?))
    }
}

/// `sha256:<hex>` of `bytes`, the digest form every bundle kind uses.
pub(crate) fn sha256_digest(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{}", hex_lower(&h.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleMeta {
    /// Monotonic. Bumps only when `contentDigest` changes.
    pub version: u64,
    pub issued_at: Timestamp,
    /// The bundle is worthless after this instant. Bounded by the earliest active grant expiry, so a
    /// relaxation lapses without any revocation mechanism.
    pub not_after: Timestamp,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundlePayload {
    pub schema: String,
    pub meta: BundleMeta,
    pub content: BundleContent,
    pub content_digest: String,
}

impl BundlePayload {
    /// A payload of the default schema, [`SCHEMA`] (v3), for `content`, with its digest.
    ///
    /// # Errors
    /// As [`BundlePayload::new_v3`].
    pub fn new(meta: BundleMeta, content: BundleContent) -> Result<Self, BundleError> {
        Self::new_v3(meta, content)
    }

    /// A v3 payload for `content`.
    ///
    /// # Errors
    /// `BundleError::Malformed` when `content` has a v4 member, which no reader would accept in v3;
    /// otherwise as [`BundleContent::canonical_bytes`]: content without a canonical form is never
    /// wrapped.
    pub fn new_v3(meta: BundleMeta, content: BundleContent) -> Result<Self, BundleError> {
        if content.has_v4_members() {
            return Err(BundleError::Malformed(
                "v3 content cannot carry a v4 member (subjects, includes, from, resources, sources)"
                    .to_string(),
            ));
        }
        Self::with_schema(SchemaVersion::V3, meta, content)
    }

    /// A v4 payload for `content` (ADR-0026).
    ///
    /// # Errors
    /// `BundleError::Malformed` when `content` has no `subjects`, which v4 always carries
    /// (ADR-0024); otherwise as [`BundleContent::canonical_bytes`].
    pub fn new_v4(meta: BundleMeta, content: BundleContent) -> Result<Self, BundleError> {
        if content.subjects.is_none() {
            return Err(BundleError::Malformed(
                "v4 content must carry subjects".to_string(),
            ));
        }
        Self::with_schema(SchemaVersion::V4, meta, content)
    }

    fn with_schema(
        version: SchemaVersion,
        meta: BundleMeta,
        content: BundleContent,
    ) -> Result<Self, BundleError> {
        let content_digest = content.digest()?;
        Ok(BundlePayload {
            schema: version.schema().to_string(),
            meta,
            content,
            content_digest,
        })
    }

    /// Which schema the payload says it is, when this crate knows it.
    pub fn schema_version(&self) -> Option<SchemaVersion> {
        SchemaVersion::from_schema(&self.schema)
    }
}
