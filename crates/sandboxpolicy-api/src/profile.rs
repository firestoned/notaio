use std::collections::BTreeMap;

use kube::CustomResource;
use sandboxpolicy_types::{DataScope, KnobId, Step};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Condition;

/// A named bundle of knob steps (Fortress, Hardened, Standard, Lab). Cluster-scoped and
/// platform-owned: teams pick a profile, they do not edit one.
#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "sandbox.firestoned.io",
    version = "v1alpha1",
    kind = "SandboxProfile",
    status = "SandboxProfileStatus",
    shortname = "sbprofile",
    printcolumn = r#"{"name":"Scope","type":"string","jsonPath":".spec.dataScope"}"#,
    printcolumn = r#"{"name":"Valid","type":"string","jsonPath":".status.conditions[?(@.type==\"Valid\")].status"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct SandboxProfileSpec {
    #[serde(default)]
    pub description: String,
    /// `LabOnly` profiles hold no sensitive data, which is what allows them to combine untrusted
    /// content with an open outbound channel.
    pub data_scope: DataScope,
    /// Step per knob id. A knob that is absent stays at R0, the locked default.
    #[serde(default)]
    pub knobs: BTreeMap<KnobId, Step>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SandboxProfileStatus {
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
    /// Digest of the validated spec. Recorded in every bundle so a change to the profile is visible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}
