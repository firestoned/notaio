#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0

use serde_json::json;

use super::*;

fn canon(v: serde_json::Value) -> String {
    String::from_utf8(canonicalize(&v).unwrap()).unwrap()
}

#[test]
fn sorts_members_by_utf16_code_units_rfc8785_3_2_3() {
    // RFC 8785 section 3.2.3. UTF-8 byte order would put U+FB33 before U+1F600; UTF-16 code unit
    // order (a surrogate pair starting 0xD83D) puts the emoji first.
    let v = json!({
        "\u{20ac}": "Euro Sign",
        "\r": "Carriage Return",
        "\u{fb33}": "Hebrew Letter Dalet With Dagesh",
        "1": "One",
        "\u{1f600}": "Emoji: Grinning Face",
        "\u{0080}": "Control",
        "\u{00f6}": "Latin Small Letter O With Diaeresis"
    });
    assert_eq!(
        canon(v),
        "{\"\\r\":\"Carriage Return\",\"1\":\"One\",\"\u{0080}\":\"Control\",\
         \"\u{00f6}\":\"Latin Small Letter O With Diaeresis\",\"\u{20ac}\":\"Euro Sign\",\
         \"\u{1f600}\":\"Emoji: Grinning Face\",\"\u{fb33}\":\"Hebrew Letter Dalet With Dagesh\"}"
    );
}

#[test]
fn escapes_strings_like_rfc8785_3_2_2_2() {
    // RFC 8785 section 3.2.2.2: "\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/"
    let v = json!("\u{20ac}$\u{000f}\nA'B\"\\\\\"/");
    assert_eq!(canon(v), r#""€$\u000f\nA'B\"\\\\\"/""#);
}

#[test]
fn control_characters_use_short_or_lowercase_unicode_escapes() {
    // Only U+0000 to U+001F are escaped; DEL (U+007F) and everything above are written as is.
    let v = json!("\u{8}\u{9}\u{a}\u{c}\u{d}\u{1f}\u{7f}");
    assert_eq!(canon(v), "\"\\b\\t\\n\\f\\r\\u001f\u{7f}\"");
}

#[test]
fn no_whitespace_and_nested_members_sorted() {
    let v = json!({"b": [null, true, false], "a": {"y": 1, "x": []}});
    assert_eq!(canon(v), r#"{"a":{"x":[],"y":1},"b":[null,true,false]}"#);
}

#[test]
fn integers_in_the_safe_range_are_written_plainly() {
    assert_eq!(canon(json!(0)), "0");
    assert_eq!(canon(json!(MAX_SAFE_INTEGER)), "9007199254740991");
    assert_eq!(
        canon(json!(-(MAX_SAFE_INTEGER as i64))),
        "-9007199254740991"
    );
}

#[test]
fn integers_beyond_the_safe_range_are_refused() {
    for v in [
        json!(MAX_SAFE_INTEGER + 1),
        json!(u64::MAX),
        json!(-(MAX_SAFE_INTEGER as i64) - 1),
    ] {
        assert!(
            matches!(canonicalize(&v), Err(BundleError::NonCanonical(_))),
            "{v} was canonicalised"
        );
    }
}

#[test]
fn floats_are_refused() {
    for v in [json!(1.5), json!(1.0)] {
        assert!(
            matches!(canonicalize(&v), Err(BundleError::NonCanonical(_))),
            "{v} was canonicalised"
        );
    }
}
