#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0

use sandboxpolicy_api::{
    Approval, CeilingAudience, ExposureCeilingSpec, SandboxAudience, SandboxAudienceSpec,
    SandboxAudienceStatus,
};
use sandboxpolicy_types::{
    Access, AudienceCredential, AudienceDefinition, AudienceGrant, Backend, CredentialFormat,
    CredentialSource, EndpointRule, HeaderPair, OpenBaoSource, Placement, Role, SecretRef,
};

use super::*;
use crate::lint::{has_errors, Severity};

const PLATFORM: &str = "sandbox-platform";

fn rule(method: &str, path: &str) -> EndpointRule {
    EndpointRule {
        method: method.into(),
        path: path.into(),
    }
}

fn secret_ref() -> CredentialSource {
    CredentialSource::SecretRef(SecretRef {
        name: "svc".into(),
        token_key: "token".into(),
        username_key: None,
    })
}

fn openbao() -> CredentialSource {
    CredentialSource::Openbao(OpenBaoSource {
        mount: "kv-platform".into(),
        path: "mediatore/x".into(),
        key: "api-key".into(),
    })
}

fn def(endpoint: &str) -> AudienceDefinition {
    AudienceDefinition {
        credential: AudienceCredential {
            backend: Backend::Secret,
            placement: Placement::Gateway,
            format: CredentialFormat::Bearer,
            source: Some(secret_ref()),
            header: None,
            extra_headers: vec![],
            refresh: None,
        },
        endpoints: vec![endpoint.into()],
        rules: vec![rule("GET", "/v2/**")],
        max_ttl_seconds: 900,
        ca_ref: None,
    }
}

fn audience(ns: &str, name: &str, d: AudienceDefinition) -> SandboxAudience {
    let mut a = SandboxAudience::new(name, SandboxAudienceSpec { definition: d });
    a.metadata.namespace = Some(ns.into());
    a
}

fn team(name: &str, endpoint: &str) -> SandboxAudience {
    audience("team-a", name, def(endpoint))
}

fn platform(name: &str, endpoint: &str) -> SandboxAudience {
    let mut d = def(endpoint);
    d.credential.source = Some(openbao());
    audience(PLATFORM, name, d)
}

fn codes(f: &[Finding]) -> Vec<&'static str> {
    f.iter().map(|f| f.code).collect()
}

fn lint_one(a: &SandboxAudience, all: &[SandboxAudience]) -> Vec<Finding> {
    lint_audience(a, &AudienceCatalog::new(PLATFORM, all))
}

fn grant(name: &str) -> AudienceGrant {
    AudienceGrant {
        name: name.into(),
        access: Access::Read,
        scopes: vec![],
        ttl_seconds: Some(300),
        endpoints: vec![],
        rules: vec![],
    }
}

// A clean team and a clean platform audience lint clean: every rule below is a departure from these.

#[test]
fn a_clean_team_audience_has_no_findings() {
    let a = team("artifactory", "af.example.com:443");
    assert!(lint_one(&a, std::slice::from_ref(&a)).is_empty());
}

#[test]
fn a_clean_platform_audience_has_no_findings() {
    let a = platform("anthropic", "api.example.com:443");
    assert!(lint_one(&a, std::slice::from_ref(&a)).is_empty());
}

// L022

#[test]
fn a_team_audience_with_a_minting_backend_is_l022() {
    for backend in [Backend::Sts, Backend::Delegated, Backend::EntraObo] {
        let mut d = def("af.example.com:443");
        d.credential.backend = backend;
        d.credential.source = None;
        let a = audience("team-a", "x", d);
        assert_eq!(
            codes(&lint_one(&a, std::slice::from_ref(&a))),
            ["L022"],
            "{backend:?}"
        );
    }
}

#[test]
fn a_team_audience_with_backend_none_is_allowed() {
    let mut d = def("af.example.com:443");
    d.credential.backend = Backend::None;
    d.credential.source = None;
    let a = audience("team-a", "x", d);
    assert!(lint_one(&a, std::slice::from_ref(&a)).is_empty());
}

#[test]
fn a_platform_audience_may_use_any_backend() {
    for backend in [Backend::Sts, Backend::Delegated, Backend::EntraObo] {
        let mut d = def("api.example.com:443");
        d.credential.backend = backend;
        d.credential.source = None;
        let a = audience(PLATFORM, "x", d);
        assert!(
            lint_one(&a, std::slice::from_ref(&a)).is_empty(),
            "{backend:?}"
        );
    }
}

