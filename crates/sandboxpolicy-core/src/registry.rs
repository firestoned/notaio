//! The knob register, transcribed from section 9 of the AgentSandbox threat model.
//!
//! `max` is the loosest step that exists for the knob. `approvers` are the roles that must all
//! approve a relaxation of it. The threat model is the source of truth: when it changes, this file
//! changes in the same pull request, and `registry_matches_profiles` in the tests keeps the built-in
//! profile matrix honest against it.

use sandboxpolicy_types::{Role, Step};

#[derive(Debug, Clone, Copy)]
pub struct KnobDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Trust boundary, 1 to 8.
    pub boundary: u8,
    pub max: Step,
    pub approvers: &'static [Role],
}

use Role::*;
use Step::*;

const fn k(
    id: &'static str,
    name: &'static str,
    boundary: u8,
    max: Step,
    approvers: &'static [Role],
) -> KnobDef {
    KnobDef {
        id,
        name,
        boundary,
        max,
        approvers,
    }
}

pub static REGISTRY: &[KnobDef] = &[
    k("K1.1", "Web and document fetching", 1, R3, &[Security]),
    k(
        "K1.2",
        "Project instruction loading",
        1,
        R2,
        &[PlatformOwner],
    ),
    k("K1.3", "Tool and MCP set", 1, R3, &[Security, ToolOwner]),
    k("K1.4", "Output export", 1, R3, &[DataOwner]),
    k("K2.1", "Command execution", 2, R3, &[Security]),
    k("K2.2", "Filesystem scope", 2, R2, &[DataOwner]),
    k("K2.3", "Package installation", 2, R2, &[PlatformOwner]),
    k("K2.4", "Resource limits", 2, R2, &[PlatformOwner]),
    k(
        "K2.5",
        "Command jail mode",
        2,
        R2,
        &[PlatformOwner, Security],
    ),
    k("K3.1", "Credential delivery", 3, R2, &[Security]),
    k("K3.2", "Approval mode", 3, R3, &[PlatformOwner, Security]),
    k("K3.3", "Guard strictness", 3, R2, &[Security]),
    k("K4.1", "Egress mode", 4, R3, &[Security]),
    k("K4.2", "DNS", 4, R2, &[Security]),
    k("K4.3", "Egress volume caps", 4, R2, &[Security]),
    k("K4.4", "TLS inspection", 4, R1, &[Security]),
    k("K4.5", "Inbound exposure", 4, R2, &[Security]),
    k("K5.1", "Token TTL", 5, R2, &[Security]),
    k("K5.2", "Token scope", 5, R3, &[DataOwner, Security]),
    k("K5.3", "Audience set", 5, R2, &[DataOwner]),
    k("K5.4", "Sender constraint", 5, R2, &[Security]),
    k("K5.5", "Renewal re-evaluation", 5, R1, &[PlatformOwner]),
    k("K5.6", "Identity type", 5, R2, &[PlatformOwner, Security]),
    k("K6.1", "Tenancy tier", 6, R2, &[PlatformOwner, Security]),
    k(
        "K6.2",
        "Virtual hardware and guest tools",
        6,
        R1,
        &[Security],
    ),
    k("K6.3", "Attestation strictness", 6, R1, &[Security]),
    k("K7.1", "Authentication strength", 7, R2, &[Security]),
    k("K7.2", "Profile eligibility", 7, R2, &[DataOwner]),
    k(
        "K7.3",
        "Lease duration and concurrency",
        7,
        R2,
        &[PlatformOwner],
    ),
    k(
        "K7.4",
        "Unattended and scheduled runs",
        7,
        R3,
        &[PlatformOwner, Security, RiskOwner],
    ),
    k("K8.1", "Image source", 8, R2, &[Security]),
    k("K8.2", "Policy change control", 8, R1, &[Security]),
    k(
        "K8.3",
        "Logging and retention",
        8,
        R1,
        &[Security, Compliance],
    ),
    k("K8.4", "Break-glass access", 8, R1, &[Security]),
];

pub fn knob(id: &str) -> Option<&'static KnobDef> {
    REGISTRY.iter().find(|k| k.id == id)
}

/// Knobs that are only valid on `LabOnly` profiles: "Lab-only by definition" in the threat model.
pub const LAB_ONLY_KNOBS: &[&str] = &["K6.3", "K8.2"];

/// Maximum lifetime of a relaxation grant by target step, from the register's "How knobs work".
pub fn max_grant_lifetime_days(step: Step) -> i64 {
    match step {
        R0 => 0,
        R1 => 90,
        R2 => 30,
        R3 => 7,
    }
}
