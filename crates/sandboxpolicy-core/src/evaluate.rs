//! `evaluate` is the whole decision: given a policy, its profile and its grants at an instant, say
//! what is valid, what the compiled bundle is, and what the status should be.
//!
//! It performs no I/O and reads no clock. The controller supplies `now` and the stored objects.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use sandboxpolicy_api::{
    Condition, ExposedAudience, GrantPhase, KnobGrant, MaxExposure, SandboxPolicy,
    SandboxPolicySpec, SandboxPolicyStatus, SandboxProfile, SandboxProfileSpec,
};
use sandboxpolicy_bundle::{BundleContent, BundleMeta, BundlePayload, PolicyRef, ProfileRef};
use sandboxpolicy_types::{Access, AudienceGrant, KnobId, Step};
use sha2::{Digest, Sha256};

use crate::ceilings::Ceilings;
use crate::lint::{self, has_errors, step_of, summarize, Finding};
use crate::registry::REGISTRY;

pub struct EvalInput<'a> {
    pub now: DateTime<Utc>,
    pub policy: &'a SandboxPolicy,
    /// `None` when the referenced profile does not exist.
    pub profile: Option<&'a SandboxProfile>,
    /// Grants visible to the policy. Grants for other policies are ignored.
    pub grants: &'a [KnobGrant],
    /// Highest version ever published for this policy (status, or the published object).
    pub prev_version: Option<u64>,
    pub prev_digest: Option<&'a str>,
    pub prev_conditions: &'a [Condition],
    /// How long a compiled bundle stays valid. This is the upper bound on staleness.
    pub bundle_lifetime: Duration,
    pub ceilings: &'a Ceilings,
}

