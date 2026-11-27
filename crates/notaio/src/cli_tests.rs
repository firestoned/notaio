// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

#[test]
fn unset_is_the_default_of_fifteen_minutes() {
    // ADR-0018, decision 1.
    assert_eq!(DEFAULT_BUNDLE_LIFETIME_SECONDS, 900);
    assert_eq!(bundle_lifetime_seconds(None).unwrap(), 900);
}

#[test]
fn a_value_within_the_bounds_is_used() {
    for (raw, want) in [
        ("300", 300),
        ("1800", 1800),
        ("3600", 3600),
        ("86400", 86400),
    ] {
        assert_eq!(bundle_lifetime_seconds(Some(raw)).unwrap(), want, "{raw}");
    }
}

#[test]
fn a_value_that_does_not_parse_or_is_out_of_bounds_is_an_error_naming_the_variable() {
    // ADR-0018, decision 2: never a silent fallback, never zero or negative.
    for raw in ["15m", "", "abc", "0", "-900", "299", "86401", "1e3"] {
        let err = bundle_lifetime_seconds(Some(raw))
            .expect_err(raw)
            .to_string();
        assert!(err.contains(ENV_BUNDLE_LIFETIME), "{raw}: {err}");
    }
}

#[test]
fn an_unset_or_empty_platform_audience_namespace_leaves_audiences_off() {
    // ADR-0020, decision 2: opt-in.
    assert_eq!(audience_platform_namespace(None).unwrap(), None);
    assert_eq!(audience_platform_namespace(Some("")).unwrap(), None);
    assert_eq!(audience_platform_namespace(Some("  ")).unwrap(), None);
}

#[test]
fn a_platform_audience_namespace_is_a_namespace_name() {
    assert_eq!(
        audience_platform_namespace(Some("sandbox-platform")).unwrap(),
        Some("sandbox-platform".to_string())
    );
    for raw in ["Sandbox", "-x", "x-", "a/b", "a.b", &"a".repeat(64)] {
        let err = audience_platform_namespace(Some(raw))
            .expect_err(raw)
            .to_string();
        assert!(
            err.contains(ENV_AUDIENCE_PLATFORM_NAMESPACE),
            "{raw}: {err}"
        );
    }
}

// NOTAIO_BUNDLE_SCHEMA (ADR-0026 decision 5).

#[test]
fn the_bundle_schema_is_v3_unless_set() {
    assert_eq!(bundle_schema(None).unwrap(), SchemaVersion::V3);
    assert_eq!(bundle_schema(Some("v3")).unwrap(), SchemaVersion::V3);
    assert_eq!(bundle_schema(Some(" v4 ")).unwrap(), SchemaVersion::V4);
}

#[test]
fn any_other_bundle_schema_stops_the_controller_naming_the_variable() {
    for bad in ["", "V4", "4", "v5", "bundle/v4"] {
        let e = bundle_schema(Some(bad)).unwrap_err().to_string();
        assert!(e.contains(ENV_BUNDLE_SCHEMA), "{bad:?}: {e}");
    }
}
