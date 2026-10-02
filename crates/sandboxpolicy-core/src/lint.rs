//! Lint rules. Every rule has a stable code so findings can be documented, tested and alerted on.
//!
//! | Code | Rule |
//! | ---- | ---- |
//! | L001 | Unknown knob id |
//! | L002 | Step above the loosest step the knob has |
//! | L003 | Toxic combination: sensitive data, untrusted content and an outbound channel together |
//! | L004 | Lab-only knob relaxed on a profile that is not `LabOnly` |
//! | L005 | Audience set does not fit K5.3, or an audience is malformed or a wildcard |
//! | L006 | Write access without K5.2, or token TTL above the K5.1 ceiling |
//! | L007 | Egress entry does not fit K4.1 |
//! | L008 | Tool entry does not fit K1.3, or is not pinned by digest |
//! | L009 | Lease or budget above the K7.3 or K4.3 ceiling |
//! | L010 | Policy binds no subjects |
//! | L011 | Grant is not exactly one step above the profile, or duplicates another grant |
//! | L012 | Grant outlives the maximum lifetime for its target step |
//! | L013 | Grant lacks an approval from a required role |

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Duration, Utc};
use sandboxpolicy_api::{KnobGrantSpec, SandboxPolicySpec};
use sandboxpolicy_types::{Access, DataScope, KnobId, Role, Step};

use crate::ceilings::Ceilings;
use crate::registry::{self, LAB_ONLY_KNOBS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub code: &'static str,
    pub severity: Severity,
    pub path: String,
    pub message: String,
}

impl Finding {
    fn err(code: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Finding {
            code,
            severity: Severity::Error,
            path: path.into(),
            message: message.into(),
        }
    }
    fn warn(code: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Finding {
            severity: Severity::Warning,
            ..Finding::err(code, path, message)
        }
    }
}

pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}

pub fn summarize(findings: &[Finding]) -> String {
    findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| format!("{} {}: {}", f.code, f.path, f.message))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Step of a knob; absent means the locked default.
pub fn step_of(knobs: &BTreeMap<KnobId, Step>, id: &str) -> Step {
    knobs.get(&KnobId::new(id)).copied().unwrap_or(Step::R0)
}

/// The three legs of the exfiltration triangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Triangle {
    pub sensitive_data: bool,
    pub untrusted_content: bool,
    pub outbound_channel: bool,
}

impl Triangle {
    pub fn complete(&self) -> bool {
        self.sensitive_data && self.untrusted_content && self.outbound_channel
    }
}

/// Section 9, "No toxic combinations": sensitive data is K2.2 or K5.3 above R0, untrusted content
/// is K1.1 at R2 or above, an outbound channel is K4.1 at R2 or above or any K5.2 write scope. A
/// `LabOnly` profile holds no sensitive data by construction, so its first leg is never active.
pub fn triangle(scope: DataScope, knobs: &BTreeMap<KnobId, Step>) -> Triangle {
    Triangle {
        sensitive_data: scope == DataScope::Internal
            && (step_of(knobs, "K2.2") > Step::R0 || step_of(knobs, "K5.3") > Step::R0),
        untrusted_content: step_of(knobs, "K1.1") >= Step::R2,
        outbound_channel: step_of(knobs, "K4.1") >= Step::R2 || step_of(knobs, "K5.2") >= Step::R1,
    }
}

/// Knob level rules: L001, L002, L003, L004. Applies to a profile and to an effective knob set.
pub fn lint_knobs(scope: DataScope, knobs: &BTreeMap<KnobId, Step>) -> Vec<Finding> {
    let mut out = Vec::new();
    for (id, step) in knobs {
        let path = format!("knobs.{id}");
        match registry::knob(id.as_str()) {
            None => out.push(Finding::err("L001", path, "unknown knob")),
            Some(def) => {
                if *step > def.max {
                    out.push(Finding::err(
                        "L002",
                        &path,
                        format!(
                            "{step} is above the loosest step for {} ({})",
                            def.id, def.max
                        ),
                    ));
                }
                if scope != DataScope::LabOnly
                    && LAB_ONLY_KNOBS.contains(&def.id)
                    && *step > Step::R0
                {
                    out.push(Finding::err(
                        "L004",
                        path,
                        format!("{} may only be relaxed on a LabOnly profile", def.id),
                    ));
                }
            }
        }
    }
    let t = triangle(scope, knobs);
    if t.complete() {
        out.push(Finding::err(
            "L003",
            "knobs",
            "sensitive data, untrusted content and an outbound channel are all reachable; \
             keep one of the three at R0",
        ));
    }
    out
}

fn is_ip_literal(host: &str) -> bool {
    host.parse::<std::net::IpAddr>().is_ok()
}

