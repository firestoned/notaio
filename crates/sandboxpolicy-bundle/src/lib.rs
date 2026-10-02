//! The signed policy bundle: the contract between notaio (producer) and mediatore,
//! mediatore-guest and the gateway (consumers).
//!
//! Design rules, see ADR-0002:
//! - Content is canonical JSON: struct field order is fixed, maps are `BTreeMap`, lists are sorted
//!   by the compiler, and there are no floats. The digest is SHA-256 over those bytes.
//! - The version is monotonic and bumps only when the content digest changes. Validity (`issuedAt`,
//!   `notAfter`) refreshes without a version bump.
//! - Bundles are wrapped in a DSSE envelope. A consumer verifies the signature, the schema, the
//!   content digest, the validity window and that the version never goes backwards. Anything else
//!   fails closed.
//! - This crate has no Kubernetes dependency, so `mediatore-guest` can verify a bundle without one.

mod content;
mod dsse;
mod error;
mod verify;

pub use content::{BundleContent, BundleMeta, BundlePayload, PolicyRef, ProfileRef, SCHEMA};
pub use dsse::{sign_bundle, Ed25519Signer, Envelope, Signature, Signer, PAYLOAD_TYPE};
pub use error::BundleError;
pub use verify::{verify_bundle, TrustedKeys, VerifyOptions};
