// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! `notaio lint`: the controller's checks, run on files, for pull requests (ADR-0006).
//!
//! The decisions are `sandboxpolicy_core`'s: [`lint_knobs`] on every profile and
//! [`evaluate`] on every policy, with its grants and its profile. This module only loads files,
//! resolves references and reports. Input problems have their own stable codes:
//!
//! | Code | Meaning |
//! | ---- | ------- |
//! | I001 | An object in the `sandbox.firestoned.io` group is not a valid SandboxProfile, SandboxPolicy or KnobGrant |
//! | I002 | The same object (kind, namespace, name) appears more than once in the inputs |
//! | I003 | A policy references a profile that is neither in the inputs nor built in |
//! | I004 | A grant references a policy that is not in the inputs |
//! | I005 | The compiled content has no RFC 8785 canonical form (ADR-0005) |
//! | I006 | A grant has expired |
//! | I007 | A policy's profile does not lint clean; the profile's own findings are reported against it |
//! | I008 | An ExposureCeiling names a policy that is not in the inputs (ADR-0007) |

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use sandboxpolicy_api::{ExposureCeiling, GrantPhase, KnobGrant, SandboxPolicy, SandboxProfile};
use sandboxpolicy_core::ceilings::Ceilings;
use sandboxpolicy_core::lint::{lint_knobs, Finding, Severity, RULES};
use sandboxpolicy_core::{builtin, evaluate, EvalInput};
use serde_json::{json, Value};

/// The API group whose objects are checked. Everything else in the inputs is ignored.
const GROUP_PREFIX: &str = "sandbox.firestoned.io/";

/// Bundle lifetime used for the compile. It affects only `notAfter`, which lint does not report;
/// it matches the controller's default (`NOTAIO_BUNDLE_LIFETIME_SECONDS`).
const BUNDLE_LIFETIME_SECONDS: i64 = 3600;

/// The input codes, as `(code, meaning)`, in code order (the table above, as data).
pub const INPUT_RULES: &[(&str, &str)] = &[
    (
        "I001",
        "An object in the sandbox.firestoned.io group is not a valid SandboxProfile, SandboxPolicy or KnobGrant",
    ),
    (
        "I002",
        "The same object (kind, namespace, name) appears more than once in the inputs",
    ),
    (
        "I003",
        "A policy references a profile that is neither in the inputs nor built in",
    ),
    ("I004", "A grant references a policy that is not in the inputs"),
    (
        "I005",
        "The compiled content has no RFC 8785 canonical form (ADR-0005)",
    ),
    ("I006", "A grant has expired"),
    (
        "I007",
        "A policy's profile does not lint clean; the profile's own findings are reported against it",
    ),
    (
        "I008",
        "An ExposureCeiling names a policy that is not in the inputs (ADR-0007)",
    ),
];

/// One input file.
pub struct Source {
    pub path: PathBuf,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
}

/// One finding, attributed to a file and an object.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// A lint rule (`L001` to `L013`) or an input code (`I001` to `I007`).
    pub code: String,
    pub level: Level,
    pub path: PathBuf,
    /// `<Kind> <namespace>/<name>`, or `<Kind> <name>` without a namespace.
    pub object: String,
    /// The field the finding is about, when there is one.
    pub field: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    /// Objects in the `sandbox.firestoned.io` group.
    pub checked: usize,
    /// Documents outside the group.
    pub ignored: usize,
}

impl Report {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.level == Level::Error)
    }

    fn push(&mut self, code: &str, path: &Path, object: &str, field: &str, message: String) {
        self.diagnostics.push(Diagnostic {
            code: code.to_string(),
            level: Level::Error,
            path: path.to_path_buf(),
            object: object.to_string(),
            field: field.to_string(),
            message,
        });
    }

    fn push_finding(&mut self, f: &Finding, path: &Path, object: &str) {
        self.diagnostics.push(Diagnostic {
            code: f.code.to_string(),
            level: match f.severity {
                Severity::Error => Level::Error,
                Severity::Warning => Level::Warning,
            },
            path: path.to_path_buf(),
            object: object.to_string(),
            field: f.path.clone(),
            message: f.message.clone(),
        });
    }
}

