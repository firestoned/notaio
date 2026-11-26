// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! Who includes whom (ADR-0023): the relations the controller needs to keep composites and their
//! includes current, as pure functions over the policies it can see.

use std::collections::BTreeSet;

use kube::runtime::reflector::ObjectRef;
use sandboxpolicy_api::SandboxPolicy;

fn namespace_of(p: &SandboxPolicy) -> &str {
    p.metadata.namespace.as_deref().unwrap_or_default()
}

fn name_of(p: &SandboxPolicy) -> &str {
    p.metadata.name.as_deref().unwrap_or_default()
}

/// The names of the policies in `namespace` whose `includes` name `name`, sorted: the include's
/// `status.includedBy`.
pub fn included_by<'a>(
    policies: impl IntoIterator<Item = &'a SandboxPolicy>,
    namespace: &str,
    name: &str,
) -> Vec<String> {
    let found: BTreeSet<&str> = policies
        .into_iter()
        .filter(|p| namespace_of(p) == namespace)
        .filter(|p| p.spec.includes.iter().any(|i| i.name == name))
        .map(name_of)
        .collect();
    found.into_iter().map(str::to_string).collect()
}

/// The policies to re-evaluate when `changed` changes or is deleted, besides itself:
/// - every policy in its namespace that includes it, whose effective content may have moved;
/// - every policy it includes now (`spec.includes`) or did when last compiled (`status.includes`),
///   whose `includedBy` may have moved.
pub fn related<'a>(
    policies: impl IntoIterator<Item = &'a SandboxPolicy>,
    changed: &SandboxPolicy,
) -> Vec<ObjectRef<SandboxPolicy>> {
    let ns = namespace_of(changed);
    let me = name_of(changed);
    let mut names: BTreeSet<String> = included_by(policies, ns, me).into_iter().collect();
    names.extend(changed.spec.includes.iter().map(|i| i.name.clone()));
    if let Some(status) = &changed.status {
        names.extend(status.includes.iter().map(|i| i.name.clone()));
    }
    names.remove(me);
    names
        .into_iter()
        .map(|n| ObjectRef::new(&n).within(ns))
        .collect()
}

#[cfg(test)]
#[path = "include_tests.rs"]
mod include_tests;