fn valid_digest(d: &str) -> bool {
    d.strip_prefix("sha256:")
        .is_some_and(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Policy level rules against the effective knobs: L005 to L010.
pub fn lint_policy(
    effective: &BTreeMap<KnobId, Step>,
    spec: &SandboxPolicySpec,
    ceilings: &Ceilings,
) -> Vec<Finding> {
    let mut out = Vec::new();

    if spec.subjects.groups.is_empty() {
        out.push(Finding::warn(
            "L010",
            "subjects.groups",
            "no groups bound: nobody can claim a sandbox under this policy",
        ));
    }

    // Audiences (K5.3, K5.2, K5.1).
    let k53 = step_of(effective, "K5.3");
    let limit = match k53 {
        Step::R0 => Some(0),
        Step::R1 => Some(1),
        _ => None,
    };
    if let Some(n) = limit {
        if spec.audiences.len() > n {
            out.push(Finding::err(
                "L005",
                "audiences",
                format!(
                    "K5.3 is {k53}: at most {n} audience(s) allowed, found {}",
                    spec.audiences.len()
                ),
            ));
        }
    }
    let ttl_ceiling = ceilings.token_ttl(step_of(effective, "K5.1"));
    let writes_allowed = step_of(effective, "K5.2") >= Step::R1;
    let mut seen = BTreeSet::new();
    for (i, a) in spec.audiences.iter().enumerate() {
        let path = format!("audiences[{i}]");
        if a.name.trim().is_empty() || a.name.contains('*') {
            out.push(Finding::err(
                "L005",
                &path,
                "audience must be named and never a wildcard",
            ));
        }
        if !seen.insert(a.name.clone()) {
            out.push(Finding::err(
                "L005",
                &path,
                format!("duplicate audience {:?}", a.name),
            ));
        }
        if a.access == Access::Write && !writes_allowed {
            out.push(Finding::err(
                "L006",
                &path,
                "write access needs K5.2 at R1 or above",
            ));
        }
        if let Some(ttl) = a.ttl_seconds {
            if ttl == 0 || ttl > ttl_ceiling {
                out.push(Finding::err(
                    "L006",
                    &path,
                    format!(
                        "ttlSeconds {ttl} must be between 1 and the K5.1 ceiling {ttl_ceiling}"
                    ),
                ));
            }
        }
    }

    // Egress (K4.1).
    let k41 = step_of(effective, "K4.1");
    if k41 == Step::R0 && !spec.egress.is_empty() {
        out.push(Finding::err(
            "L007",
            "egress",
            "K4.1 is R0: no egress entries allowed, all calls are brokered",
        ));
    }
    for (i, e) in spec.egress.iter().enumerate() {
        let path = format!("egress[{i}]");
        if e.host.trim().is_empty() || e.port == 0 {
            out.push(Finding::err("L007", &path, "host and port are required"));
        }
        if is_ip_literal(&e.host) {
            out.push(Finding::err(
                "L007",
                &path,
                "direct-IP egress is never allowed, use a name",
            ));
        }
        if e.host.contains('*') && k41 < Step::R3 {
            out.push(Finding::err(
                "L007",
                &path,
                "wildcard hosts need K4.1 at R3",
            ));
        }
        if (!e.methods.is_empty() || !e.paths.is_empty()) && k41 < Step::R2 {
            out.push(Finding::err(
                "L007",
                &path,
                "method and path rules need K4.1 at R2 or above",
            ));
        }
    }

    // Tools (K1.3).
    let k13 = step_of(effective, "K1.3");
    if k13 == Step::R0 && !spec.tools.is_empty() {
        out.push(Finding::err(
            "L008",
            "tools",
            "K1.3 is R0: only built-in tools, no tool entries allowed",
        ));
    }
    let mut tool_names = BTreeSet::new();
    for (i, t) in spec.tools.iter().enumerate() {
        let path = format!("tools[{i}]");
        if !valid_digest(&t.manifest_digest) {
            out.push(Finding::err(
                "L008",
                &path,
                "manifestDigest must be sha256:<64 hex>",
            ));
        }
        if !tool_names.insert(t.name.clone()) {
            out.push(Finding::err(
                "L008",
                &path,
                format!("duplicate tool {:?}", t.name),
            ));
        }
    }

    // Lease and budgets (K7.3, K4.3).
    let lease_cap = ceilings.lease_limit(step_of(effective, "K7.3"));
    if let Some(l) = &spec.lease {
        if l.max_duration_seconds == 0 || l.max_duration_seconds > lease_cap.max_duration_seconds {
            out.push(Finding::err(
                "L009",
                "lease.maxDurationSeconds",
                format!(
                    "must be between 1 and the K7.3 ceiling {}",
                    lease_cap.max_duration_seconds
                ),
            ));
        }
        if l.max_concurrent == 0 || l.max_concurrent > lease_cap.max_concurrent {
            out.push(Finding::err(
                "L009",
                "lease.maxConcurrent",
                format!(
                    "must be between 1 and the K7.3 ceiling {}",
                    lease_cap.max_concurrent
                ),
            ));
        }
    }
    if let Some(b) = &spec.budgets {
        let cap = ceilings.egress_limit(step_of(effective, "K4.3"));
        if b.max_egress_bytes > cap {
            out.push(Finding::err(
                "L009",
                "budgets.maxEgressBytes",
                format!("above the K4.3 ceiling {cap}"),
            ));
        }
        if b.max_tool_calls == 0 || b.max_wall_clock_seconds == 0 {
            out.push(Finding::err(
                "L009",
                "budgets",
                "tool call and wall clock budgets must be above zero",
            ));
        }
    }
    out
}

/// Grant rules against the profile's own knob steps: L001, L002, L011, L012, L013. The caller
/// handles expiry and the combined knob lint (L003, L004) when it applies the grant.
pub fn lint_grant(
    profile_knobs: &BTreeMap<KnobId, Step>,
    spec: &KnobGrantSpec,
    now: DateTime<Utc>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    let Some(def) = registry::knob(spec.knob.as_str()) else {
        out.push(Finding::err("L001", "knob", "unknown knob"));
        return out;
    };
    if spec.step > def.max {
        out.push(Finding::err(
            "L002",
            "step",
            format!(
                "{} is above the loosest step for {} ({})",
                spec.step, def.id, def.max
            ),
        ));
    }
    let base = step_of(profile_knobs, def.id);
    if base.next() != Some(spec.step) {
        out.push(Finding::err(
            "L011",
            "step",
            format!("profile is at {base}: a grant must be exactly one step looser, never a jump"),
        ));
    }
    if spec.step > Step::R0 {
        let max_days = registry::max_grant_lifetime_days(spec.step);
        if spec.expires_at > now + Duration::days(max_days) {
            out.push(Finding::err(
                "L012",
                "expiresAt",
                format!("{} grants may last at most {max_days} days", spec.step),
            ));
        }
    }
    let have: BTreeSet<Role> = spec
        .approvals
        .iter()
        .filter(|a| !a.reference.trim().is_empty())
        .map(|a| a.role)
        .collect();
    for role in def.approvers {
        if !have.contains(role) {
            out.push(Finding::err(
                "L013",
                "approvals",
                format!(
                    "{} needs an approval from {role:?} with a reference",
                    def.id
                ),
            ));
        }
    }
    if spec.owner.trim().is_empty() || spec.reason.trim().is_empty() {
        out.push(Finding::err(
            "L013",
            "owner",
            "owner and reason are required",
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;

    fn knobs(pairs: &[(&str, Step)]) -> BTreeMap<KnobId, Step> {
        pairs.iter().map(|(k, s)| (KnobId::new(k), *s)).collect()
    }

    fn codes(f: &[Finding]) -> Vec<&'static str> {
        f.iter().map(|f| f.code).collect()
    }

    #[test]
    fn builtin_profiles_are_clean() {
        for (name, p) in builtin::all() {
            let f = lint_knobs(p.data_scope, &p.knobs);
            assert!(!has_errors(&f), "{name}: {f:?}");
        }
    }

    #[test]
    fn triangle_fires_for_internal_profile() {
        let k = knobs(&[("K2.2", Step::R1), ("K1.1", Step::R2), ("K4.1", Step::R2)]);
        assert!(codes(&lint_knobs(DataScope::Internal, &k)).contains(&"L003"));
    }

    #[test]
    fn any_two_legs_are_allowed() {
        let sens_out = knobs(&[("K2.2", Step::R1), ("K4.1", Step::R2)]);
        let sens_untrusted = knobs(&[("K2.2", Step::R1), ("K1.1", Step::R2)]);
        let untrusted_out = knobs(&[("K1.1", Step::R2), ("K4.1", Step::R2)]);
        for k in [sens_out, sens_untrusted, untrusted_out] {
            assert!(!codes(&lint_knobs(DataScope::Internal, &k)).contains(&"L003"));
        }
    }

    #[test]
    fn write_scope_counts_as_an_outbound_channel() {
        let k = knobs(&[("K5.3", Step::R1), ("K1.1", Step::R2), ("K5.2", Step::R1)]);
        assert!(codes(&lint_knobs(DataScope::Internal, &k)).contains(&"L003"));
    }

    #[test]
    fn lab_only_scope_clears_the_sensitive_leg() {
        let k = knobs(&[("K2.2", Step::R2), ("K1.1", Step::R3), ("K4.1", Step::R3)]);
        assert!(!codes(&lint_knobs(DataScope::LabOnly, &k)).contains(&"L003"));
        assert!(codes(&lint_knobs(DataScope::Internal, &k)).contains(&"L003"));
    }

    #[test]
    fn lab_only_knobs_are_refused_elsewhere() {
        let k = knobs(&[("K6.3", Step::R1)]);
        assert!(codes(&lint_knobs(DataScope::Internal, &k)).contains(&"L004"));
        assert!(!has_errors(&lint_knobs(DataScope::LabOnly, &k)));
    }

    #[test]
    fn unknown_and_over_max_knobs() {
        let k = knobs(&[("K9.9", Step::R1), ("K4.4", Step::R3)]);
        let c = codes(&lint_knobs(DataScope::Internal, &k));
        assert!(c.contains(&"L001") && c.contains(&"L002"));
    }
}
