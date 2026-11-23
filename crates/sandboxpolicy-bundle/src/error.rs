// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BundleError {
    #[error("unexpected payload type {0:?}")]
    PayloadType(String),
    #[error("no signature from a trusted key verified")]
    NoValidSignature,
    #[error("payload is not valid base64 or JSON: {0}")]
    Malformed(String),
    #[error("unsupported schema {0:?}")]
    Schema(String),
    #[error("content digest does not match the content")]
    DigestMismatch,
    #[error("bundle expired at {0}")]
    Expired(String),
    #[error("bundle issued in the future: {0}")]
    NotYetValid(String),
    #[error("bundle version {got} is older than the required minimum {min}")]
    Downgrade { got: u64, min: u64 },
    #[error("signing failed: {0}")]
    Signing(String),
    #[error("invalid key material: {0}")]
    Key(String),
    #[error("content has no RFC 8785 canonical form: {0}")]
    NonCanonical(String),
    #[error("bundle is from an older incarnation of the policy (uid {uid}) than the one accepted")]
    StaleIncarnation { uid: String },
    #[error("version floor: {0}")]
    Floor(String),
    /// The signed content names another policy or audience than the one the consumer asked for
    /// (ADR-0008, decision 7): a valid bundle substituted in transit.
    #[error("bundle is for {got}, not {expected}")]
    WrongSubject { expected: String, got: String },
    /// `notAfter` is further from `issuedAt` than the consumer allows (ADR-0002, decision
    /// 5): a bundle signed to outlive the controller's own maximum.
    #[error("bundle lifetime of {lifetime_seconds}s exceeds the maximum of {max_seconds}s")]
    LifetimeTooLong {
        lifetime_seconds: i64,
        max_seconds: i64,
    },
}
