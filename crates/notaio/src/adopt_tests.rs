// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use std::collections::BTreeMap;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

use super::*;

fn cm(labels: &[(&str, &str)]) -> ConfigMap {
    ConfigMap {
        metadata: ObjectMeta {
            name: Some("ca-bundle".into()),
            labels: (!labels.is_empty()).then(|| {
                labels
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<BTreeMap<_, _>>()
            }),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn a_free_name_is_not_foreign() {
    assert!(!is_foreign(None));
}

#[test]
fn a_configmap_notaio_published_is_not_foreign() {
    assert!(!is_foreign(Some(&cm(&[(MANAGED_BY_LABEL, MANAGED_BY)]))));
    assert!(!is_foreign(Some(&cm(&[
        ("other", "x"),
        (MANAGED_BY_LABEL, MANAGED_BY)
    ]))));
}

/// The case the rule exists for: somebody else's ConfigMap under the derived name.
#[test]
fn a_configmap_without_the_label_is_foreign() {
    assert!(is_foreign(Some(&cm(&[]))));
    assert!(is_foreign(Some(&cm(&[("app", "trust-manager")]))));
}

#[test]
fn a_configmap_managed_by_something_else_is_foreign() {
    assert!(is_foreign(Some(&cm(&[(MANAGED_BY_LABEL, "Helm")]))));
    assert!(is_foreign(Some(&cm(&[(MANAGED_BY_LABEL, "")]))));
}

#[test]
fn the_message_names_the_configmap_and_the_label() {
    let m = name_taken_message("ca-bundle");
    assert!(
        m.contains("ca-bundle") && m.contains(MANAGED_BY_LABEL),
        "{m}"
    );
}
