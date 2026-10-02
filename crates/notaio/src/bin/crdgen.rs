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

fn emit<T: serde::Serialize>(doc: &T) {
    // serde_yaml_ng emits a document without a leading separator.
    match serde_yaml_ng::to_string(doc) {
        Ok(s) => print!("---\n{s}"),
        Err(e) => {
            eprintln!("serialisation failed: {e}");
            std::process::exit(1);
        }
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("crds") => {
            emit(&SandboxProfile::crd());
            emit(&SandboxPolicy::crd());
            emit(&KnobGrant::crd());
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
