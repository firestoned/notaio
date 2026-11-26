// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! A policy composed of others (ADR-0022): `compose` turns a policy and the policies it includes
//! into the effective policy that is linted, checked against the composite's own ceiling and
//! compiled. Pure, like the rest of this crate.
//!
//! An include contributes its named parts only (audiences by `name`, egress by `endpoint`, tools by
//! `name`). Everything sandbox-wide stays the composite's. An entry the composite declares replaces
//! an included one with the same key, whole; two includes supplying a key the composite does not
//! declare are refused (L031). One level only (L032), and the composite's groups must be in every
//! include's (L033). The order of `includes` means nothing.

use std::collections::{BTreeMap, BTreeSet};

use sandboxpolicy_api::{IncludeStatus, SandboxPolicySpec, MAX_INCLUDES};
use sandboxpolicy_bundle::{jcs, BundleError};
use sandboxpolicy_types::{AudienceGrant, EgressRule, ToolRef};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::lint::Finding;

/// The named parts an include contributes, as they appear in finding paths.
const AUDIENCES: &str = "audiences";
const EGRESS: &str = "egress";
const TOOLS: &str = "tools";

/// A policy that may be included: one of the candidates `compose` picks from by name.
#[derive(Debug, Clone, Copy)]
pub struct Included<'a> {
    /// The policy's name, which `includes` refers to.
    pub name: &'a str,
    /// Its uid, recorded in the composite's status.
    pub uid: &'a str,
    pub spec: &'a SandboxPolicySpec,
}

/// Where each entry of the effective named parts came from: `None` for the composite's own,
/// otherwise the include's name. Each list runs parallel to the same list in the effective spec.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Origins {
    pub audiences: Vec<Option<String>>,
    pub egress: Vec<Option<String>>,
    pub tools: Vec<Option<String>>,
}

/// A composite with its includes resolved.
#[derive(Debug, Clone)]
pub struct EffectivePolicy {
    /// The composite's spec with the included entries added after its own, includes in name
    /// order, and `includes` cleared: what is linted and compiled.
    pub spec: SandboxPolicySpec,
    pub origins: Origins,
    /// Each include as it was composed, sorted by name (ADR-0022, decision 7).
    pub includes: Vec<IncludeStatus>,
}

impl EffectivePolicy {
    /// Name the include in every finding about an entry that came from one, so the author of the
    /// composite knows which policy to look at. Findings about anything else are left alone.
    pub fn attribute(&self, findings: &mut [Finding]) {
        for f in findings {
            let Some(origin) = self.origin_of(&f.path) else {
                continue;
            };
            f.message = format!("from include {origin}: {}", f.message);
        }
    }

    fn origin_of(&self, path: &str) -> Option<&str> {
        let (part, rest) = path.split_once('[')?;
        let index: usize = rest.split_once(']')?.0.parse().ok()?;
        let list = match part {
            AUDIENCES => &self.origins.audiences,
            EGRESS => &self.origins.egress,
            TOOLS => &self.origins.tools,
            _ => return None,
        };
        list.get(index)?.as_deref()
    }
}

/// Why a policy could not be composed.
#[derive(Debug)]
pub enum ComposeError {
    /// Includes that are not among the candidates, in the order the policy names them.
    Missing(Vec<String>),
    /// L031, L032 or L033 findings.
    Invalid(Vec<Finding>),
    /// An include's contribution has no RFC 8785 canonical form (ADR-0005).
    NotCanonical(BundleError),
}

/// What an include contributes, normalised so that order means nothing: the input to its
/// `contentDigest`.
#[derive(Serialize)]
struct Contribution {
    audiences: Vec<AudienceGrant>,
    egress: Vec<EgressRule>,
    tools: Vec<ToolRef>,
}

/// `sha256:` of the RFC 8785 form of what `spec` contributes: its audiences, egress entries and
/// tools, each list and each entry's own lists sorted. Lease, knobs and subjects are not
/// contributed, so they are not in it.
///
/// # Errors
/// `BundleError` when the contribution has no canonical form.
pub fn content_digest(spec: &SandboxPolicySpec) -> Result<String, BundleError> {
    let mut audiences = spec.audiences.clone();
    for a in &mut audiences {
        a.scopes.sort();
        a.endpoints.sort();
        a.rules.sort();
    }
    audiences.sort_by(|a, b| a.name.cmp(&b.name));
    let mut egress = spec.egress.clone();
    for e in &mut egress {
        e.rules.sort();
    }
    egress.sort_by(|a, b| a.endpoint.cmp(&b.endpoint));
    let mut tools = spec.tools.clone();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    let value = serde_json::to_value(Contribution {
        audiences,
        egress,
        tools,
    })
    .map_err(|e| BundleError::NonCanonical(e.to_string()))?;
    let bytes = jcs::canonicalize(&value)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(&bytes))))
}

/// The shape of `includes` itself (L032): at most [`MAX_INCLUDES`], each name once, and none on a
/// policy that is not published. The API server enforces the same, but `notaio lint` has none.
fn lint_shape(policy: &SandboxPolicySpec) -> Vec<Finding> {
    let mut out = Vec::new();
    if !policy.publish {
        out.push(Finding::err(
            "L032",
            "publish",
            "a policy with publish: false cannot have includes: it could be neither run nor included",
        ));
    }
    if policy.includes.len() > MAX_INCLUDES {
        out.push(Finding::err(
            "L032",
            "includes",
            format!(
                "at most {MAX_INCLUDES} includes, found {}",
                policy.includes.len()
            ),
        ));
    }
    let mut seen = BTreeSet::new();
    for (i, inc) in policy.includes.iter().enumerate() {
        if !seen.insert(inc.name.as_str()) {
            out.push(Finding::err(
                "L032",
                format!("includes[{i}]"),
                format!("{} is included twice", inc.name),
            ));
        }
    }
    out
}