#[derive(Debug, Clone)]
pub struct GrantResult {
    pub name: String,
    pub phase: GrantPhase,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct EvalOutput {
    pub status: SandboxPolicyStatus,
    pub grants: Vec<GrantResult>,
    /// Present only when the policy is valid. Never signed here.
    pub bundle: Option<BundlePayload>,
    pub findings: Vec<Finding>,
    pub requeue_after: Duration,
}

/// `sha256:<hex>` of a profile spec. Maps are ordered, so the bytes are stable.
pub fn profile_digest(spec: &SandboxProfileSpec) -> String {
    let bytes = serde_json::to_vec(spec).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    format!(
        "sha256:{}",
        d.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

/// Build a condition, keeping the previous transition time when the status did not change.
pub fn condition(
    prev: &[Condition],
    type_: &str,
    ok: bool,
    reason: &str,
    message: &str,
    now: DateTime<Utc>,
    generation: Option<i64>,
) -> Condition {
    let status = if ok { "True" } else { "False" };
    let last_transition_time = prev
        .iter()
        .find(|c| c.type_ == type_ && c.status == status)
        .map(|c| c.last_transition_time)
        .unwrap_or(now);
    Condition {
        type_: type_.to_string(),
        status: status.to_string(),
        reason: reason.to_string(),
        message: message.to_string(),
        last_transition_time,
        observed_generation: generation,
    }
}

fn full_knobs(effective: &BTreeMap<KnobId, Step>) -> BTreeMap<KnobId, Step> {
    REGISTRY
        .iter()
        .map(|d| (KnobId::new(d.id), step_of(effective, d.id)))
        .collect()
}

fn exposure(c: &BundleContent) -> MaxExposure {
    MaxExposure {
        audiences: c
            .audiences
            .iter()
            .map(|a| ExposedAudience {
                name: a.name.clone(),
                access: a.access,
                ttl_seconds: a.ttl_seconds.unwrap_or(0),
            })
            .collect(),
        egress_hosts: c
            .egress
            .iter()
            .map(|e| format!("{}:{}", e.host, e.port))
            .collect(),
        write_capable: c.audiences.iter().any(|a| a.access == Access::Write),
        tools: c.tools.iter().map(|t| t.name.clone()).collect(),
        lease_max_seconds: c.lease.max_duration_seconds,
    }
}

fn compile_content(
    policy: &SandboxPolicy,
    profile_name: &str,
    profile_digest: String,
    effective: &BTreeMap<KnobId, Step>,
    spec: &SandboxPolicySpec,
    ceilings: &Ceilings,
) -> BundleContent {
    let ttl_default = ceilings.token_ttl(step_of(effective, "K5.1"));
    let mut audiences: Vec<AudienceGrant> = spec
        .audiences
        .iter()
        .cloned()
        .map(|mut a| {
            a.ttl_seconds = Some(a.ttl_seconds.unwrap_or(ttl_default));
            a.scopes.sort();
            a
        })
        .collect();
    audiences.sort_by(|a, b| a.name.cmp(&b.name));
    let mut egress = spec.egress.clone();
    for e in &mut egress {
        e.methods.sort();
        e.paths.sort();
    }
    egress.sort_by(|a, b| (&a.host, a.port).cmp(&(&b.host, b.port)));
    let mut tools = spec.tools.clone();
    tools.sort_by(|a, b| a.name.cmp(&b.name));

    let k73 = step_of(effective, "K7.3");
    let k43 = step_of(effective, "K4.3");
    BundleContent {
        policy: PolicyRef {
            namespace: policy.metadata.namespace.clone().unwrap_or_default(),
            name: policy.metadata.name.clone().unwrap_or_default(),
            uid: policy.metadata.uid.clone().unwrap_or_default(),
        },
        profile: ProfileRef {
            name: profile_name.to_string(),
            digest: profile_digest,
        },
        knobs: full_knobs(effective),
        audiences,
        egress,
        tools,
        lease: spec
            .lease
            .clone()
            .unwrap_or_else(|| ceilings.lease_limit(k73)),
        budgets: spec
            .budgets
            .clone()
            .unwrap_or_else(|| ceilings.default_budgets(k73, k43)),
    }
}

pub fn evaluate(input: &EvalInput<'_>) -> EvalOutput {
    let now = input.now;
    let generation = input.policy.metadata.generation;
    let policy_name = input.policy.metadata.name.clone().unwrap_or_default();
    let cond = |t: &str, ok: bool, reason: &str, msg: &str| {
        condition(input.prev_conditions, t, ok, reason, msg, now, generation)
    };

    let mut status = SandboxPolicyStatus {
        observed_generation: generation,
        bundle_version: input.prev_version,
        bundle_digest: input.prev_digest.map(str::to_string),
        ..Default::default()
    };
    let retry = Duration::seconds(60);

    let mut my_grants: Vec<&KnobGrant> = input
        .grants
        .iter()
        .filter(|g| g.spec.policy_ref == policy_name)
        .collect();
    my_grants.sort_by_key(|g| g.metadata.name.clone().unwrap_or_default());
    let grant_name = |g: &KnobGrant| g.metadata.name.clone().unwrap_or_default();

    // 1. The profile must exist and be valid.
    let Some(profile) = input.profile else {
        status.conditions = vec![
            cond(
                "ProfileResolved",
                false,
                "ProfileNotFound",
                "the referenced SandboxProfile does not exist",
            ),
            cond(
                "Ready",
                false,
                "ProfileNotFound",
                "no bundle can be compiled",
            ),
        ];
        let grants = my_grants
            .iter()
            .map(|g| GrantResult {
                name: grant_name(g),
                phase: GrantPhase::Pending,
                message: "waiting for the profile".to_string(),
            })
            .collect();
        return EvalOutput {
            status,
            grants,
            bundle: None,
            findings: vec![],
            requeue_after: retry,
        };
    };
    let scope = profile.spec.data_scope;
    let profile_findings = lint::lint_knobs(scope, &profile.spec.knobs);
    if has_errors(&profile_findings) {
        let msg = summarize(&profile_findings);
        status.conditions = vec![
            cond("ProfileResolved", false, "InvalidProfile", &msg),
            cond(
                "Ready",
                false,
                "InvalidProfile",
                "no bundle can be compiled",
            ),
        ];
        let grants = my_grants
            .iter()
            .map(|g| GrantResult {
                name: grant_name(g),
                phase: GrantPhase::Pending,
                message: "waiting for a valid profile".to_string(),
            })
            .collect();
        return EvalOutput {
            status,
            grants,
            bundle: None,
            findings: profile_findings,
            requeue_after: retry,
        };
    }
    let p_digest = profile_digest(&profile.spec);
    status.profile_digest = Some(p_digest.clone());

    // 2. Grants, in name order so the result is deterministic.
    let mut effective = profile.spec.knobs.clone();
    let mut granted: BTreeMap<KnobId, String> = BTreeMap::new();
    let mut grants = Vec::new();
    let mut active = Vec::new();
    let mut earliest: Option<DateTime<Utc>> = None;
    for g in my_grants {
        let name = grant_name(g);
        if g.spec.expires_at <= now {
            grants.push(GrantResult {
                name,
                phase: GrantPhase::Expired,
                message: format!("expired at {}", g.spec.expires_at.to_rfc3339()),
            });
            continue;
        }
        let mut f = lint::lint_grant(&profile.spec.knobs, &g.spec, now);
        if let Some(other) = granted.get(&g.spec.knob) {
            f.push(Finding {
                code: "L011",
                severity: lint::Severity::Error,
                path: "knob".into(),
                message: format!("already relaxed by grant {other}"),
            });
        }
        let mut trial = effective.clone();
        if !has_errors(&f) {
            trial.insert(g.spec.knob.clone(), g.spec.step);
            f.extend(lint::lint_knobs(scope, &trial));
        }
        if has_errors(&f) {
            grants.push(GrantResult {
                name,
                phase: GrantPhase::Rejected,
                message: summarize(&f),
            });
            continue;
        }
        effective = trial;
        granted.insert(g.spec.knob.clone(), name.clone());
        earliest = Some(earliest.map_or(g.spec.expires_at, |e| e.min(g.spec.expires_at)));
        active.push(name.clone());
        grants.push(GrantResult {
            name,
            phase: GrantPhase::Active,
            message: "applied".into(),
        });
    }
    status.active_grants = active;
    status.next_grant_expiry = earliest;

    // 3. The policy itself, against the effective knobs.
    let findings = lint::lint_policy(&effective, &input.policy.spec, input.ceilings);
    let mut requeue = input.bundle_lifetime / 2;
    if let Some(e) = earliest {
        requeue = requeue.min(e - now + Duration::seconds(1));
    }
    let requeue = requeue.max(Duration::seconds(10));
    if has_errors(&findings) {
        let msg = summarize(&findings);
        status.conditions = vec![
            cond("ProfileResolved", true, "Resolved", "profile is valid"),
            cond("Valid", false, "InvalidPolicy", &msg),
            cond(
                "Ready",
                false,
                "InvalidPolicy",
                "no bundle compiled for this generation",
            ),
        ];
        return EvalOutput {
            status,
            grants,
            bundle: None,
            findings,
            requeue_after: requeue,
        };
    }

    // 4. Compile. The version moves only when the content moves.
    let content = compile_content(
        input.policy,
        profile
            .metadata
            .name
            .as_deref()
            .unwrap_or(&input.policy.spec.profile_ref),
        p_digest,
        &effective,
        &input.policy.spec,
        input.ceilings,
    );
    let digest = content.digest();
    let version = match (input.prev_digest, input.prev_version) {
        (Some(d), Some(v)) if d == digest => v,
        (_, v) => v.unwrap_or(0) + 1,
    };
    let mut not_after = now + input.bundle_lifetime;
    if let Some(e) = earliest {
        not_after = not_after.min(e);
    }
    status.bundle_version = Some(version);
    status.bundle_digest = Some(digest);
    status.bundle_not_after = Some(not_after);
    status.max_exposure = Some(exposure(&content));
    status.conditions = vec![
        cond("ProfileResolved", true, "Resolved", "profile is valid"),
        cond("Valid", true, "Valid", "policy passes every lint rule"),
        cond(
            "Ready",
            true,
            "Compiled",
            "bundle compiled, awaiting signature and publication",
        ),
    ];
    let bundle = BundlePayload::new(
        BundleMeta {
            version,
            issued_at: now,
            not_after,
        },
        content,
    );
    EvalOutput {
        status,
        grants,
        bundle: Some(bundle),
        findings,
        requeue_after: requeue,
    }
}
