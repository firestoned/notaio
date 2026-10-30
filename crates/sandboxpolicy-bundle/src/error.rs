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
}