/// One named part, merged: the composite's entries, then each include's in name order, skipping
/// a key the composite declares and refusing (L031) a key two includes supply.
fn merge<T: Clone>(
    part: &str,
    own: &[T],
    includes: &[(&str, &[T])],
    key: impl Fn(&T) -> &str,
    findings: &mut Vec<Finding>,
) -> (Vec<T>, Vec<Option<String>>) {
    let declared: BTreeSet<&str> = own.iter().map(&key).collect();
    let mut entries: Vec<T> = own.to_vec();
    let mut origins: Vec<Option<String>> = vec![None; own.len()];
    let mut supplier: BTreeMap<&str, &str> = BTreeMap::new();
    for (name, list) in includes {
        for entry in *list {
            let k = key(entry);
            if declared.contains(k) {
                continue;
            }
            if let Some(first) = supplier.get(k) {
                findings.push(Finding::err(
                    "L031",
                    "includes",
                    format!(
                        "{part} {k} is supplied by both {first} and {name}; declare it in this \
                         policy to choose"
                    ),
                ));
                continue;
            }
            supplier.insert(k, name);
            entries.push(entry.clone());
            origins.push(Some((*name).to_string()));
        }
    }
    (entries, origins)
}

/// Compose `policy` with the policies it includes, picked by name from `candidates` (any others
/// are ignored). A policy with no includes composes to itself.
///
/// # Errors
/// `Missing` when an included name is not among the candidates, `Invalid` with L031, L032 or
/// L033 findings, and `NotCanonical` when an include's contribution cannot be digested.
pub fn compose(
    policy: &SandboxPolicySpec,
    candidates: &[Included<'_>],
) -> Result<EffectivePolicy, ComposeError> {
    if policy.includes.is_empty() {
        return Ok(EffectivePolicy {
            spec: policy.clone(),
            origins: Origins {
                audiences: vec![None; policy.audiences.len()],
                egress: vec![None; policy.egress.len()],
                tools: vec![None; policy.tools.len()],
            },
            includes: vec![],
        });
    }
    let mut findings = lint_shape(policy);

    let missing: Vec<String> = policy
        .includes
        .iter()
        .filter(|i| !candidates.iter().any(|c| c.name == i.name))
        .map(|i| i.name.clone())
        .collect();
    if !missing.is_empty() && findings.is_empty() {
        return Err(ComposeError::Missing(missing));
    }

    // Each include once, in name order, with the index the policy names it at.
    let mut resolved: BTreeMap<&str, (usize, &Included<'_>)> = BTreeMap::new();
    for (i, inc) in policy.includes.iter().enumerate() {
        let Some(c) = candidates.iter().find(|c| c.name == inc.name) else {
            continue;
        };
        resolved.entry(c.name).or_insert((i, c));
    }

    for (name, (i, inc)) in &resolved {
        if !inc.spec.includes.is_empty() {
            findings.push(Finding::err(
                "L032",
                format!("includes[{i}]"),
                format!(
                    "{name} has includes of its own: an included policy may not include others"
                ),
            ));
        }
        let allowed: BTreeSet<&str> = inc
            .spec
            .subjects
            .groups
            .iter()
            .map(String::as_str)
            .collect();
        let wider: Vec<&str> = policy
            .subjects
            .groups
            .iter()
            .map(String::as_str)
            .filter(|g| !allowed.contains(g))
            .collect();
        if !wider.is_empty() {
            findings.push(Finding::err(
                "L033",
                "subjects.groups",
                format!(
                    "{} not in the groups of include {name}: a composite may not hand an \
                     include's grants to groups its author did not",
                    wider.join(", ")
                ),
            ));
        }
    }

    let ordered: Vec<(&str, &Included<'_>)> =
        resolved.iter().map(|(n, (_, inc))| (*n, *inc)).collect();
    let audience_lists: Vec<(&str, &[AudienceGrant])> = ordered
        .iter()
        .map(|(n, inc)| (*n, inc.spec.audiences.as_slice()))
        .collect();
    let egress_lists: Vec<(&str, &[EgressRule])> = ordered
        .iter()
        .map(|(n, inc)| (*n, inc.spec.egress.as_slice()))
        .collect();
    let tool_lists: Vec<(&str, &[ToolRef])> = ordered
        .iter()
        .map(|(n, inc)| (*n, inc.spec.tools.as_slice()))
        .collect();
    let (audiences, aud_origins) = merge(
        AUDIENCES,
        &policy.audiences,
        &audience_lists,
        |a| a.name.as_str(),
        &mut findings,
    );
    let (egress, egress_origins) = merge(
        EGRESS,
        &policy.egress,
        &egress_lists,
        |e| e.endpoint.as_str(),
        &mut findings,
    );
    let (tools, tool_origins) = merge(
        TOOLS,
        &policy.tools,
        &tool_lists,
        |t| t.name.as_str(),
        &mut findings,
    );
    if !findings.is_empty() {
        return Err(ComposeError::Invalid(findings));
    }

    let mut includes = Vec::with_capacity(ordered.len());
    for (name, inc) in &ordered {
        includes.push(IncludeStatus {
            name: (*name).to_string(),
            uid: inc.uid.to_string(),
            content_digest: content_digest(inc.spec).map_err(ComposeError::NotCanonical)?,
        });
    }
    let mut spec = policy.clone();
    spec.includes.clear();
    spec.audiences = audiences;
    spec.egress = egress;
    spec.tools = tools;
    Ok(EffectivePolicy {
        spec,
        origins: Origins {
            audiences: aud_origins,
            egress: egress_origins,
            tools: tool_origins,
        },
        includes,
    })
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod compose_tests;
