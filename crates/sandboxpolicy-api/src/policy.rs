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
    version = "v1alpha1",
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
    pub subjects: Subjects,
    #[serde(default)]
    pub audiences: Vec<AudienceGrant>,
    #[serde(default)]
    pub egress: Vec<EgressRule>,
    #[serde(default)]
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
    pub name: String,
    pub access: Access,
    pub ttl_seconds: u32,
}

/// What a fully compromised sandbox under this policy could reach (objective O3): computed, not
/// asserted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MaxExposure {
    pub audiences: Vec<ExposedAudience>,
    pub egress_hosts: Vec<String>,
    pub write_capable: bool,
    pub tools: Vec<String>,
    pub lease_max_seconds: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SandboxPolicyStatus {
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
    /// Monotonic version of the compiled content. Bumps only when the content changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_version: Option<u64>,
    /// `sha256:<hex>` of the canonical bundle content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_not_after: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_exposure: Option<MaxExposure>,
    /// Names of the KnobGrants currently applied to this policy.
    #[serde(default)]
    pub active_grants: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_grant_expiry: Option<DateTime<Utc>>,
}
