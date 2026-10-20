//! RFC 8785 JSON Canonicalization Scheme, for the integer-only JSON the bundle content is (ADR-0005).
//!
//! Implemented here rather than taken from a crate: the subset is small (no floats), the bound in
//! [`MAX_SAFE_INTEGER`] must be enforced either way, and this crate runs as root in every sandbox VM
//! through mediatore-guest, so every dependency it avoids is attack surface it does not carry.

use std::fmt::Write as _;

use crate::BundleError;

/// The largest integer an IEEE-754 double holds exactly (I-JSON). RFC 8785 writes numbers in that
/// form, so content integers must stay within plus or minus this value (ADR-0005, decision 2).
pub const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// The RFC 8785 canonical bytes of `value`.
///
/// Object members are sorted by the UTF-16 code units of their names, strings use the minimal
/// escaping of RFC 8785 section 3.2.2.2, and there is no insignificant whitespace.
///
/// # Errors
/// `BundleError::NonCanonical` for a float, or an integer beyond plus or minus
/// [`MAX_SAFE_INTEGER`]: their RFC 8785 form is a double, which would not round-trip exactly, so
/// two implementations could disagree on the digest.
pub fn canonicalize(value: &serde_json::Value) -> Result<Vec<u8>, BundleError> {
    let mut out = String::new();
    write_value(&mut out, value)?;
    Ok(out.into_bytes())
}

fn write_value(out: &mut String, value: &serde_json::Value) -> Result<(), BundleError> {
    match value {
        serde_json::Value::Null => out.push_str("null"),
        serde_json::Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        serde_json::Value::Number(n) => write_number(out, n)?,
        serde_json::Value::String(s) => write_string(out, s),
        serde_json::Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item)?;
            }
            out.push(']');
        }
        serde_json::Value::Object(map) => {
            let mut members: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            members.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, (name, member)) in members.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, name);
                out.push(':');
                write_value(out, member)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn write_number(out: &mut String, n: &serde_json::Number) -> Result<(), BundleError> {
    if let Some(u) = n.as_u64() {
        if u > MAX_SAFE_INTEGER {
            return Err(BundleError::NonCanonical(format!(
                "integer {u} is above 2^53 - 1"
            )));
        }
        out.push_str(&u.to_string());
        return Ok(());
    }
    if let Some(i) = n.as_i64() {
        if i.unsigned_abs() > MAX_SAFE_INTEGER {
            return Err(BundleError::NonCanonical(format!(
                "integer {i} is below -(2^53 - 1)"
            )));
        }
        out.push_str(&i.to_string());
        return Ok(());
    }
    Err(BundleError::NonCanonical(format!("{n} is not an integer")))
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{09}' => out.push_str("\\t"),
            '\u{0a}' => out.push_str("\\n"),
            '\u{0c}' => out.push_str("\\f"),
            '\u{0d}' => out.push_str("\\r"),
            c if c < '\u{20}' => {
                // Writing to a String cannot fail.
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
#[path = "jcs_tests.rs"]
mod jcs_tests;
