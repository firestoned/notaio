// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! Pure data types shared by the SandboxPolicy CRDs, the compiler and the signed bundle.
//!
//! This crate deliberately has no Kubernetes dependency. `mediatore-guest` runs as root in every
//! sandbox VM and must be able to verify and read a bundle without pulling in a cluster client.

mod audience;
mod knob;
mod parts;

pub use audience::{
    AudienceCredential, AudienceDefinition, Backend, ConfigMapKeyRef, CredentialFormat,
    CredentialSource, DelegatedRefresh, HeaderPair, OpenBaoSource, Placement, SecretRef,
};

pub use knob::{KnobId, Role, Step};
pub use parts::{
    Access, AudienceGrant, Budgets, DataScope, EgressRule, EndpointRule, Lease, ToolRef,
};