// L023

#[test]
fn a_team_audience_reading_openbao_is_l023() {
    let mut d = def("af.example.com:443");
    d.credential.source = Some(openbao());
    let a = audience("team-a", "x", d);
    assert_eq!(codes(&lint_one(&a, std::slice::from_ref(&a))), ["L023"]);
}

#[test]
fn a_platform_audience_may_read_a_secret_in_its_namespace() {
    let a = audience(PLATFORM, "x", def("api.example.com:443"));
    assert!(lint_one(&a, std::slice::from_ref(&a)).is_empty());
}

// L024 and L025 land on the newcomer (ADR-0020, decision 6): an object is stable when its
// published bundle is its current content, and a colliding object is refused unless it is stable
// and the other is not.

/// `a` as published: status carries the digest of its current content.
fn published(mut a: SandboxAudience) -> SandboxAudience {
    let digest = audience_content(&a).digest().unwrap();
    a.status = Some(SandboxAudienceStatus {
        bundle_version: Some(1),
        bundle_digest: Some(digest),
        ..Default::default()
    });
    a
}

#[test]
fn a_new_team_audience_named_like_a_published_platform_one_is_l024() {
    let p = published(platform("anthropic", "api.example.com:443"));
    let t = team("anthropic", "proxy.team-a.example.com:443");
    let all = [p.clone(), t.clone()];
    assert_eq!(codes(&lint_one(&t, &all)), ["L024"]);
    assert!(lint_one(&p, &all).is_empty());
}

#[test]
fn a_new_platform_audience_named_like_a_published_team_one_is_l024_on_the_platform() {
    let p = platform("artifactory", "af.platform.example.com:443");
    let t = published(team("artifactory", "af.example.com:443"));
    let all = [p.clone(), t.clone()];
    assert_eq!(codes(&lint_one(&p, &all)), ["L024"]);
    assert!(lint_one(&t, &all).is_empty());
}

#[test]
fn a_name_collision_with_neither_published_is_l024_on_both() {
    let p = platform("anthropic", "api.example.com:443");
    let t = team("anthropic", "proxy.team-a.example.com:443");
    let all = [p.clone(), t.clone()];
    assert_eq!(codes(&lint_one(&t, &all)), ["L024"]);
    assert_eq!(codes(&lint_one(&p, &all)), ["L024"]);
}

/// A published audience edited into a collision is no longer stable: the edit is refused, the
/// unchanged one keeps publishing.
#[test]
fn an_edit_into_a_collision_is_refused_and_the_unchanged_one_is_not() {
    let a = published(team("one", "af.example.com:443"));
    let mut b = published(team("two", "other.example.com:443"));
    b.spec.definition.endpoints = vec!["af.example.com:443".into()];
    let all = [a.clone(), b.clone()];
    assert_eq!(codes(&lint_one(&b, &all)), ["L025"]);
    assert!(lint_one(&a, &all).is_empty());
}

#[test]
fn both_published_and_unchanged_in_a_collision_are_both_refused() {
    let a = published(team("one", "af.example.com:443"));
    let b = published(team("two", "af.example.com:443"));
    let all = [a.clone(), b.clone()];
    assert_eq!(codes(&lint_one(&a, &all)), ["L025"]);
    assert_eq!(codes(&lint_one(&b, &all)), ["L025"]);
}

#[test]
fn the_same_team_name_in_two_namespaces_is_fine() {
    let a = team("git", "git.a.example.com:443");
    let b = audience("team-b", "git", def("git.b.example.com:443"));
    let all = [a.clone(), b.clone()];
    assert!(lint_one(&a, &all).is_empty());
    assert!(lint_one(&b, &all).is_empty());
}

#[test]
fn a_new_team_endpoint_held_by_a_published_platform_audience_is_l025() {
    let p = published(platform("anthropic", "api.example.com:443"));
    let t = team("mine", "api.example.com:443");
    let all = [p.clone(), t.clone()];
    assert_eq!(codes(&lint_one(&t, &all)), ["L025"]);
    assert!(lint_one(&p, &all).is_empty());
}

