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