/// An input that cannot be read or is not YAML. `notaio lint` exits 2 on one.
#[derive(Debug)]
pub struct InputError {
    pub path: PathBuf,
    pub message: String,
}

/// Reads `paths`: files as given, directories searched recursively for `*.yaml` and `*.yml`, in
/// a stable order.
///
/// # Errors
/// `InputError` for a path that does not exist or a file that cannot be read.
pub fn load(paths: &[PathBuf]) -> Result<Vec<Source>, InputError> {
    let mut files = Vec::new();
    for p in paths {
        collect(p, &mut files)?;
    }
    files
        .into_iter()
        .map(|path| match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Source { path, text }),
            Err(e) => Err(InputError {
                path,
                message: e.to_string(),
            }),
        })
        .collect()
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), InputError> {
    let err = |e: std::io::Error| InputError {
        path: path.to_path_buf(),
        message: e.to_string(),
    };
    let meta = std::fs::metadata(path).map_err(err)?;
    if meta.is_file() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
        .map_err(err)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            collect(&entry, out)?;
            continue;
        }
        let yaml = entry
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e == "yaml" || e == "yml");
        if yaml {
            out.push(entry);
        }
    }
    Ok(())
}

/// Lints `sources` at `now` under the default ceilings.
///
/// # Errors
/// As [`lint_sources_with`].
pub fn lint_sources(sources: &[Source], now: DateTime<Utc>) -> Result<Report, InputError> {
    lint_sources_with(sources, now, &Ceilings::default())
}