#[test]
fn a_new_platform_endpoint_held_by_a_published_team_audience_is_l025_on_the_platform() {
    let p = platform("shared", "af.example.com:443");
    let t = published(team("af", "af.example.com:443"));
    let all = [p.clone(), t.clone()];
    assert_eq!(codes(&lint_one(&p, &all)), ["L025"]);
    assert!(lint_one(&t, &all).is_empty());
}

#[test]
fn two_new_team_audiences_on_one_endpoint_are_both_l025() {
    let a = team("one", "af.example.com:443");
    let b = team("two", "af.example.com:443");
    let all = [a.clone(), b.clone()];
    assert_eq!(codes(&lint_one(&a, &all)), ["L025"]);
    assert_eq!(codes(&lint_one(&b, &all)), ["L025"]);
}

#[test]
fn one_endpoint_in_two_team_namespaces_is_fine() {
    let a = team("af", "af.example.com:443");
    let b = audience("team-b", "af", def("af.example.com:443"));
    let all = [a.clone(), b.clone()];
    assert!(lint_one(&a, &all).is_empty());
}

#[test]
fn two_new_platform_audiences_on_one_endpoint_are_both_l025() {
    let a = platform("one", "api.example.com:443");
    let b = platform("two", "api.example.com:443");
    let all = [a.clone(), b.clone()];
    assert_eq!(codes(&lint_one(&a, &all)), ["L025"]);
    assert_eq!(codes(&lint_one(&b, &all)), ["L025"]);
}

/// `a`, published and then edited: its status digest is no longer its content's.
fn changed(a: SandboxAudience) -> SandboxAudience {
    let mut a = published(a);
    a.spec.definition.max_ttl_seconds -= 1;
    a
}

/// A refused newcomer left in place must never turn the next edit of the audience it collides
/// with into a finding: an audience with a finding is no longer re-signed, so one team namespace
/// could withdraw a platform audience from every namespace (decision 6, amended).
#[test]
fn a_never_published_audience_refuses_nothing() {
    let p = changed(platform("anthropic", "api.example.com:443"));
    let by_endpoint = team("mine", "api.example.com:443");
    let all = [p.clone(), by_endpoint.clone()];
    assert!(lint_one(&p, &all).is_empty(), "L025 from a parked newcomer");
    assert_eq!(codes(&lint_one(&by_endpoint, &all)), ["L025"]);

    let by_name = team("anthropic", "other.example.com:443");
    let all = [p.clone(), by_name.clone()];
    assert!(lint_one(&p, &all).is_empty(), "L024 from a parked newcomer");
    assert_eq!(codes(&lint_one(&by_name, &all)), ["L024"]);
}

/// The same between two team audiences in one namespace: the working one's edit goes through.
#[test]
fn a_changed_team_audience_is_not_refused_by_a_new_one() {
    let a = changed(team("one", "af.example.com:443"));
    let b = team("two", "af.example.com:443");
    let all = [a.clone(), b.clone()];
    assert!(lint_one(&a, &all).is_empty());
    assert_eq!(codes(&lint_one(&b, &all)), ["L025"]);
}

/// Two changed audiences of different tenants: the namespace decides, the platform is kept.
#[test]
fn between_two_changed_audiences_the_platform_wins() {
    let p = changed(platform("anthropic", "api.example.com:443"));
    let t = changed(team("mine", "api.example.com:443"));
    let all = [p.clone(), t.clone()];
    assert!(lint_one(&p, &all).is_empty());
    assert_eq!(codes(&lint_one(&t, &all)), ["L025"]);
}

#[test]
fn two_changed_audiences_of_one_kind_are_both_refused() {
    let a = changed(team("one", "af.example.com:443"));
    let b = changed(team("two", "af.example.com:443"));
    let all = [a.clone(), b.clone()];
    assert_eq!(codes(&lint_one(&a, &all)), ["L025"]);
    assert_eq!(codes(&lint_one(&b, &all)), ["L025"]);
}

/// A stable team audience still holds its endpoint against a platform edit: first come stays.
#[test]
fn a_changed_platform_audience_is_refused_by_a_stable_team_one() {
    let p = changed(platform("anthropic", "api.example.com:443"));
    let t = published(team("mine", "api.example.com:443"));
    let all = [p.clone(), t.clone()];
    assert_eq!(codes(&lint_one(&p, &all)), ["L025"]);
    assert!(lint_one(&t, &all).is_empty());
}

