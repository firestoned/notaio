// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! Audience definitions (ADR-0020): the lint rules on a `SandboxAudience` (L017 on its endpoints
//! and rules, L022 to L026, L029), on the policy grants that name one (L027, L028), and the
//! definition's half of the ceiling check (L014). Pure, like the rest of this crate.
//!
//! None of this applies until the platform audience namespace is configured: the caller passes no
//! [`AudienceCatalog`], and policies behave as before ADR-0020 (decision 2, opt-in).

use std::collections::BTreeSet;

use jiff::{SignedDuration, Timestamp};
use sandboxpolicy_api::{Condition, ExposureCeilingSpec, SandboxAudience, SandboxAudienceStatus};
use sandboxpolicy_bundle::{AudienceBundleContent, AudienceBundlePayload, AudienceRef, BundleMeta};
use sandboxpolicy_types::{AudienceGrant, Backend, CredentialSource, EndpointRule, Placement};

use crate::evaluate::condition;
use crate::lint::{endpoint_problem, has_errors, rule_problem, summarize, Finding};

/// The shortest requeue, whatever the bundle lifetime: the same floor as a policy's.
const MIN_REQUEUE_SECONDS: i64 = 10;

/// Header names an audience may never set (ADR-0020, decision 7): they frame the request or
/// belong to the proxy, so setting them could smuggle a request or override the framing.
pub const RESERVED_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "te",
    "upgrade",
    "trailer",
    "keep-alive",
    "proxy-authorization",
    "proxy-connection",
];

/// Every `SandboxAudience` notaio sees, with the platform audience namespace that decides which
/// are platform audiences (ADR-0020, decision 2).
#[derive(Debug, Clone, Copy)]
pub struct AudienceCatalog<'a> {
    platform_namespace: &'a str,
    audiences: &'a [SandboxAudience],
}

impl<'a> AudienceCatalog<'a> {
    pub fn new(platform_namespace: &'a str, audiences: &'a [SandboxAudience]) -> Self {
        AudienceCatalog {
            platform_namespace,
            audiences,
        }
    }

    pub fn platform_namespace(&self) -> &'a str {
        self.platform_namespace
    }

    /// Is `a` a platform audience? The namespace decides, never a field.
    pub fn is_platform(&self, a: &SandboxAudience) -> bool {
        namespace(a) == self.platform_namespace
    }

    /// The audiences a sandbox in `ns` can see: the platform ones and the team ones in `ns`.
    pub fn visible_from(&self, ns: &'a str) -> impl Iterator<Item = &'a SandboxAudience> + 'a {
        let platform = self.platform_namespace;
        self.audiences.iter().filter(move |a| {
            let n = namespace(a);
            n == platform || n == ns
        })
    }

    /// The definition a policy in `policy_ns` gets for `name`: the platform audience if there is
    /// one (a name means the same everywhere, decision 6), else the team audience in `policy_ns`.
    ///
    /// # Errors
    /// The `L027` message: no definition is visible, or the only one is another namespace's team
    /// audience (decision 3).
    pub fn resolve(&self, policy_ns: &str, name: &str) -> Result<&'a SandboxAudience, String> {
        let named = || {
            self.audiences
                .iter()
                .filter(|a| a.metadata.name.as_deref() == Some(name))
        };
        if let Some(p) = named().find(|a| self.is_platform(a)) {
            return Ok(p);
        }
        if let Some(t) = named().find(|a| namespace(a) == policy_ns) {
            return Ok(t);
        }
        let elsewhere: Vec<&str> = named().map(namespace).collect();
        if elsewhere.is_empty() {
            return Err(format!("no SandboxAudience {name:?} is defined"));
        }
        Err(format!(
            "{name:?} is a team audience of {}, usable only there",
            elsewhere.join(", ")
        ))
    }
}

fn namespace(a: &SandboxAudience) -> &str {
    a.metadata.namespace.as_deref().unwrap_or_default()
}

fn name(a: &SandboxAudience) -> &str {
    a.metadata.name.as_deref().unwrap_or_default()
}

