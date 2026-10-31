// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! How loudly to log what a controller's stream yields as an error. Most of it is routine: the
//! watcher recovers by itself, or the reconciler's error policy has already logged it.

use kube::runtime::{controller, watcher};

/// Whether a controller stream error deserves a warning. False for what needs nobody's attention:
/// - a watch that expired (`410 Gone`, "too old resource version"): the watcher re-lists;
/// - a scheduled reconcile of an object deleted meanwhile;
/// - a failed reconcile, which `error_policy` has already logged and counted.
pub fn deserves_warning<R>(err: &controller::Error<R, watcher::Error>) -> bool {
    match err {
        controller::Error::QueueError(watcher::Error::WatchError(status)) => status.code != 410,
        controller::Error::ObjectNotFound(_) | controller::Error::ReconcilerFailed(..) => false,
        _ => true,
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
