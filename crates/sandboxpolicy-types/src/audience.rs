// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! The definition of one audience (ADR-0020): what it is, how the consumer obtains and places its
//! credential, where it is reached and the widest any grant of it may be. It carries references to
//! credential material, never the material. Shared by the `SandboxAudience` CRD and the signed
//! audience bundle, so it has no Kubernetes dependency.

use serde::{Deserialize, Serialize};

use crate::EndpointRule;

/// How the consumer obtains the credential, the vocabulary mediatore implements.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Backend {
    /// The consumer mints a token with its own signing key. Platform audiences only (L022).
    Sts,
    /// An operator-held credential, read from the source.
    Secret,
    /// The requester's own credential, delegated with the claim. Platform audiences only (L022).
    Delegated,
    /// Entra ID On-Behalf-Of with the consumer's app registration. Platform audiences only (L022).
    EntraObo,
    /// No credential: the audience is reached through the proxy, under its endpoints and rules.
    None,
}

/// Where the credential is placed.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Placement {
    /// At the gateway, outside the pool VM.
    Gateway,
    /// In the pool VM's egress proxy: the credential sits in the VM's memory (L029 for a team).
    Proxy,
}

/// What the credential is.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialFormat {
    /// A bearer token.
    #[default]
    Bearer,
    /// A username and a password or token, sent as HTTP Basic.
    Basic,
}

/// A path in the platform's OpenBao, read with the consumer's own identity. Platform audiences
/// only (L023).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenBaoSource {
    /// The KV mount.
    pub mount: String,
    /// The path under the mount.
    pub path: String,
    /// The key in the secret.
    pub key: String,
}

/// A Secret in the audience's own namespace. There is no namespace field on purpose: a reference
/// is always resolved where the audience lives (ADR-0020, decision 4).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretRef {
    /// The Secret's name.
    pub name: String,
    /// The key holding the token or password.
    pub token_key: String,
    /// The key holding the username, for `format: basic`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username_key: Option<String>,
}

/// Where the credential comes from: exactly one of the two.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialSource {
    /// The platform's OpenBao.
    Openbao(OpenBaoSource),
    /// A Secret in the audience's namespace.
    SecretRef(SecretRef),
}

/// One header set on the upstream request with a fixed value.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderPair {
    /// The header name, a lowercase token (L026).
    pub name: String,
    /// The value, as sent: printable ASCII, no line break (L026). Published in the bundle as
    /// written, so never a credential.
    pub value: String,
}

/// How a `delegated` credential is refreshed.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegatedRefresh {
    /// The token endpoint, an `https://` URL.
    pub token_endpoint: String,
    /// The OAuth client id.
    pub client_id: String,
    /// The scope to request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// A key of a ConfigMap in the audience's own namespace (no namespace field, as for
/// [`SecretRef`]).
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigMapKeyRef {
    /// The ConfigMap's name.
    pub name: String,
    /// The key holding the PEM bundle.
    pub key: String,
}

/// How the consumer obtains and places the credential.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudienceCredential {
    /// How the credential is obtained.
    pub backend: Backend,
    /// Where it is placed. Required: a misspelling must never fall back to a default.
    pub placement: Placement,
    /// What it is. Defaults to `bearer`.
    #[serde(default)]
    pub format: CredentialFormat,
    /// Where it comes from, for the `secret` backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<CredentialSource>,
    /// The header the credential is placed in, a lowercase token. Unset means `authorization`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    /// Further headers set with fixed values.
    #[serde(default)]
    pub extra_headers: Vec<HeaderPair>,
    /// Refresh settings for the `delegated` backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh: Option<DelegatedRefresh>,
}

/// The definition of one audience: the spec of a `SandboxAudience`, and the content of its signed
/// bundle.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudienceDefinition {
    /// The credential.
    pub credential: AudienceCredential,
    /// Where the audience is reached, `host:port`, exact names only (L017). Every one must be in
    /// the ceiling of each policy that grants the audience (L014).
    pub endpoints: Vec<String>,
    /// The widest any grant may be (L028). None means every method and path.
    #[serde(default)]
    pub rules: Vec<EndpointRule>,
    /// The longest token lifetime any grant may have (L028), and no more than the ceiling's (L014).
    pub max_ttl_seconds: u32,
    /// A CA bundle for an upstream on a private PKI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_ref: Option<ConfigMapKeyRef>,
}