/// Whatever the standings, at least one side of a collision is refused: two colliding
/// definitions are never both published.
#[test]
fn at_least_one_side_of_every_collision_is_refused() {
    type Make = fn(SandboxAudience) -> SandboxAudience;
    let standings: [(&str, Make); 3] =
        [("new", |a| a), ("stable", published), ("changed", changed)];
    for (ln, lf) in standings {
        for (rn, rf) in standings {
            for cross in [true, false] {
                let l = if cross {
                    lf(platform("one", "x.example.com:443"))
                } else {
                    lf(team("one", "x.example.com:443"))
                };
                let r = rf(team("two", "x.example.com:443"));
                let all = [l.clone(), r.clone()];
                let refused = !lint_one(&l, &all).is_empty() || !lint_one(&r, &all).is_empty();
                assert!(
                    refused,
                    "{ln} against {rn}, cross-tenant {cross}: neither refused"
                );
            }
        }
    }
}

// L026

#[test]
fn a_reserved_or_malformed_header_is_l026() {
    for bad in [
        "host",
        "Content-Length",
        "x api key",
        "",
        "transfer-encoding",
        "proxy-authorization",
    ] {
        let mut d = def("af.example.com:443");
        d.credential.header = Some(bad.into());
        let a = audience("team-a", "x", d);
        assert_eq!(
            codes(&lint_one(&a, std::slice::from_ref(&a))),
            ["L026"],
            "{bad:?}"
        );
    }
}

#[test]
fn a_reserved_extra_header_is_l026() {
    let mut d = def("af.example.com:443");
    d.credential.extra_headers = vec![HeaderPair {
        name: "connection".into(),
        value: "close".into(),
    }];
    let a = audience("team-a", "x", d);
    assert_eq!(codes(&lint_one(&a, std::slice::from_ref(&a))), ["L026"]);
}

/// A line break in a value would end the header and start another (decision 7, amended).
#[test]
fn a_header_value_with_a_control_character_is_l026() {
    for bad in [
        "v\r\nhost: evil.example.com",
        "v\nx",
        "v\0",
        "caf\u{e9}",
        " padded",
        "padded ",
        "",
    ] {
        let mut d = def("af.example.com:443");
        d.credential.extra_headers = vec![HeaderPair {
            name: "x-ok".into(),
            value: bad.into(),
        }];
        let a = audience("team-a", "x", d);
        let f = lint_one(&a, std::slice::from_ref(&a));
        assert_eq!(codes(&f), ["L026"], "{bad:?}");
        assert!(f[0].path.ends_with(".value"), "{}", f[0].path);
    }
}

#[test]
fn a_header_value_with_inner_spaces_and_tabs_passes() {
    let mut d = def("af.example.com:443");
    d.credential.extra_headers = vec![HeaderPair {
        name: "x-ok".into(),
        value: "a b\tc; q=0.5".into(),
    }];
    let a = audience("team-a", "x", d);
    assert!(lint_one(&a, std::slice::from_ref(&a)).is_empty());
}

#[test]
fn ordinary_lowercase_headers_pass() {
    let mut d = def("af.example.com:443");
    d.credential.header = Some("x-api-key".into());
    d.credential.extra_headers = vec![HeaderPair {
        name: "anthropic-version".into(),
        value: "2023-06-01".into(),
    }];
    let a = audience("team-a", "x", d);
    assert!(lint_one(&a, std::slice::from_ref(&a)).is_empty());
}

// L029

#[test]
fn a_team_audience_in_the_proxy_is_a_warning_l029() {
    let mut d = def("af.example.com:443");
    d.credential.placement = Placement::Proxy;
    let a = audience("team-a", "x", d);
    let f = lint_one(&a, std::slice::from_ref(&a));
    assert_eq!(codes(&f), ["L029"]);
    assert_eq!(f[0].severity, Severity::Warning);
    assert!(!has_errors(&f));
}

#[test]
fn a_platform_audience_in_the_proxy_is_not_warned() {
    let mut d = def("api.example.com:443");
    d.credential.placement = Placement::Proxy;
    let a = audience(PLATFORM, "x", d);
    assert!(lint_one(&a, std::slice::from_ref(&a)).is_empty());
}

