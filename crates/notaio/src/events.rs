//! Which Kubernetes Events to record (ADR-0010, decision 5).
//!
//! Events mark transitions, never steady state: a reconcile that changes nothing records nothing.
//! These functions decide; the controller records what they return, and a failure to record is
//! logged, never fatal.

use sandboxpolicy_api::{Condition, GrantPhase, SandboxPolicyStatus};

/// The `action` of every Event notaio records.
pub const ACTION: &str = "Reconcile";

/// The API server refuses an Event whose note is longer than this, in bytes.
pub const MAX_NOTE: usize = 1024;

/// `s` cut to at most `MAX_NOTE` bytes on a character boundary, marked when cut.
pub fn fit(s: &str) -> String {
    if s.len() <= MAX_NOTE {
        return s.to_string();
    }
    const MARK: &str = " [truncated]";
    let mut end = MAX_NOTE - MARK.len();
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{MARK}", &s[..end])
}

/// One Event to record on an object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub warning: bool,
    pub reason: String,
    pub note: String,
}

fn ready(conditions: &[Condition]) -> Option<&Condition> {
    conditions.iter().find(|c| c.type_ == "Ready")
}

/// The Event for a policy whose status goes from `prev` to `next`, if any.
///
/// A new bundle version is `Published`. A `Ready` condition that turns false, or stays false with
/// a new reason, is a warning with that reason. Anything else, including the periodic refresh of
/// an unchanged bundle, records nothing.
pub fn policy(prev: &SandboxPolicyStatus, next: &SandboxPolicyStatus) -> Option<Note> {
    let before = ready(&prev.conditions);
    let after = ready(&next.conditions)?;
    if after.status == "True" {
        let version = next.bundle_version?;
        if prev.bundle_version == Some(version) {
            return None;
        }
        return Some(Note {
            warning: false,
            reason: "Published".to_string(),
            note: format!("signed bundle version {version} published"),
        });
    }
    if before.is_some_and(|b| b.status == after.status && b.reason == after.reason) {
        return None;
    }
    Some(Note {
        warning: true,
        reason: after.reason.clone(),
        note: fit(&after.message),
    })
}

/// The Event for a grant whose phase goes from `prev` to `next`, if any: it lapsed, or it was
/// refused.
pub fn grant(prev: GrantPhase, next: GrantPhase, message: &str) -> Option<Note> {
    if prev == next {
        return None;
    }
    let (warning, reason) = match next {
        GrantPhase::Expired => (false, "Expired"),
        GrantPhase::Rejected => (true, "Rejected"),
        GrantPhase::Pending | GrantPhase::Active => return None,
    };
    Some(Note {
        warning,
        reason: reason.to_string(),
        note: fit(message),
    })
}

/// The Event for a profile whose `Valid` condition goes from `prev` to `next`: only when it
/// becomes invalid, or its findings change while invalid.
pub fn profile(prev: &[Condition], next: &[Condition]) -> Option<Note> {
    let valid = |c: &[Condition]| c.iter().find(|c| c.type_ == "Valid").cloned();
    let after = valid(next)?;
    if after.status == "True" {
        return None;
    }
    if valid(prev).is_some_and(|b| b.status == after.status && b.message == after.message) {
        return None;
    }
    Some(Note {
        warning: true,
        reason: after.reason,
        note: fit(&after.message),
    })
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
