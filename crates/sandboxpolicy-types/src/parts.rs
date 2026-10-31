// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use serde::{Deserialize, Serialize};

/// Classification of the data a profile may touch. It exists so the toxic-combination rule can be
/// evaluated: a `LabOnly` profile may combine untrusted content with an outbound channel because it
/// holds no sensitive data by construction.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataScope {
    LabOnly,
    Internal,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Access {
    #[default]
    Read,
    Write,
}

/// One HTTP rule for an endpoint (ADR-0013): a method, or `*` for any, and a path glob in which `*`
/// matches one segment and `**` zero or more, matched against the path without the query. The
/// shape mediatore's egress proxy uses (its ADR-0025).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointRule {
    /// An HTTP method, or `*`.
    pub method: String,
    /// A path glob starting with `/`.
    pub path: String,
}

/// One downstream audience the sandbox may obtain tokens for.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudienceGrant {
    /// The audience: the downstream service.
    pub name: String,
    /// `Read` or `Write`. `Write` needs K5.2 above R0 (lint L006).
    #[serde(default)]
    pub access: Access,
    #[serde(default)]
    /// Scopes requested for the token.
    pub scopes: Vec<String>,
    /// Requested token lifetime. Defaults to the K5.1 ceiling when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u32>,
    /// Where the audience is reached, `host:port`, through the broker's proxy, which injects the
    /// token: brokered calls, allowed at any K4.1 step. Exact names only (lint L017).
    #[serde(default)]
    pub endpoints: Vec<String>,
    /// What may be called at those endpoints. None means every method and path.
    #[serde(default)]
    pub rules: Vec<EndpointRule>,
}

/// One direct egress entry: reached without the broker's proxy and without a token, enforced at
/// the gateway and in the host or network layer (ADR-0013).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EgressRule {
    /// `host:port`, a DNS name. IP literals are refused; a wildcard host needs K4.1 R3 (lint L007).
    pub endpoint: String,
    /// What may be called there. Only accepted at K4.1 R2 and above. None means anything.
    #[serde(default)]
    pub rules: Vec<EndpointRule>,
}

/// A tool or MCP server the agent may use, pinned by manifest digest.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRef {
    /// The tool or MCP server's name.
    pub name: String,
    /// `sha256:<hex>` of the tool manifest.
    pub manifest_digest: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lease {
    /// The longest a sandbox may run, in seconds.
    pub max_duration_seconds: u32,
    /// How many sandboxes one person may run at once.
    pub max_concurrent: u8,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Budgets {
    /// The most tool calls per lease.
    pub max_tool_calls: u32,
    /// The most wall-clock time per lease, in seconds.
    pub max_wall_clock_seconds: u32,
    /// The most bytes of egress per lease.
    pub max_egress_bytes: u64,
}