/// The content of `a`'s signed bundle (ADR-0020, decision 9). Lists are sorted so the digest does
/// not depend on the order they were written in.
pub fn audience_content(a: &SandboxAudience) -> AudienceBundleContent {
    let mut definition = a.spec.definition.clone();
    definition.endpoints.sort();
    definition.endpoints.dedup();
    definition.rules.sort();
    definition.rules.dedup();
    definition.credential.extra_headers.sort();
    AudienceBundleContent {
        audience: AudienceRef {
            namespace: namespace(a).to_string(),
            name: name(a).to_string(),
            uid: a.metadata.uid.clone().unwrap_or_default(),
        },
        definition,
    }
}

/// Where an audience stands in a collision (ADR-0020, decision 6), read from its status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    /// No published bundle: it can only lose a collision, never refuse another.
    New,
    /// Published, and the content has moved since.
    Changed,
    /// Published, and the published bundle is the current content.
    Stable,
}

fn standing(a: &SandboxAudience) -> Standing {
    let Some(published) = a.status.as_ref().and_then(|s| s.bundle_digest.as_deref()) else {
        return Standing::New;
    };
    if audience_content(a).digest().is_ok_and(|d| d == published) {
        Standing::Stable
    } else {
        Standing::Changed
    }
}

/// Of a collision between `me` and `other`, is `me` refused (decision 6)? The newcomer is; a
/// changed audience gives way to a stable one; two changed audiences of different tenants are
/// decided by the namespace, the platform kept. At least one side is always refused.
///
/// An audience that was never published refuses nothing: otherwise a refused newcomer left in
/// place would turn the next edit of a working audience into a finding, and one team namespace
/// could withdraw a platform audience from every namespace.
fn refused_in_collision(
    me: &SandboxAudience,
    other: &SandboxAudience,
    catalog: &AudienceCatalog<'_>,
) -> bool {
    match (standing(me), standing(other)) {
        (Standing::New, _) => true,
        (Standing::Stable, Standing::Stable) => true,
        (Standing::Stable, _) => false,
        (Standing::Changed, Standing::New) => false,
        (Standing::Changed, Standing::Stable) => true,
        (Standing::Changed, Standing::Changed) => {
            !(catalog.is_platform(me) && !catalog.is_platform(other))
        }
    }
}

/// A lowercase RFC 9110 token.
fn is_lowercase_token(h: &str) -> bool {
    const TCHAR_SYMBOLS: &[u8] = b"!#$%&'*+-.^_`|~";
    !h.is_empty()
        && h.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || TCHAR_SYMBOLS.contains(&b))
}

fn header_problem(h: &str) -> Option<&'static str> {
    if !is_lowercase_token(h) {
        return Some("a header name is a lowercase token");
    }
    if RESERVED_HEADERS.contains(&h) {
        return Some("a reserved header name: it frames the request or belongs to the proxy");
    }
    None
}

/// Why `v` is not a header value notaio signs, if it is not: printable ASCII, with spaces and
/// tabs inside it only. A carriage return or line feed would end the header and start another.
fn header_value_problem(v: &str) -> Option<&'static str> {
    const SPACE: u8 = b' ';
    const TAB: u8 = b'\t';
    if v.is_empty() {
        return Some("a header value is not empty");
    }
    if !v
        .bytes()
        .all(|b| b.is_ascii_graphic() || b == SPACE || b == TAB)
    {
        return Some("a header value is printable ASCII: no line break or other control character");
    }
    if v.starts_with([' ', '\t']) || v.ends_with([' ', '\t']) {
        return Some("a header value has no leading or trailing whitespace");
    }
    None
}

