// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A minimal condition type. Kept local so the CRD schema does not depend on the time
/// representation used by `k8s-openapi`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// The condition: `Ready`, `Valid`, `ProfileResolved` or `Applied`, by kind.
    #[serde(rename = "type")]
    pub type_: String,
    /// `True`, `False` or `Unknown`.
    pub status: String,
    /// The reason for the status, in CamelCase, such as `Published` or `SigningFailed`.
    pub reason: String,
    /// A human-readable explanation, with lint codes when findings caused it.
    pub message: String,
    /// When the status last changed.
    pub last_transition_time: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The object generation this condition was computed from.
    pub observed_generation: Option<i64>,
}
