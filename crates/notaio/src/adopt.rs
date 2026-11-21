// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! Which ConfigMap notaio may publish a bundle to (ADR-0003, decision 1, amended).
//!
//! The ConfigMap's name is derived from the policy's or the audience's, so whoever names one of
//! those also names a ConfigMap. notaio publishes with a forced server-side apply and an owner
//! reference: without this check, a policy named `ca` would take over an existing `ca-bundle` in
//! its namespace, and deleting the policy would garbage-collect it.

use k8s_openapi::api::core::v1::ConfigMap;

/// The label on every ConfigMap notaio publishes.
pub const MANAGED_BY_LABEL: &str = "app.kubernetes.io/managed-by";
/// Its value: notaio's field manager.
pub const MANAGED_BY: &str = "notaio";
/// The `Ready` reason when the bundle's ConfigMap name belongs to something else.
pub const REASON_NAME_TAKEN: &str = "BundleNameTaken";

/// True when a ConfigMap already holds the name and is not notaio's: it lacks
/// [`MANAGED_BY_LABEL`]`=`[`MANAGED_BY`]. notaio then publishes nothing and leaves it untouched.
/// No ConfigMap at all is not foreign: the name is free.
pub fn is_foreign(existing: Option<&ConfigMap>) -> bool {
    let Some(cm) = existing else {
        return false;
    };
    cm.metadata
        .labels
        .as_ref()
        .and_then(|l| l.get(MANAGED_BY_LABEL))
        .is_none_or(|v| v != MANAGED_BY)
}

/// What the object's status says when the name is taken.
pub fn name_taken_message(name: &str) -> String {
    format!(
        "refused to publish: ConfigMap {name} exists and is not notaio's (no {MANAGED_BY_LABEL}={MANAGED_BY} label); rename this object or remove that ConfigMap"
    )
}

#[cfg(test)]
#[path = "adopt_tests.rs"]
mod adopt_tests;
