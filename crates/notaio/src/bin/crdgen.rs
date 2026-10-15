//! Generates the raw manifests that are checked in under `deploy/`.
//!
//! `crdgen crds`      prints the three CustomResourceDefinitions.
//! `crdgen profiles`  prints the four built-in SandboxProfile objects.
//!
//! The checked-in YAML is always the output of this tool, never hand edited. CI runs
//! `make manifests-check` to fail on drift.

use kube::CustomResourceExt;
use sandboxpolicy_api::{KnobGrant, SandboxPolicy, SandboxProfile};
use sandboxpolicy_core::builtin;

/// Kubernetes formats for the unsigned integer formats schemars emits. The API server does not
/// recognise `uint8` to `uint64` and, since Kubernetes 1.36, warns on every apply. The width is kept:
/// `u8` and `u16` fit `int32`, `u32` and `u64` map to `int64`, which is also every integer's storage
/// width in the API server. schemars already adds `minimum: 0` (and the `maximum` for `u8`/`u16`), so
/// validation does not change.
const FORMAT_REWRITES: [(&str, &str); 4] = [
    ("uint8", "int32"),
    ("uint16", "int32"),
    ("uint32", "int64"),
    ("uint64", "int64"),
];

/// Rewrites every `format` in a schema tree that Kubernetes would not recognise (`FORMAT_REWRITES`).
fn rewrite_formats(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(f)) = map.get_mut("format") {
                if let Some((_, k8s)) = FORMAT_REWRITES.iter().find(|(from, _)| from == f) {
                    *f = (*k8s).to_string();
                }
            }
            map.values_mut().for_each(rewrite_formats);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(rewrite_formats),
        _ => {}
    }
}

fn emit<T: serde::Serialize>(doc: &T) {
    // serde_saphyr emits a document without a leading separator.
    match serde_saphyr::to_string(doc) {
        Ok(s) => print!("---\n{s}"),
        Err(e) => {
            eprintln!("serialisation failed: {e}");
            std::process::exit(1);
        }
    }
}

/// Emits a CRD with its schema formats made Kubernetes-compatible.
fn emit_crd<T: serde::Serialize>(crd: &T) {
    let mut v = match serde_json::to_value(crd) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("serialisation failed: {e}");
            std::process::exit(1);
        }
    };
    rewrite_formats(&mut v);
    emit(&v);
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("crds") => {
            emit_crd(&SandboxProfile::crd());
            emit_crd(&SandboxPolicy::crd());
            emit_crd(&KnobGrant::crd());
        }
        Some("profiles") => {
            for (name, spec) in builtin::all() {
                let mut p = SandboxProfile::new(name, spec);
                p.metadata.labels = Some(
                    [(
                        "app.kubernetes.io/managed-by".to_string(),
                        "notaio".to_string(),
                    )]
                    .into(),
                );
                emit(&p);
            }
        }
        _ => {
            eprintln!("usage: crdgen <crds|profiles>");
            std::process::exit(2);
        }
    }
}
