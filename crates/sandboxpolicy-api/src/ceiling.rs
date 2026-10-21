use kube::CustomResource;
use sandboxpolicy_types::Access;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Approval;

/// One audience a policy may expose, with the most access and the longest token it may have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CeilingAudience {
    pub name: String,
    /// `Write` allows `Read`.
    pub access: Access,
    pub max_ttl_seconds: u32,
}

/// The approved maximum exposure of the SandboxPolicy with the same name and namespace
/// (ADR-0007). A policy whose computed `status.maxExposure` this does not cover publishes no
/// bundle, and neither does a policy without one. Changes only through review and GitOps, like
/// the other kinds; the controller reads it and never writes it.
#[derive(CustomResource, Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "sandbox.firestoned.io",
    version = "v1alpha1",
    kind = "ExposureCeiling",
    namespaced,
    shortname = "exposureceiling",
    printcolumn = r#"{"name":"Lease","type":"integer","jsonPath":".spec.maxLeaseSeconds"}"#,
    printcolumn = r#"{"name":"Age","type":"date","jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct ExposureCeilingSpec {
    #[serde(default)]
    pub audiences: Vec<CeilingAudience>,
    /// `host:port`, as in `status.maxExposure.egressHosts`.
    #[serde(default)]
    pub egress_hosts: Vec<String>,
    /// Tool names.
    #[serde(default)]
    pub tools: Vec<String>,
    pub max_lease_seconds: u32,
    /// Must include one from the `Security` role (lint L016).
    pub approvals: Vec<Approval>,
}
