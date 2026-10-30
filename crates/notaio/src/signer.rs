// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;

use anyhow::Context as _;
use notaio::openbao::{choose_signer, SignerChoice, TransitSigner};
use sandboxpolicy_bundle::{
    envelope, sign_bundle, signing_input, BundlePayload, Ed25519Signer, Envelope, Signer as _,
};

use crate::error::Error;

/// The key that signs bundles (ADR-0004). Unsigned bundles are never published: with no signer the
/// controller validates and reports, and publishes nothing.
pub enum BundleSigner {
    /// A seed on disk, opt-in for local experiments only. Anyone who can read the Secret can sign,
    /// which the threat model forbids for production (T8.2).
    Development(Ed25519Signer),
    /// A non-exportable OpenBao transit key: notaio may ask for signatures, never hold the key.
    OpenBao(TransitSigner),
}

pub type SharedSigner = Arc<BundleSigner>;

impl BundleSigner {
    /// The signed envelope for `payload`.
    ///
    /// # Errors
    /// `Error::Bundle` if the payload cannot be serialised, `Error::Signing` if OpenBao does not
    /// sign. Nothing is published on either.
    pub async fn sign(&self, payload: &BundlePayload) -> Result<Envelope, Error> {
        match self {
            BundleSigner::Development(s) => Ok(sign_bundle(payload, s)?),
            BundleSigner::OpenBao(t) => {
                let input = signing_input(payload)?;
                let (key_id, sig) = t.sign(&input.message).await?;
                Ok(envelope(&input.payload, &key_id, &sig))
            }
        }
    }

    /// For the startup log.
    pub fn describe(&self) -> String {
        match self {
            BundleSigner::Development(s) => format!("development key {:?}", s.key_id()),
            BundleSigner::OpenBao(t) => match t.key_version() {
                Some(v) => format!("OpenBao transit key {:?}, pinned to version {v}", t.key()),
                None => format!("OpenBao transit key {:?}, latest version", t.key()),
            },
        }
    }
}

/// The signer the environment configures (`notaio::openbao::choose_signer`).
///
/// # Errors
/// A configuration that names both signers, an OpenBao address without `https://` or a CA, or a
/// development seed that cannot be read.
pub fn from_env() -> anyhow::Result<Option<SharedSigner>> {
    match choose_signer(|name| std::env::var(name).ok())? {
        SignerChoice::None => Ok(None),
        SignerChoice::Development { key_file, key_id } => {
            let raw = std::fs::read_to_string(&key_file)
                .with_context(|| format!("reading {}", key_file.display()))?;
            let signer = Ed25519Signer::from_seed_b64(&key_id, &raw)?;
            Ok(Some(Arc::new(BundleSigner::Development(signer))))
        }
        SignerChoice::OpenBao(cfg) => Ok(Some(Arc::new(BundleSigner::OpenBao(
            TransitSigner::new(cfg)?,
        )))),
    }
}