/// The rules on one `SandboxAudience`, given every audience in `catalog`: L017 on its endpoints
/// and rules, L022 to L026, and the L029 warning.
pub fn lint_audience(a: &SandboxAudience, catalog: &AudienceCatalog<'_>) -> Vec<Finding> {
    let mut out = Vec::new();
    let d = &a.spec.definition;
    let team = !catalog.is_platform(a);

    for e in &d.endpoints {
        if let Some(why) = endpoint_problem(e) {
            out.push(Finding::err("L017", format!("spec.endpoints[{e}]"), why));
        }
    }
    for (j, r) in d.rules.iter().enumerate() {
        if let Some(why) = rule_problem(r) {
            out.push(Finding::err("L017", format!("spec.rules[{j}]"), why));
        }
    }

    // L022: minting, refreshing or exchanging uses the consumer's own material.
    if team && !matches!(d.credential.backend, Backend::Secret | Backend::None) {
        out.push(Finding::err(
            "L022",
            "spec.credential.backend",
            "a team audience's backend is secret or none; the others act with the platform's keys",
        ));
    }
    // L023: the consumer reads OpenBao with its own identity, so a team must not choose the path.
    if team && matches!(d.credential.source, Some(CredentialSource::Openbao(_))) {
        out.push(Finding::err(
            "L023",
            "spec.credential.source.openbao",
            "a team audience's credential is a Secret in its own namespace, never OpenBao",
        ));
    }

    // L024: one name, one meaning, between the platform and every team.
    for other in catalog
        .audiences
        .iter()
        .filter(|o| name(o) == name(a) && catalog.is_platform(o) != catalog.is_platform(a))
    {
        if refused_in_collision(a, other, catalog) {
            out.push(Finding::err(
                "L024",
                "metadata.name",
                format!(
                    "{:?} is also an audience in {}: a name means one audience in every namespace",
                    name(a),
                    namespace(other)
                ),
            ));
        }
    }

    // L025: within what one namespace can see, an endpoint belongs to one audience.
    let mine: BTreeSet<&str> = d.endpoints.iter().map(String::as_str).collect();
    let peers: Vec<&SandboxAudience> = if team {
        catalog.visible_from(namespace(a)).collect()
    } else {
        // A platform audience is visible from every namespace, so it meets every audience.
        catalog.audiences.iter().collect()
    };
    for other in peers {
        if std::ptr::eq(other, a) || (namespace(other) == namespace(a) && name(other) == name(a)) {
            continue;
        }
        let shared: Vec<&str> = other
            .spec
            .definition
            .endpoints
            .iter()
            .map(String::as_str)
            .filter(|e| mine.contains(e))
            .collect();
        if shared.is_empty() || !refused_in_collision(a, other, catalog) {
            continue;
        }
        for e in shared {
            out.push(Finding::err(
                "L025",
                format!("spec.endpoints[{e}]"),
                format!(
                    "also an endpoint of audience {}/{}",
                    namespace(other),
                    name(other)
                ),
            ));
        }
    }

    // L026
    if let Some(h) = &d.credential.header {
        if let Some(why) = header_problem(h) {
            out.push(Finding::err("L026", "spec.credential.header", why));
        }
    }
    for (j, h) in d.credential.extra_headers.iter().enumerate() {
        if let Some(why) = header_problem(&h.name) {
            out.push(Finding::err(
                "L026",
                format!("spec.credential.extraHeaders[{j}].name"),
                why,
            ));
        }
        if let Some(why) = header_value_problem(&h.value) {
            out.push(Finding::err(
                "L026",
                format!("spec.credential.extraHeaders[{j}].value"),
                why,
            ));
        }
    }

    // L029: allowed, but the credential then sits in the pool VM's memory, outside the jail.
    if team && d.credential.placement == Placement::Proxy {
        out.push(Finding::warn(
            "L029",
            "spec.credential.placement",
            "a team credential placed in the proxy sits in the pool VM's memory; prefer gateway",
        ));
    }
    out
}

/// Path segments as the proxy matches them: split on `/`, empty segments dropped.
fn segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// Does the glob `outer` match every path the glob `inner` matches, segment-wise (`*` one segment,
/// `**` zero or more)? Sound, not complete: a `*` or a literal never covers a `**`, and a literal
/// never covers a `*`. The same rule as mediatore's `glob_covers`, so both sides agree.
fn glob_covers(outer: &[&str], inner: &[&str]) -> bool {
    match outer.split_first() {
        None => inner.is_empty(),
        Some((&"**", rest)) => (0..=inner.len()).any(|skip| glob_covers(rest, &inner[skip..])),
        Some((&"*", rest)) => inner
            .split_first()
            .is_some_and(|(head, tail)| *head != "**" && glob_covers(rest, tail)),
        Some((lit, rest)) => inner
            .split_first()
            .is_some_and(|(head, tail)| head == lit && glob_covers(rest, tail)),
    }
}

