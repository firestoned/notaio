// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! When a published bundle is re-signed. A bundle is refreshed once half its lifetime is left
//! (ADR-0018, decision 1), so the signer's outage budget is the other half.

use jiff::{SignedDuration, Timestamp};

/// Added to the wait for a bundle to turn stale, so the reconcile that wakes up finds it stale
/// rather than a moment short of it.
pub const RECHECK_SLACK: SignedDuration = SignedDuration::from_secs(1);

/// The shortest wait before checking a policy again, so a bundle that is already stale (which
/// the reconcile re-signs, so it should not happen) cannot spin the controller.
pub const MIN_RECHECK: SignedDuration = SignedDuration::from_secs(1);

/// True when a bundle valid until `not_after` is due for re-signing at `now`: no bundle yet, or
/// half the lifetime or less left.
pub fn is_stale(not_after: Option<Timestamp>, lifetime: SignedDuration, now: Timestamp) -> bool {
    not_after.is_none_or(|t| t <= now + lifetime / 2)
}

/// How long to wait before reconciling an unchanged policy again: the evaluator's `requeue`
/// (half the lifetime, or sooner for a grant expiry), or the moment the bundle turns stale,
/// whichever is first.
///
/// Waiting only for `requeue` lets the refresh slip to almost `notAfter`: a reconcile just before
/// the bundle turns stale finds it fresh, and the next comes half a lifetime later. Any watch
/// event, a leader change or a restart puts reconciles at such a moment.
pub fn next_check(
    requeue: SignedDuration,
    not_after: Option<Timestamp>,
    lifetime: SignedDuration,
    now: Timestamp,
) -> SignedDuration {
    let Some(not_after) = not_after else {
        return requeue;
    };
    let until_stale = (not_after - lifetime / 2).duration_since(now) + RECHECK_SLACK;
    requeue.min(until_stale).max(MIN_RECHECK)
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod refresh_tests;
