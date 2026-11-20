// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use jiff::Timestamp;
use kube::CustomResource;
use sandboxpolicy_types::AudienceDefinition;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Condition;

/// The definition of one audience (ADR-0020). In the platform audience namespace it is a platform
/// audience, usable wherever a signed grant reaches; in any other namespace it is a team audience,
/// usable and grantable only there. It holds references to credential material, never the
/// material, and grants nothing: `SandboxPolicy` grants, and `ExposureCeiling` bounds. The
/// controller compiles each valid one into its own signed bundle, and never writes the object.
#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "sandbox.firestoned.io",
    version = "v1alpha2",
    kind = "SandboxAudience",
    namespaced,
    status = "SandboxAudienceStatus",
    shortname = "sbaudience",
    printcolumn = r#"{"name":"Backend","type":"string","jsonPath":".spec.credential.backend"}"#,
    printcolumn = r#"{"name":"Placement","type":"string","jsonPath":".spec.credential.placement"}"#,
    printcolumn = r#"{"name":"Version","type":"integer","jsonPath":".status.bundleVersion"}"#,
    printcolumn = r#"{"name":"Ready","type":"string","jsonPath":".status.conditions[?(@.type==\"Ready\")].status"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct SandboxAudienceSpec {
    /// The definition, signed as it is.
    #[serde(flatten)]
    pub definition: AudienceDefinition,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SandboxAudienceStatus {
    #[serde(default)]
    /// `Ready` and `Valid`.
    pub conditions: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The generation the status was computed from.
    pub observed_generation: Option<i64>,
    /// Monotonic version of the definition. Bumps only when the content changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_version: Option<u64>,
    /// `sha256:<hex>` of the canonical bundle content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// When the published bundle stops being valid.
    pub bundle_not_after: Option<Timestamp>,
}
