// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use chrono::{DateTime, Utc};
use kube::CustomResource;
use sandboxpolicy_types::{Access, AudienceGrant, Budgets, EgressRule, Lease, ToolRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Condition;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Subjects {
    /// Identity provider group ids allowed to claim a sandbox under this policy.
    #[serde(default)]
    pub groups: Vec<String>,
}

/// Binds subjects to a profile and adds the concrete audiences, egress entries and tools. The
/// controller checks all of it against the profile's knob ceilings and compiles a signed bundle.
#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "sandbox.firestoned.io",
    version = "v1alpha2",
    kind = "SandboxPolicy",
    namespaced,
    status = "SandboxPolicyStatus",
    shortname = "sbpolicy",
    printcolumn = r#"{"name":"Profile","type":"string","jsonPath":".spec.profileRef"}"#,
    printcolumn = r#"{"name":"Version","type":"integer","jsonPath":".status.bundleVersion"}"#,
    printcolumn = r#"{"name":"Ready","type":"string","jsonPath":".status.conditions[?(@.type==\"Ready\")].status"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct SandboxPolicySpec {
    /// Name of a cluster-scoped SandboxProfile.
    pub profile_ref: String,
    #[serde(default)]
    /// Who may claim a sandbox under this policy.
    pub subjects: Subjects,
    #[serde(default)]
    /// Downstream services the sandbox may obtain tokens for.
    pub audiences: Vec<AudienceGrant>,
    #[serde(default)]
    /// The egress allowlist.
    pub egress: Vec<EgressRule>,
    #[serde(default)]
    /// Tools and MCP servers the agent may use.
    pub tools: Vec<ToolRef>,
    /// Lease limits. Defaults to the K7.3 ceiling for the profile when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<Lease>,
    /// Budgets. Defaults to the ceilings for the profile when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budgets: Option<Budgets>,
}

/// One audience in the computed maximum exposure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExposedAudience {
    /// The audience.
    pub name: String,
    /// `Read` or `Write`.
    pub access: Access,
    /// The token lifetime, in seconds.
    pub ttl_seconds: u32,
    /// Where the token can be used, `host:port`.
    #[serde(default)]
    pub endpoints: Vec<String>,
}

/// What a fully compromised sandbox under this policy could reach (objective O3): computed, not
/// asserted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MaxExposure {
    /// Every audience the sandbox can get a token for.
    pub audiences: Vec<ExposedAudience>,
    /// Every `host:port` it can reach.
    pub egress_hosts: Vec<String>,
    /// Whether any audience grants `Write`.
    pub write_capable: bool,
    /// Every tool it can use.
    pub tools: Vec<String>,
    /// The longest lease, in seconds.
    pub lease_max_seconds: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SandboxPolicyStatus {
    #[serde(default)]
    /// `Ready`, `Valid` and `ProfileResolved`.
    pub conditions: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The generation the status was computed from.
    pub observed_generation: Option<i64>,
    /// Monotonic version of the compiled content. Bumps only when the content changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_version: Option<u64>,
    /// `sha256:<hex>` of the canonical bundle content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Digest of the profile the bundle was built on.
    pub profile_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// When the published bundle stops being valid.
    pub bundle_not_after: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_exposure: Option<MaxExposure>,
    /// Names of the KnobGrants currently applied to this policy.
    #[serde(default)]
    pub active_grants: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The soonest expiry among the active grants.
    pub next_grant_expiry: Option<DateTime<Utc>>,
}