/// Does the definition's rule `outer` allow everything the grant's rule `inner` allows?
pub fn rule_covers(outer: &EndpointRule, inner: &EndpointRule) -> bool {
    let method = outer.method == "*" || outer.method.eq_ignore_ascii_case(&inner.method);
    method && glob_covers(&segments(&outer.path), &segments(&inner.path))
}

/// The policy's audience grants against their definitions: L027 and L028. `grants` are the
/// compiled grants (a token lifetime filled in from K5.1 when the policy left it out).
pub fn lint_grants(
    grants: &[AudienceGrant],
    policy_ns: &str,
    catalog: &AudienceCatalog<'_>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for g in grants {
        let path = format!("audiences[{}]", g.name);
        let def = match catalog.resolve(policy_ns, &g.name) {
            Ok(a) => &a.spec.definition,
            Err(why) => {
                out.push(Finding::err("L027", path, why));
                continue;
            }
        };
        // Empty grant endpoints reach nothing, so they are never wider.
        for e in g.endpoints.iter().filter(|e| !def.endpoints.contains(e)) {
            out.push(Finding::err(
                "L028",
                format!("{path}.endpoints[{e}]"),
                "endpoint not in the audience's definition",
            ));
        }
        // Empty grant rules are the definition's; an empty definition bounds nothing by rule.
        if !def.rules.is_empty() {
            for (j, r) in g.rules.iter().enumerate() {
                if !def.rules.iter().any(|d| rule_covers(d, r)) {
                    out.push(Finding::err(
                        "L028",
                        format!("{path}.rules[{j}]"),
                        format!(
                            "{} {} is wider than every rule of the audience's definition",
                            r.method, r.path
                        ),
                    ));
                }
            }
        }
        if let Some(ttl) = g.ttl_seconds.filter(|t| *t > def.max_ttl_seconds) {
            out.push(Finding::err(
                "L028",
                format!("{path}.ttlSeconds"),
                format!(
                    "{ttl}s above the definition's maxTtlSeconds {}",
                    def.max_ttl_seconds
                ),
            ));
        }
    }
    out
}

/// The definitions of the granted audiences against the policy's ceiling (ADR-0020, decision 8):
/// every definition endpoint and its `maxTtlSeconds` must be approved, so a team cannot open
/// egress Security did not approve by writing it into an audience it owns. A missing ceiling, or
/// an audience missing from it, is reported by `lint_exposure` (L015, L014) and not repeated here,
/// nor is an unresolved grant (L027).
pub fn lint_definitions_against_ceiling(
    grants: &[AudienceGrant],
    policy_ns: &str,
    catalog: &AudienceCatalog<'_>,
    ceiling: Option<&ExposureCeilingSpec>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    let Some(c) = ceiling else {
        return out;
    };
    for g in grants {
        let Ok(a) = catalog.resolve(policy_ns, &g.name) else {
            continue;
        };
        let Some(approved) = c.audiences.iter().find(|x| x.name == g.name) else {
            continue;
        };
        let def = &a.spec.definition;
        let path = format!("audiences[{}].definition", g.name);
        for e in def
            .endpoints
            .iter()
            .filter(|e| !approved.endpoints.contains(e))
        {
            out.push(Finding::err(
                "L014",
                format!("{path}.endpoints[{e}]"),
                "the audience's definition reaches an endpoint the ceiling does not approve",
            ));
        }
        if def.max_ttl_seconds > approved.max_ttl_seconds {
            out.push(Finding::err(
                "L014",
                format!("{path}.maxTtlSeconds"),
                format!(
                    "the definition's {}s is above the approved {}s",
                    def.max_ttl_seconds, approved.max_ttl_seconds
                ),
            ));
        }
    }
    out
}

