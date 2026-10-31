// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use k8s_openapi::api::core::v1::ConfigMap;
use kube::core::Status;
use kube::runtime::reflector::ObjectRef;

use super::*;

type Error = controller::Error<std::io::Error, watcher::Error>;

fn watch_error(code: u16) -> Error {
    controller::Error::QueueError(watcher::Error::WatchError(
        Status::failure("too old resource version: 1 (2)", "Expired")
            .with_code(code)
            .boxed(),
    ))
}

#[test]
fn an_expired_watch_is_routine() {
    assert!(!deserves_warning(&watch_error(410)));
}

#[test]
fn other_watch_errors_are_warned() {
    assert!(deserves_warning(&watch_error(403)));
    assert!(deserves_warning(&watch_error(500)));
    assert!(deserves_warning(&Error::QueueError(
        watcher::Error::NoResourceVersion
    )));
}

#[test]
fn what_is_handled_elsewhere_is_routine() {
    let gone = ObjectRef::<ConfigMap>::new("gone")
        .within("sandboxes")
        .erase();
    assert!(!deserves_warning(&Error::ObjectNotFound(Box::new(
        gone.clone()
    ))));
    assert!(!deserves_warning(&Error::ReconcilerFailed(
        std::io::Error::other("signing failed"),
        Box::new(gone)
    )));
}