/// Lints `sources` at `now` under `ceilings`.
///
/// # Errors
/// `InputError` for a source that is not YAML. Problems with the objects themselves are
/// diagnostics, never errors.
pub fn lint_sources_with(
    sources: &[Source],
    now: DateTime<Utc>,
    ceilings: &Ceilings,
) -> Result<Report, InputError> {
    let mut report = Report::default();
    let mut seen = BTreeSet::new();
    let mut profiles: BTreeMap<String, (PathBuf, SandboxProfile)> = BTreeMap::new();
    let mut policies: Vec<(PathBuf, SandboxPolicy)> = Vec::new();
    let mut grants: Vec<(PathBuf, KnobGrant)> = Vec::new();
    // By (namespace, name): a ceiling caps the policy with the same name (ADR-0007).
    let mut ceilings_by_policy: BTreeMap<(Option<String>, String), (PathBuf, ExposureCeiling)> =
        BTreeMap::new();

    for source in sources {
        let docs: Vec<Value> =
            serde_saphyr::from_multiple(&source.text).map_err(|e| InputError {
                path: source.path.clone(),
                message: e.to_string(),
            })?;
        for doc in docs {
            if doc.is_null() {
                continue;
            }
            let in_group = doc["apiVersion"]
                .as_str()
                .is_some_and(|a| a.starts_with(GROUP_PREFIX));
            if !in_group {
                report.ignored += 1;
                continue;
            }
            report.checked += 1;
            let kind = doc["kind"].as_str().unwrap_or_default().to_string();
            let name = doc["metadata"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let ns = doc["metadata"]["namespace"].as_str().map(str::to_string);
            let object = describe(&kind, ns.as_deref(), &name);
            if !seen.insert((kind.clone(), ns.clone(), name.clone())) {
                report.push("I002", &source.path, &object, "", "duplicate object".into());
                continue;
            }
            let invalid = |e: serde_json::Error| format!("not a valid {kind}: {e}");
            match kind.as_str() {
                "SandboxProfile" => match serde_json::from_value::<SandboxProfile>(doc) {
                    Ok(p) => {
                        profiles.insert(name, (source.path.clone(), p));
                    }
                    Err(e) => report.push("I001", &source.path, &object, "", invalid(e)),
                },
                "SandboxPolicy" => match serde_json::from_value::<SandboxPolicy>(doc) {
                    Ok(p) => policies.push((source.path.clone(), p)),
                    Err(e) => report.push("I001", &source.path, &object, "", invalid(e)),
                },
                "KnobGrant" => match serde_json::from_value::<KnobGrant>(doc) {
                    Ok(g) => grants.push((source.path.clone(), g)),
                    Err(e) => report.push("I001", &source.path, &object, "", invalid(e)),
                },
                "ExposureCeiling" => match serde_json::from_value::<ExposureCeiling>(doc) {
                    Ok(c) => {
                        ceilings_by_policy.insert((ns, name), (source.path.clone(), c));
                    }
                    Err(e) => report.push("I001", &source.path, &object, "", invalid(e)),
                },
                other => report.push(
                    "I001",
                    &source.path,
                    &object,
                    "kind",
                    format!("unknown kind {other:?}"),
                ),
            }
        }
    }

    for (name, (path, p)) in &profiles {
        let object = describe("SandboxProfile", None, name);
        for f in lint_knobs(p.spec.data_scope, &p.spec.knobs) {
            report.push_finding(&f, path, &object);
        }
    }

    for (path, g) in &grants {
        let owner = policies.iter().any(|(_, p)| {
            p.metadata.namespace == g.metadata.namespace
                && p.metadata.name.as_deref() == Some(g.spec.policy_ref.as_str())
        });
        if !owner {
            report.push(
                "I004",
                path,
                &grant_object(g),
                "policyRef",
                format!("no SandboxPolicy {:?} in the inputs", g.spec.policy_ref),
            );
        }
    }

    for ((ns, name), (path, _)) in &ceilings_by_policy {
        let owner = policies.iter().any(|(_, p)| {
            p.metadata.namespace == *ns && p.metadata.name.as_deref() == Some(name.as_str())
        });
        if !owner {
            report.push(
                "I008",
                path,
                &describe("ExposureCeiling", ns.as_deref(), name),
                "metadata.name",
                format!("no SandboxPolicy {name:?} in the inputs"),
            );
        }
    }

    for (path, policy) in &policies {
        let key = (
            policy.metadata.namespace.clone(),
            policy.metadata.name.clone().unwrap_or_default(),
        );
        let exposure_ceiling = ceilings_by_policy.get(&key).map(|(_, c)| c);
        let refs = PolicyRefs {
            profiles: &profiles,
            grants: &grants,
            exposure_ceiling,
        };
        lint_policy(&mut report, path, policy, &refs, now, ceilings);
    }
    Ok(report)
}

/// What a policy may reference among the inputs.
struct PolicyRefs<'a> {
    profiles: &'a BTreeMap<String, (PathBuf, SandboxProfile)>,
    grants: &'a [(PathBuf, KnobGrant)],
    exposure_ceiling: Option<&'a ExposureCeiling>,
}