// L017 on a definition

#[test]
fn a_definition_endpoint_that_is_a_wildcard_or_ip_is_l017() {
    for bad in ["*.example.com:443", "192.0.2.1:443", "no-port"] {
        let a = audience("team-a", "x", def(bad));
        assert_eq!(
            codes(&lint_one(&a, std::slice::from_ref(&a))),
            ["L017"],
            "{bad:?}"
        );
    }
}

// L027

#[test]
fn a_grant_resolves_a_platform_audience_from_any_namespace() {
    let all = [platform("anthropic", "api.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    assert!(lint_grants(&[grant("anthropic")], "team-z", &cat).is_empty());
}

#[test]
fn a_grant_with_no_definition_is_l027() {
    let all: [SandboxAudience; 0] = [];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    assert_eq!(
        codes(&lint_grants(&[grant("ghost")], "team-a", &cat)),
        ["L027"]
    );
}

#[test]
fn a_grant_of_another_namespaces_team_audience_is_l027() {
    let all = [team("artifactory", "af.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    assert!(lint_grants(&[grant("artifactory")], "team-a", &cat).is_empty());
    let f = lint_grants(&[grant("artifactory")], "team-b", &cat);
    assert_eq!(codes(&f), ["L027"]);
    assert!(f[0].message.contains("team-a"), "{}", f[0].message);
}

#[test]
fn a_platform_audience_wins_over_a_team_audience_of_the_same_name() {
    let p = platform("anthropic", "api.example.com:443");
    let t = team("anthropic", "other.example.com:443");
    let all = [p, t];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    let resolved = cat.resolve("team-a", "anthropic").unwrap();
    assert_eq!(resolved.metadata.namespace.as_deref(), Some(PLATFORM));
}

// L028

#[test]
fn a_grant_inside_its_definition_passes() {
    let all = [team("af", "af.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    let mut g = grant("af");
    g.endpoints = vec!["af.example.com:443".into()];
    g.rules = vec![
        rule("GET", "/v2/library/**"),
        rule("GET", "/v2/x/manifests/*"),
    ];
    assert!(lint_grants(&[g], "team-a", &cat).is_empty());
}

/// Empty grant endpoints reach nothing (fail closed, as mediatore's effective grant), and empty
/// grant rules are the definition's: neither is wider than the definition.
#[test]
fn empty_grant_endpoints_and_rules_are_never_wider() {
    let all = [team("af", "af.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    assert!(lint_grants(&[grant("af")], "team-a", &cat).is_empty());
}

#[test]
fn a_grant_endpoint_outside_the_definition_is_l028() {
    let all = [team("af", "af.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    let mut g = grant("af");
    g.endpoints = vec!["elsewhere.example.com:443".into()];
    assert_eq!(codes(&lint_grants(&[g], "team-a", &cat)), ["L028"]);
}

#[test]
fn a_grant_rule_wider_than_the_definition_is_l028() {
    let all = [team("af", "af.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    for wide in [
        rule("PUT", "/v2/x"),
        rule("*", "/v2/x"),
        rule("GET", "/**"),
        rule("GET", "/v3/x"),
    ] {
        let mut g = grant("af");
        g.rules = vec![wide.clone()];
        assert_eq!(
            codes(&lint_grants(&[g], "team-a", &cat)),
            ["L028"],
            "{wide:?}"
        );
    }
}

#[test]
fn a_grant_ttl_above_the_definition_is_l028() {
    let all = [team("af", "af.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    let mut g = grant("af");
    g.ttl_seconds = Some(901);
    assert_eq!(codes(&lint_grants(&[g], "team-a", &cat)), ["L028"]);
}

#[test]
fn a_definition_without_rules_bounds_nothing_by_rule() {
    let mut d = def("af.example.com:443");
    d.rules = vec![];
    let all = [audience("team-a", "af", d)];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    let mut g = grant("af");
    g.rules = vec![rule("*", "/**")];
    assert!(lint_grants(&[g], "team-a", &cat).is_empty());
}

// Rule containment, the heart of L028.

#[test]
fn rule_containment() {
    let yes = [
        ("GET", "/v2/**", "GET", "/v2/a/b"),
        ("GET", "/v2/**", "GET", "/v2"),
        ("GET", "/v2/**", "GET", "/v2/*/x/**"),
        ("*", "/v2/*", "HEAD", "/v2/a"),
        ("*", "/v2/*", "*", "/v2/*"),
        ("get", "/a", "GET", "/a"),
        ("GET", "/**", "GET", "/**"),
    ];
    for (dm, dp, gm, gp) in yes {
        assert!(
            rule_covers(&rule(dm, dp), &rule(gm, gp)),
            "{dm} {dp} should cover {gm} {gp}"
        );
    }
    let no = [
        ("GET", "/v2/*", "GET", "/v2/a/b"),
        ("GET", "/v2/*", "GET", "/v2/**"),
        ("GET", "/v2/a", "GET", "/v2/*"),
        ("GET", "/v2/**", "*", "/v2/a"),
        ("GET", "/v2/**", "GET", "/v20"),
    ];
    for (dm, dp, gm, gp) in no {
        assert!(
            !rule_covers(&rule(dm, dp), &rule(gm, gp)),
            "{dm} {dp} must not cover {gm} {gp}"
        );
    }
}

// L014 extended

fn ceiling(endpoints: &[&str], max_ttl: u32) -> ExposureCeilingSpec {
    ExposureCeilingSpec {
        audiences: vec![CeilingAudience {
            name: "af".into(),
            access: Access::Read,
            max_ttl_seconds: max_ttl,
            endpoints: endpoints.iter().map(|e| e.to_string()).collect(),
            data_scope: None,
        }],
        egress_hosts: vec![],
        tools: vec![],
        max_lease_seconds: 3600,
        approvals: vec![Approval {
            role: Role::Security,
            reference: "r".into(),
        }],
    }
}

#[test]
fn a_definition_covered_by_the_ceiling_passes() {
    let all = [team("af", "af.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    let c = ceiling(&["af.example.com:443"], 900);
    assert!(lint_definitions_against_ceiling(&[grant("af")], "team-a", &cat, Some(&c)).is_empty());
}

#[test]
fn a_definition_endpoint_outside_the_ceiling_is_l014() {
    let all = [team("af", "af.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    let c = ceiling(&["other.example.com:443"], 900);
    let f = lint_definitions_against_ceiling(&[grant("af")], "team-a", &cat, Some(&c));
    assert_eq!(codes(&f), ["L014"]);
}

#[test]
fn a_definition_ttl_above_the_ceiling_is_l014() {
    let all = [team("af", "af.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    let c = ceiling(&["af.example.com:443"], 600);
    let f = lint_definitions_against_ceiling(&[grant("af")], "team-a", &cat, Some(&c));
    assert_eq!(codes(&f), ["L014"]);
}

/// The ceiling's own absence or missing audience is the existing L015 / L014, reported by
/// `lint_exposure`; this function does not report it twice.
#[test]
fn no_ceiling_or_no_ceiling_audience_is_left_to_lint_exposure() {
    let all = [team("af", "af.example.com:443")];
    let cat = AudienceCatalog::new(PLATFORM, &all);
    assert!(lint_definitions_against_ceiling(&[grant("af")], "team-a", &cat, None).is_empty());
    let mut c = ceiling(&[], 900);
    c.audiences.clear();
    assert!(lint_definitions_against_ceiling(&[grant("af")], "team-a", &cat, Some(&c)).is_empty());
}

#[test]
fn every_new_code_is_in_rules() {
    for code in [
        "L022", "L023", "L024", "L025", "L026", "L027", "L028", "L029",
    ] {
        assert!(crate::lint::RULES.iter().any(|(c, _)| *c == code), "{code}");
    }
}

// evaluate_audience

fn now() -> jiff::Timestamp {
    "2026-11-20T12:00:00Z".parse().unwrap()
}

fn eval<'a>(
    a: &'a SandboxAudience,
    all: &'a [SandboxAudience],
    prev_version: Option<u64>,
    prev_digest: Option<&'a str>,
) -> AudienceEvalOutput {
    let cat = AudienceCatalog::new(PLATFORM, all);
    evaluate_audience(&AudienceEvalInput {
        now: now(),
        audience: a,
        catalog: &cat,
        prev_version,
        prev_digest,
        prev_conditions: &[],
        bundle_lifetime: jiff::SignedDuration::from_mins(15),
    })
}

fn ready(o: &AudienceEvalOutput) -> &sandboxpolicy_api::Condition {
    o.status
        .conditions
        .iter()
        .find(|c| c.type_ == "Ready")
        .unwrap()
}

#[test]
fn a_valid_audience_compiles_version_one() {
    let a = team("af", "af.example.com:443");
    let o = eval(&a, std::slice::from_ref(&a), None, None);
    let b = o.bundle.as_ref().expect("a bundle");
    assert_eq!(b.meta.version, 1);
    assert_eq!(
        b.meta.not_after,
        now() + jiff::SignedDuration::from_mins(15)
    );
    assert_eq!(b.content.audience.namespace, "team-a");
    assert_eq!(o.status.bundle_version, Some(1));
    assert_eq!(
        o.status.bundle_digest.as_deref(),
        Some(b.content_digest.as_str())
    );
    assert_eq!(ready(&o).status, "True");
}

#[test]
fn an_unchanged_audience_keeps_its_version() {
    let a = team("af", "af.example.com:443");
    let digest = audience_content(&a).digest().unwrap();
    let o = eval(&a, std::slice::from_ref(&a), Some(4), Some(&digest));
    assert_eq!(o.bundle.unwrap().meta.version, 4);
}

#[test]
fn a_changed_audience_bumps_its_version() {
    let a = team("af", "af.example.com:443");
    let o = eval(&a, std::slice::from_ref(&a), Some(4), Some("sha256:old"));
    assert_eq!(o.bundle.unwrap().meta.version, 5);
}

/// The order endpoints and rules are written in is not content.
#[test]
fn list_order_does_not_change_the_digest() {
    let mut a = team("af", "af.example.com:443");
    a.spec.definition.endpoints = vec!["b.example.com:443".into(), "a.example.com:443".into()];
    let mut b = a.clone();
    b.spec.definition.endpoints.reverse();
    assert_eq!(
        audience_content(&a).digest().unwrap(),
        audience_content(&b).digest().unwrap()
    );
}

/// A finding blocks a new version and keeps the last valid one's version and digest in status:
/// that bundle stays until its lifetime ends (decision 11).
#[test]
fn a_finding_publishes_nothing_and_keeps_the_last_version() {
    let mut d = def("af.example.com:443");
    d.credential.backend = Backend::Sts;
    let a = audience("team-a", "af", d);
    let o = eval(&a, std::slice::from_ref(&a), Some(3), Some("sha256:last"));
    assert!(o.bundle.is_none());
    assert_eq!(o.status.bundle_version, Some(3));
    assert_eq!(o.status.bundle_digest.as_deref(), Some("sha256:last"));
    assert_eq!(ready(&o).status, "False");
    let valid = o
        .status
        .conditions
        .iter()
        .find(|c| c.type_ == "Valid")
        .unwrap();
    assert_eq!(valid.status, "False");
    assert!(valid.message.contains("L022"), "{}", valid.message);
}

#[test]
fn a_warning_still_publishes() {
    let mut d = def("af.example.com:443");
    d.credential.placement = Placement::Proxy;
    let a = audience("team-a", "af", d);
    let o = eval(&a, std::slice::from_ref(&a), None, None);
    assert!(o.bundle.is_some());
    assert_eq!(codes(&o.findings), ["L029"]);
}

/// The previous version comes in part from a ConfigMap annotation anyone in the namespace can
/// write: at the largest safe integer nothing more is issued, rather than a version consumers
/// misread, or one that wraps to 0 and is refused as a downgrade.
#[test]
fn a_version_at_the_safe_integer_limit_issues_nothing_and_keeps_status() {
    let a = team("af", "af.example.com:443");
    for prev in [(1u64 << 53) - 1, u64::MAX] {
        let o = eval(
            &a,
            std::slice::from_ref(&a),
            Some(prev),
            Some("sha256:other"),
        );
        assert!(o.bundle.is_none(), "{prev}");
        assert_eq!(o.status.bundle_version, Some(prev));
        assert_eq!(ready(&o).reason, "VersionExhausted");
    }
    let o = eval(
        &a,
        std::slice::from_ref(&a),
        Some((1 << 53) - 2),
        Some("sha256:other"),
    );
    assert_eq!(o.bundle.unwrap().meta.version, (1 << 53) - 1);
}