/// Everything [`evaluate_audience`] decides from. The controller supplies `now` and the stored
/// objects; nothing here reads a clock or a cluster.
pub struct AudienceEvalInput<'a> {
    pub now: Timestamp,
    pub audience: &'a SandboxAudience,
    pub catalog: &'a AudienceCatalog<'a>,
    /// Highest version ever published for this audience (status, or the published object).
    pub prev_version: Option<u64>,
    pub prev_digest: Option<&'a str>,
    pub prev_conditions: &'a [Condition],
    /// How long a bundle stays valid: the policy bundle's lifetime (ADR-0018).
    pub bundle_lifetime: SignedDuration,
}

#[derive(Debug, Clone)]
pub struct AudienceEvalOutput {
    pub status: SandboxAudienceStatus,
    /// Present only when the audience is valid. Never signed here.
    pub bundle: Option<AudienceBundlePayload>,
    pub findings: Vec<Finding>,
    pub requeue_after: SignedDuration,
}

/// The whole decision for one `SandboxAudience` (ADR-0020, decisions 9 and 11): its findings, its
/// status, and, when it has no error, the payload to sign. The version moves only with the
/// content digest. A finding publishes nothing new and keeps the last version and digest in
/// status, because the last valid bundle is what consumers still hold until it expires.
pub fn evaluate_audience(input: &AudienceEvalInput<'_>) -> AudienceEvalOutput {
    let now = input.now;
    let generation = input.audience.metadata.generation;
    let cond = |t: &str, ok: bool, reason: &str, msg: &str| {
        condition(input.prev_conditions, t, ok, reason, msg, now, generation)
    };
    let mut status = SandboxAudienceStatus {
        observed_generation: generation,
        bundle_version: input.prev_version,
        bundle_digest: input.prev_digest.map(str::to_string),
        ..Default::default()
    };
    let requeue = (input.bundle_lifetime / 2).max(SignedDuration::from_secs(MIN_REQUEUE_SECONDS));

    let findings = lint_audience(input.audience, input.catalog);
    if has_errors(&findings) {
        let msg = summarize(&findings);
        status.conditions = vec![
            cond("Valid", false, "InvalidAudience", &msg),
            cond(
                "Ready",
                false,
                "InvalidAudience",
                "no bundle compiled for this generation",
            ),
        ];
        return AudienceEvalOutput {
            status,
            bundle: None,
            findings,
            requeue_after: requeue,
        };
    }

    let content = audience_content(input.audience);
    let digest = match content.digest() {
        Ok(d) => d,
        Err(e) => return not_canonical(status, findings, requeue, &e.to_string(), cond),
    };
    let version = match (input.prev_digest, input.prev_version) {
        (Some(d), Some(v)) if d == digest => v,
        (_, v) => v.unwrap_or(0) + 1,
    };
    let not_after = now + input.bundle_lifetime;
    let bundle = match AudienceBundlePayload::new(
        BundleMeta {
            version,
            issued_at: now,
            not_after,
        },
        content,
    ) {
        Ok(b) => b,
        Err(e) => return not_canonical(status, findings, requeue, &e.to_string(), cond),
    };
    status.bundle_version = Some(version);
    status.bundle_digest = Some(digest);
    status.bundle_not_after = Some(not_after);
    status.conditions = vec![
        cond("Valid", true, "Valid", "audience passes every lint rule"),
        cond(
            "Ready",
            true,
            "Compiled",
            "bundle compiled, awaiting signature and publication",
        ),
    ];
    AudienceEvalOutput {
        status,
        bundle: Some(bundle),
        findings,
        requeue_after: requeue,
    }
}

/// Content without an RFC 8785 canonical form (ADR-0005) is never published.
fn not_canonical(
    mut status: SandboxAudienceStatus,
    findings: Vec<Finding>,
    requeue: SignedDuration,
    why: &str,
    cond: impl Fn(&str, bool, &str, &str) -> Condition,
) -> AudienceEvalOutput {
    status.conditions = vec![
        cond("Valid", true, "Valid", "audience passes every lint rule"),
        cond("Ready", false, "NotCanonical", why),
    ];
    AudienceEvalOutput {
        status,
        bundle: None,
        findings,
        requeue_after: requeue,
    }
}

#[cfg(test)]
#[path = "audience_tests.rs"]
mod audience_tests;
