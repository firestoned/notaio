// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Classification of the data a profile may touch. It exists so the toxic-combination rule can be
/// evaluated: a `LabOnly` profile may combine untrusted content with an outbound channel because it
/// holds no sensitive data by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DataScope {
    LabOnly,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum Access {
    #[default]
    Read,
    Write,
}

/// One downstream audience the sandbox may obtain tokens for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
}

/// One egress allowlist entry, enforced at the gateway and in the host or network layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EgressRule {
    /// A DNS name. IP literals are refused (lint L007).
    pub host: String,
    /// The TCP port.
    pub port: u16,
    /// Method restriction. Only accepted at K4.1 R2 and above.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<String>,
    /// Path restriction. Only accepted at K4.1 R2 and above.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

/// A tool or MCP server the agent may use, pinned by manifest digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolRef {
    /// The tool or MCP server's name.
    pub name: String,
    /// `sha256:<hex>` of the tool manifest.
    pub manifest_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Lease {
    /// The longest a sandbox may run, in seconds.
    pub max_duration_seconds: u32,
    /// How many sandboxes one person may run at once.
    pub max_concurrent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Budgets {
    /// The most tool calls per lease.
    pub max_tool_calls: u32,
    /// The most wall-clock time per lease, in seconds.
    pub max_wall_clock_seconds: u32,
    /// The most bytes of egress per lease.
    pub max_egress_bytes: u64,
}