fn lint_policy(
    report: &mut Report,
    path: &Path,
    policy: &SandboxPolicy,
    refs: &PolicyRefs<'_>,
    now: DateTime<Utc>,
    ceilings: &Ceilings,
) {
    let PolicyRefs {
        profiles,
        grants,
        exposure_ceiling,
    } = *refs;
    let name = policy.metadata.name.clone().unwrap_or_default();
    let object = describe("SandboxPolicy", policy.metadata.namespace.as_deref(), &name);
    let profile_name = &policy.spec.profile_ref;
    let profile = profiles
        .get(profile_name)
        .map(|(_, p)| p.clone())
        .or_else(|| builtin::profile(profile_name).map(|s| SandboxProfile::new(profile_name, s)));
    let Some(profile) = profile else {
        report.push(
            "I003",
            path,
            &object,
            "profileRef",
            format!("no SandboxProfile {profile_name:?} in the inputs or built in"),
        );
        return;
    };
    let mine: Vec<&(PathBuf, KnobGrant)> = grants
        .iter()
        .filter(|(_, g)| {
            g.metadata.namespace == policy.metadata.namespace && g.spec.policy_ref == name
        })
        .collect();
    let my_grants: Vec<KnobGrant> = mine.iter().map(|(_, g)| g.clone()).collect();
    let out = evaluate(&EvalInput {
        now,
        exposure_ceiling,
        policy,
        profile: Some(&profile),
        grants: &my_grants,
        prev_version: None,
        prev_digest: None,
        prev_conditions: &[],
        bundle_lifetime: Duration::seconds(BUNDLE_LIFETIME_SECONDS),
        ceilings,
    });

    let condition = |t: &str| out.status.conditions.iter().find(|c| c.type_ == t);
    if condition("ProfileResolved").is_some_and(|c| c.reason == "InvalidProfile") {
        // The profile's findings are reported against the profile itself.
        report.push(
            "I007",
            path,
            &object,
            "profileRef",
            format!("profile {profile_name:?} does not lint clean"),
        );
        return;
    }
    for f in &out.findings {
        report.push_finding(f, path, &object);
    }
    if let Some(c) = condition("Ready").filter(|c| c.reason == "NotCanonical") {
        report.push("I005", path, &object, "", c.message.clone());
    }
    for r in &out.grants {
        let Some((gpath, g)) = mine
            .iter()
            .find(|(_, g)| g.metadata.name.as_deref() == Some(r.name.as_str()))
        else {
            continue;
        };
        let gobject = grant_object(g);
        match r.phase {
            GrantPhase::Rejected => {
                for f in &r.findings {
                    report.push_finding(f, gpath, &gobject);
                }
            }
            GrantPhase::Expired => {
                report.push("I006", gpath, &gobject, "expiresAt", r.message.clone());
            }
            GrantPhase::Active | GrantPhase::Pending => {}
        }
    }
}

fn describe(kind: &str, ns: Option<&str>, name: &str) -> String {
    match ns {
        Some(ns) => format!("{kind} {ns}/{name}"),
        None => format!("{kind} {name}"),
    }
}

fn grant_object(g: &KnobGrant) -> String {
    describe(
        "KnobGrant",
        g.metadata.namespace.as_deref(),
        g.metadata.name.as_deref().unwrap_or_default(),
    )
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::Error => "error",
        Level::Warning => "warning",
    }
}

fn with_field(d: &Diagnostic) -> String {
    if d.field.is_empty() {
        String::new()
    } else {
        format!(" {}", d.field)
    }
}

/// One line per finding, then a summary line.
pub fn render_text(report: &Report) -> String {
    let mut out = String::new();
    // Writing to a String cannot fail.
    for d in &report.diagnostics {
        let _ = writeln!(
            out,
            "{}: {}: {} {}{}: {}",
            d.path.display(),
            d.object,
            level_name(d.level),
            d.code,
            with_field(d),
            d.message
        );
    }
    let errors = report
        .diagnostics
        .iter()
        .filter(|d| d.level == Level::Error)
        .count();
    let warnings = report.diagnostics.len() - errors;
    let _ = writeln!(
        out,
        "notaio lint: {} objects checked, {errors} errors, {warnings} warnings, {} other documents ignored",
        report.checked, report.ignored
    );
    out
}

/// SARIF 2.1.0, for GitHub code scanning. Every lint rule and input code is listed as a rule, so
/// a code that does not fire still has a description.
pub fn render_sarif(report: &Report) -> String {
    let rules: Vec<Value> = RULES
        .iter()
        .chain(INPUT_RULES)
        .map(|(id, text)| json!({ "id": id, "shortDescription": { "text": text } }))
        .collect();
    let results: Vec<Value> = report
        .diagnostics
        .iter()
        .map(|d| {
            json!({
                "ruleId": d.code,
                "level": level_name(d.level),
                "message": { "text": format!("{}{}: {}", d.object, with_field(d), d.message) },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": d.path.to_string_lossy() }
                    }
                }]
            })
        })
        .collect();
    let sarif = json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "notaio",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/firestoned/notaio",
                    "rules": rules
                }
            },
            "results": results
        }]
    });
    // Serialising a json! value cannot fail.
    serde_json::to_string_pretty(&sarif).unwrap_or_default()
}
