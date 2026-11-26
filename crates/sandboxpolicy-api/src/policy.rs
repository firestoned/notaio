// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use jiff::Timestamp;
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

/// The most policies one policy may include (ADR-0023, decision 1). Small enough that a reviewer
/// reads every grant of a composite one step away.
pub const MAX_INCLUDES: usize = 4;

/// Another `SandboxPolicy` in the same namespace whose audiences, egress entries and tools this
/// policy includes (ADR-0023).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyInclude {
    /// The included policy's name, in this policy's namespace.
    pub name: String,
}

fn default_publish() -> bool {
    true
}

// serde's `skip_serializing_if` passes a reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_true(b: &bool) -> bool {
    *b
}

/// Binds subjects to a profile and adds the concrete audiences, egress entries and tools. The
/// controller checks all of it against the profile's knob ceilings and compiles a signed bundle.
/// A policy may include others (`includes`): their audiences, egress entries and tools are added
/// to its own, and the result is checked against this policy's profile and ceiling (ADR-0023).
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
#[schemars(extend("x-kubernetes-validations" = [{
    "rule": "!has(self.includes) || !has(self.publish) || self.publish",
    "message": "a policy with publish: false cannot have includes (ADR-0023)"
}]))]
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
    /// Other policies in this namespace whose audiences, egress entries and tools this one
    /// includes, at most four, names unique. An entry this policy declares replaces an included
    /// one with the same key; two includes supplying the same key are refused (L031). An included
    /// policy may not include others (L032), and this policy's groups must be in every include's
    /// (L033). Everything else (profile, subjects, lease, budgets, knobs) is this policy's own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(
        length(max = MAX_INCLUDES),
        extend("x-kubernetes-list-type" = "map", "x-kubernetes-list-map-keys" = ["name"])
    )]
    pub includes: Vec<PolicyInclude>,
    /// `false` for a policy that exists only to be included: linted against its profile, but
    /// never compiled, signed or published, and needing no ExposureCeiling. A sandbox that names
    /// it finds no bundle. Defaults to `true` (ADR-0023, decision 8).
    #[serde(default = "default_publish", skip_serializing_if = "is_true")]
    pub publish: bool,
}

/// One included policy, as the composite was compiled from it (ADR-0023, decision 7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IncludeStatus {
    /// The included policy's name.
    pub name: String,
    /// Its uid, so a deleted and recreated include reads as a different one.
    #[serde(default)]
    pub uid: String,
    /// `sha256:<hex>` of the RFC 8785 form of what it contributes: its audiences, egress entries
    /// and tools, each sorted.
    pub content_digest: String,
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
    pub bundle_not_after: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_exposure: Option<MaxExposure>,
    /// Names of the KnobGrants currently applied to this policy.
    #[serde(default)]
    pub active_grants: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The soonest expiry among the active grants.
    pub next_grant_expiry: Option<Timestamp>,
    /// The policies this one includes, as it was last compiled from them (ADR-0023). Always
    /// written, empty included: status goes out as a merge patch, where absent means unchanged.
    #[serde(default)]
    pub includes: Vec<IncludeStatus>,
    /// The policies that include this one, sorted (ADR-0023). Always written, like `includes`.
    #[serde(default)]
    pub included_by: Vec<String>,
}
