#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::path::PathBuf;

use base64::engine::general_purpose::STANDARD;

use super::*;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: BTreeMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |name| map.get(name).cloned()
}

#[test]
fn no_signer_configured() {
    assert!(matches!(choose_signer(env(&[])), Ok(SignerChoice::None)));
}

#[test]
fn development_signer_with_its_default_key_id() {
    let c = choose_signer(env(&[("NOTAIO_SIGNING_KEY_FILE", "/k/seed")])).unwrap();
    let SignerChoice::Development { key_file, key_id } = c else {
        panic!("not the development signer: {c:?}");
    };
    assert_eq!(key_file, PathBuf::from("/k/seed"));
    assert_eq!(key_id, "dev");
}

#[test]
fn openbao_signer_with_defaults() {
    let c = choose_signer(env(&[
        ("NOTAIO_OPENBAO_ADDR", "https://openbao.openbao.svc:8200"),
        ("NOTAIO_OPENBAO_CA_FILE", "/ca/ca.crt"),
    ]))
    .unwrap();
    let SignerChoice::OpenBao(t) = c else {
        panic!("not the OpenBao signer: {c:?}");
    };
    assert_eq!(t.addr, "https://openbao.openbao.svc:8200");
    assert_eq!(t.ca_file, PathBuf::from("/ca/ca.crt"));
    assert_eq!(t.role, "notaio");
    assert_eq!(t.key, "notaio");
    assert_eq!(t.auth_path, "kubernetes");
    assert_eq!(
        t.token_file,
        PathBuf::from("/var/run/secrets/openbao/token")
    );
    assert_eq!(t.key_version, None, "unpinned signs with the latest");
}

fn openbao_with_version(v: &str) -> Result<SignerChoice, SignError> {
    choose_signer(env(&[
        ("NOTAIO_OPENBAO_ADDR", "https://openbao.openbao.svc:8200"),
        ("NOTAIO_OPENBAO_CA_FILE", "/ca/ca.crt"),
        ("NOTAIO_OPENBAO_KEY_VERSION", v),
    ]))
}

#[test]
fn a_key_version_can_be_pinned() {
    let Ok(SignerChoice::OpenBao(t)) = openbao_with_version("3") else {
        panic!("not the OpenBao signer");
    };
    assert_eq!(t.key_version, Some(3));
}

#[test]
fn a_bad_key_version_is_refused_at_startup() {
    for bad in ["0", "-1", "v3", "latest", "", "1.5"] {
        assert!(openbao_with_version(bad).is_err(), "{bad:?} was accepted");
    }
}

#[test]
fn the_sign_request_names_the_pinned_version_only() {
    let unpinned = sign_request(b"m", None);
    assert_eq!(unpinned["input"], STANDARD.encode(b"m"));
    assert!(unpinned.get("key_version").is_none());
    assert_eq!(sign_request(b"m", Some(4))["key_version"], 4);
}

#[test]
fn a_signature_from_another_version_is_refused_when_pinned() {
    // ADR-0004, amended: a bundle signed by a version consumers may not trust is never published.
    assert!(ensure_version("notaio", "notaio:v3", Some(3)).is_ok());
    assert!(ensure_version("notaio", "notaio:v4", None).is_ok());
    let e = ensure_version("notaio", "notaio:v4", Some(3)).unwrap_err();
    assert!(e.to_string().contains("pins v3"), "{e}");
    // A prefix is not a match: v13 is not v1.
    assert!(ensure_version("notaio", "notaio:v13", Some(1)).is_err());
}

#[test]
fn both_signers_configured_is_refused() {
    // Ambiguity about which key signs is never resolved silently (ADR-0004, decision 5).
    let r = choose_signer(env(&[
        ("NOTAIO_SIGNING_KEY_FILE", "/k/seed"),
        ("NOTAIO_OPENBAO_ADDR", "https://openbao.openbao.svc:8200"),
        ("NOTAIO_OPENBAO_CA_FILE", "/ca/ca.crt"),
    ]));
    assert!(r.is_err(), "{r:?}");
}

#[test]
fn openbao_without_a_ca_is_refused() {
    // TLS to OpenBao is verified against its own CA, never the system roots or nothing.
    let r = choose_signer(env(&[(
        "NOTAIO_OPENBAO_ADDR",
        "https://openbao.openbao.svc:8200",
    )]));
    assert!(r.is_err(), "{r:?}");
}

#[test]
fn openbao_over_plain_http_is_refused() {
    let r = choose_signer(env(&[
        ("NOTAIO_OPENBAO_ADDR", "http://openbao.openbao.svc:8200"),
        ("NOTAIO_OPENBAO_CA_FILE", "/ca/ca.crt"),
    ]));
    assert!(r.is_err(), "{r:?}");
}

#[test]
fn a_transit_signature_becomes_a_key_id_and_raw_bytes() {
    let raw = [9u8; 64];
    let (key_id, sig) =
        parse_signature("notaio", &format!("vault:v3:{}", STANDARD.encode(raw))).unwrap();
    // ADR-0004, decision 3: the key id names the key version.
    assert_eq!(key_id, "notaio:v3");
    assert_eq!(sig, raw.to_vec());
}

#[test]
fn malformed_transit_signatures_are_refused() {
    let good = STANDARD.encode([9u8; 64]);
    for bad in [
        format!("v1:{good}"),
        format!("vault:1:{good}"),
        format!("vault:vx:{good}"),
        "vault:v1:not base64!!".to_string(),
        format!("vault:v1:{}", STANDARD.encode([9u8; 32])),
        String::new(),
    ] {
        assert!(
            parse_signature("notaio", &bad).is_err(),
            "{bad:?} was accepted"
        );
    }
}

#[test]
fn a_login_response_gives_the_token_and_its_lifetime() {
    let body = br#"{"auth":{"client_token":"s.abc","lease_duration":900,"renewable":true}}"#;
    let (token, lease) = parse_login(body).unwrap();
    assert_eq!(token, "s.abc");
    assert_eq!(lease, 900);
}

#[test]
fn a_login_response_without_a_token_is_refused() {
    for body in [
        br#"{"auth":null}"#.as_slice(),
        br#"{"auth":{"client_token":"","lease_duration":900}}"#.as_slice(),
        b"not json".as_slice(),
    ] {
        assert!(parse_login(body).is_err());
    }
}
