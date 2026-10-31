// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! CRD types for the SandboxPolicy API, `sandbox.firestoned.io/v1alpha2`.
//!
//! Consumers (banlieue admission, notaio controller, tooling) depend on this crate. Nothing
//! that runs inside a sandbox VM should: use `sandboxpolicy-bundle` there instead.

mod ceiling;
mod grant;
mod policy;
mod profile;
mod status;

pub use ceiling::{CeilingAudience, ExposureCeiling, ExposureCeilingSpec};
pub use grant::{Approval, GrantPhase, KnobGrant, KnobGrantSpec, KnobGrantStatus};
pub use policy::{
    ExposedAudience, MaxExposure, SandboxPolicy, SandboxPolicySpec, SandboxPolicyStatus, Subjects,
};
pub use profile::{SandboxProfile, SandboxProfileSpec, SandboxProfileStatus};
pub use status::Condition;

/// API group of every kind in this crate. One constant so a rename is a one-line change.
pub const GROUP: &str = "sandbox.firestoned.io";
pub const VERSION: &str = "v1alpha2";
