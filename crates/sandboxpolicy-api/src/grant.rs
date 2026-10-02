use chrono::{DateTime, Utc};
use kube::CustomResource;
use sandboxpolicy_types::{KnobId, Role, Step};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Condition;

/// A recorded approval by a role named in the knob register. `reference` points at the reviewed
/// change (a pull request URL, for example), because policy changes only arrive through review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub role: Role,
    pub reference: String,
}

/// A time-boxed relaxation of one knob for one policy. Every relaxation has an owner, a reason, an
/// approver and an expiry, so the knob dashboard is a query over this kind.
#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "sandbox.firestoned.io",
    version = "v1alpha1",
    kind = "KnobGrant",
    namespaced,
    status = "KnobGrantStatus",
    shortname = "knobgrant",
    printcolumn = r#"{"name":"Policy","type":"string","jsonPath":".spec.policyRef"}"#,
    printcolumn = r#"{"name":"Knob","type":"string","jsonPath":".spec.knob"}"#,
    printcolumn = r#"{"name":"Step","type":"string","jsonPath":".spec.step"}"#,
    printcolumn = r#"{"name":"Phase","type":"string","jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Expires","type":"string","jsonPath":".spec.expiresAt"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct KnobGrantSpec {
    /// SandboxPolicy in the same namespace.
    pub policy_ref: String,
    pub knob: KnobId,
    /// Target step. Must be exactly one step above the profile's step for this knob.
    pub step: Step,
    pub owner: String,
    pub reason: String,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    pub approvals: Vec<Approval>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum GrantPhase {
    #[default]
    Pending,
    Active,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnobGrantStatus {
    #[serde(default)]
    pub phase: GrantPhase,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
}
