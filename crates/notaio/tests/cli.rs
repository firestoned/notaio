#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0

//! The `notaio` binary (ADR-0006): subcommands, exit codes, SARIF, and the arguments the
//! deployment passes.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use clap::Parser;
use notaio::cli::Cli;

const NOW: &str = "2026-10-20T00:00:00Z";

const CLEAN_POLICY: &str = r#"apiVersion: sandbox.firestoned.io/v1alpha2
kind: SandboxPolicy
metadata:
  name: agent-default
  namespace: sandboxes
spec:
  profileRef: hardened
  subjects:
    groups: ["00000000-0000-0000-0000-000000000000"]
  audiences:
    - name: internal-git
      access: Read
      scopes: ["repo:read"]
  egress: []
  tools: []
"#;

const CEILING: &str = r#"apiVersion: sandbox.firestoned.io/v1alpha2
kind: ExposureCeiling
metadata:
  name: agent-default
  namespace: sandboxes
spec:
  audiences:
    - name: internal-git
      access: Read
      maxTtlSeconds: 300
  egressHosts: []
  tools: []
  maxLeaseSeconds: 14400
  approvals:
    - role: Security
      reference: https://example.com/review/2
"#;

const JUMPING_GRANT: &str = r#"apiVersion: sandbox.firestoned.io/v1alpha2
kind: KnobGrant
metadata:
  name: jump
  namespace: sandboxes
spec:
  policyRef: agent-default
  knob: K2.3
  step: R2
  owner: team-e2e
  reason: two steps at once
  expiresAt: "2026-10-27T00:00:00Z"
  approvals:
    - role: PlatformOwner
      reference: https://example.com/review/1
"#;

/// A fresh directory holding `files`, under cargo's per-test temporary directory.
fn repo(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("nested")).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir
}

fn notaio(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_notaio"))
        .args(args)
        .output()
        .expect("run notaio")
}

fn lint(dir: &Path, extra: &[&str]) -> Output {
    let d = dir.to_str().unwrap();
    let mut args = vec!["lint", "--now", NOW];
    args.extend_from_slice(extra);
    args.push(d);
    notaio(&args)
}

#[test]
fn clean_repository_exits_zero() {
    let dir = repo(
        "cli-clean",
        &[
            ("nested/policy.yaml", CLEAN_POLICY),
            ("ceiling.yaml", CEILING),
        ],
    );
    let out = lint(&dir, &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn an_error_finding_exits_one_and_names_code_and_file() {
    let dir = repo(
        "cli-error",
        &[
            ("policy.yaml", CLEAN_POLICY),
            ("nested/grant.yml", JUMPING_GRANT),
        ],
    );
    let out = lint(&dir, &[]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("L011"), "{text}");
    assert!(text.contains("grant.yml"), "{text}");
    assert!(text.contains("KnobGrant sandboxes/jump"), "{text}");
}

#[test]
fn unreadable_input_exits_two() {
    let dir = repo("cli-broken", &[("broken.yaml", "kind: [unclosed\n")]);
    assert_eq!(lint(&dir, &[]).status.code(), Some(2));
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-does-not-exist");
    assert_eq!(lint(&missing, &[]).status.code(), Some(2));
}

#[test]
fn sarif_output_names_rules_results_and_locations() {
    let dir = repo(
        "cli-sarif",
        &[("policy.yaml", CLEAN_POLICY), ("grant.yaml", JUMPING_GRANT)],
    );
    let out = lint(&dir, &["--format", "sarif"]);
    assert_eq!(out.status.code(), Some(1));
    let sarif: serde_json::Value = serde_json::from_slice(&out.stdout).expect("SARIF is JSON");
    assert_eq!(sarif["version"], "2.1.0");
    let run = &sarif["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "notaio");
    let rules: Vec<&str> = run["tool"]["driver"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    for code in ["L001", "L013", "L014", "L016", "I001", "I007", "I008"] {
        assert!(rules.contains(&code), "rule {code} missing from {rules:?}");
    }
    let result = run["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["ruleId"] == "L011")
        .expect("an L011 result");
    assert_eq!(result["level"], "error");
    let uri = result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
        .as_str()
        .unwrap();
    assert!(uri.ends_with("grant.yaml"), "{uri}");
}

#[test]
fn bare_notaio_prints_usage_and_fails() {
    let out = notaio(&[]);
    assert_ne!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stderr).contains("Usage"));
}

#[test]
fn the_deployment_arguments_parse() {
    // The manifests and the binary must agree on how the controller is started (ADR-0006).
    let manifest = include_str!("../../../deploy/controller/20-deployment.yaml");
    let deployment: serde_json::Value = serde_saphyr::from_str(manifest).unwrap();
    let args: Vec<String> = deployment["spec"]["template"]["spec"]["containers"][0]["args"]
        .as_array()
        .expect("the deployment passes args")
        .iter()
        .map(|a| a.as_str().unwrap().to_string())
        .collect();
    assert_eq!(args, vec!["controller"]);
    let argv = std::iter::once("notaio".to_string()).chain(args);
    Cli::try_parse_from(argv).expect("the deployment's arguments parse");
}
