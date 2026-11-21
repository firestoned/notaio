// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! The consumer's persisted version floor (ADR-0008): the highest bundle version accepted per
//! policy, remembered across restarts, so a consumer cannot be handed an older, looser bundle that
//! is still signed and still valid (threat model T5.4).
//!
//! The floor is kept per policy namespace and name, with the `uid` of the incarnation it holds.
//! A recreated policy has a new `uid` and restarts at version 1: it is accepted if it was issued
//! after the bundle the floor holds, and a replay from the older incarnation is refused.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::{
    verify_audience_bundle, verify_bundle, AudienceBundlePayload, BundleError, BundleMeta,
    BundlePayload, Envelope, TrustedKeys, VerifyOptions,
};

/// Schema of the file a [`FileFloor`] writes.
const FILE_SCHEMA: &str = "sandbox.firestoned.io/floor/v1";

/// What the floor holds for one policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloorEntry {
    /// The incarnation of the policy the floor belongs to.
    pub uid: String,
    /// The highest version accepted for that incarnation.
    pub version: u64,
    /// `issuedAt` of the latest bundle accepted, to order incarnations.
    pub issued_at: Timestamp,
}

/// Where a consumer keeps its floor.
pub trait VersionFloor {
    /// The floor for a policy, if one was ever recorded.
    fn get(&self, namespace: &str, name: &str) -> Option<FloorEntry>;

    /// Record a new floor durably.
    ///
    /// # Errors
    /// `BundleError::Floor` when it cannot be stored. [`accept`] then refuses the bundle.
    fn set(&mut self, namespace: &str, name: &str, entry: FloorEntry) -> Result<(), BundleError>;
}

fn key(namespace: &str, name: &str) -> String {
    format!("{namespace}/{name}")
}

/// A floor in memory, for tests and for consumers that persist it some other way.
#[derive(Debug, Default)]
pub struct MemoryFloor {
    entries: BTreeMap<String, FloorEntry>,
}

impl VersionFloor for MemoryFloor {
    fn get(&self, namespace: &str, name: &str) -> Option<FloorEntry> {
        self.entries.get(&key(namespace, name)).cloned()
    }

    fn set(&mut self, namespace: &str, name: &str, entry: FloorEntry) -> Result<(), BundleError> {
        self.entries.insert(key(namespace, name), entry);
        Ok(())
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct FloorFile {
    schema: String,
    entries: BTreeMap<String, FloorEntry>,
}

/// A floor in one JSON file. Each write goes to a temporary file in the same directory, is
/// synced, and is renamed over the old file, so a crash leaves either the old floor or the new one.
#[derive(Debug)]
pub struct FileFloor {
    path: PathBuf,
    entries: BTreeMap<String, FloorEntry>,
}

impl FileFloor {
    /// Opens the floor at `path`. A missing file is an empty floor.
    ///
    /// # Errors
    /// `BundleError::Floor` for a file that cannot be read or does not parse. A damaged floor is
    /// never treated as an empty one, which would accept any old bundle.
    pub fn open(path: &Path) -> Result<Self, BundleError> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(FileFloor {
                    path: path.to_path_buf(),
                    entries: BTreeMap::new(),
                });
            }
            Err(e) => return Err(floor_err(path, &e)),
        };
        let file: FloorFile = serde_json::from_str(&text)
            .map_err(|e| BundleError::Floor(format!("{}: {e}", path.display())))?;
        if file.schema != FILE_SCHEMA {
            return Err(BundleError::Floor(format!(
                "{}: unsupported schema {:?}",
                path.display(),
                file.schema
            )));
        }
        Ok(FileFloor {
            path: path.to_path_buf(),
            entries: file.entries,
        })
    }

    fn write(&self, entries: &BTreeMap<String, FloorEntry>) -> Result<(), BundleError> {
        let file = FloorFile {
            schema: FILE_SCHEMA.to_string(),
            entries: entries.clone(),
        };
        let bytes =
            serde_json::to_vec_pretty(&file).map_err(|e| BundleError::Floor(e.to_string()))?;
        let dir = self
            .path
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let tmp = dir.join(format!(".{name}.tmp-{}", std::process::id()));
        let result = (|| {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
            std::fs::rename(&tmp, &self.path)?;
            // Make the rename itself durable.
            std::fs::File::open(dir)?.sync_all()
        })();
        if let Err(e) = result {
            let _ = std::fs::remove_file(&tmp);
            return Err(floor_err(&self.path, &e));
        }
        Ok(())
    }
}

fn floor_err(path: &Path, e: &std::io::Error) -> BundleError {
    BundleError::Floor(format!("{}: {e}", path.display()))
}

impl VersionFloor for FileFloor {
    fn get(&self, namespace: &str, name: &str) -> Option<FloorEntry> {
        self.entries.get(&key(namespace, name)).cloned()
    }

    fn set(&mut self, namespace: &str, name: &str, entry: FloorEntry) -> Result<(), BundleError> {
        let mut next = self.entries.clone();
        next.insert(key(namespace, name), entry);
        // Only adopt the new floor once it is on disk.
        self.write(&next)?;
        self.entries = next;
        Ok(())
    }
}

/// The floor decision alone (ADR-0008, decisions 2 and 3): given what the floor holds for the
/// payload's policy, the entry to record, or why the payload is refused. Pure, so a consumer whose
/// floor is not a [`VersionFloor`] (an async store, a database transaction that reads the held
/// row and writes the next one) applies exactly the rule [`accept`] applies.
///
/// `payload` must come from [`verify_bundle`]: this checks the floor, nothing else. Use the entry
/// only after it is stored durably (decision 4).
///
/// # Errors
/// - `BundleError::Downgrade` for a lower version of the incarnation the floor holds;
/// - `BundleError::StaleIncarnation` for a different incarnation issued no later than the one
///   the floor holds (a replay from before a recreation).
pub fn next_floor(
    held: Option<&FloorEntry>,
    payload: &BundlePayload,
) -> Result<FloorEntry, BundleError> {
    next_floor_for(held, &payload.content.policy.uid, &payload.meta)
}

/// [`next_floor`] for an audience bundle (ADR-0020, decision 9): the same rule, on the audience's
/// incarnation. `payload` must come from [`verify_audience_bundle`].
///
/// # Errors
/// As [`next_floor`].
pub fn next_audience_floor(
    held: Option<&FloorEntry>,
    payload: &AudienceBundlePayload,
) -> Result<FloorEntry, BundleError> {
    next_floor_for(held, &payload.content.audience.uid, &payload.meta)
}

/// The floor rule on an incarnation `uid` and a bundle's `meta`, whatever the bundle kind.
fn next_floor_for(
    held: Option<&FloorEntry>,
    uid: &str,
    meta: &BundleMeta,
) -> Result<FloorEntry, BundleError> {
    match held {
        None => Ok(FloorEntry {
            uid: uid.to_string(),
            version: meta.version,
            issued_at: meta.issued_at,
        }),
        Some(held) if held.uid == uid => {
            if meta.version < held.version {
                return Err(BundleError::Downgrade {
                    got: meta.version,
                    min: held.version,
                });
            }
            Ok(FloorEntry {
                uid: held.uid.clone(),
                version: meta.version,
                issued_at: meta.issued_at.max(held.issued_at),
            })
        }
        Some(held) => {
            if meta.issued_at <= held.issued_at {
                return Err(BundleError::StaleIncarnation {
                    uid: uid.to_string(),
                });
            }
            Ok(FloorEntry {
                uid: uid.to_string(),
                version: meta.version,
                issued_at: meta.issued_at,
            })
        }
    }
}

/// Verifies `env`, checks it against the floor, records the new floor durably, and only then
/// returns the payload (ADR-0008, decision 4).
///
/// The bundle is whichever policy its signed content names, not the one the caller went looking
/// for: compare `payload.content.policy` with what was asked for, and refuse a mismatch. Whoever
/// can write the ConfigMap a bundle travels in can put another policy's valid bundle there.
///
/// # Errors
/// - any error from [`verify_bundle`]: the floor is not touched;
/// - any error from [`next_floor`];
/// - `BundleError::Floor` when the new floor cannot be stored: the bundle is refused.
pub fn accept(
    env: &Envelope,
    keys: &TrustedKeys,
    now: Timestamp,
    floor: &mut dyn VersionFloor,
) -> Result<BundlePayload, BundleError> {
    let payload = verify_bundle(env, keys, &VerifyOptions::at(now))?;
    let policy = &payload.content.policy;
    let next = next_floor(
        floor.get(&policy.namespace, &policy.name).as_ref(),
        &payload,
    )?;
    floor.set(&policy.namespace, &policy.name, next)?;
    Ok(payload)
}

/// The floor name of an audience. A Kubernetes name never holds `/`, so an audience's entry can
/// never be a policy's, even when a consumer keeps both kinds in one store.
pub fn audience_floor_name(name: &str) -> String {
    format!("{AUDIENCE_FLOOR_PREFIX}{name}")
}

/// Prefix of an audience's name in a floor (see [`audience_floor_name`]).
const AUDIENCE_FLOOR_PREFIX: &str = "audience/";

/// [`accept`] for an audience bundle: verifies `env`, checks it against the audience's floor
/// (per namespace and name, ADR-0008), records the new floor durably, and only then returns the
/// payload.
///
/// # Errors
/// As [`accept`], with [`verify_audience_bundle`] in place of [`verify_bundle`].
pub fn accept_audience(
    env: &Envelope,
    keys: &TrustedKeys,
    now: Timestamp,
    floor: &mut dyn VersionFloor,
) -> Result<AudienceBundlePayload, BundleError> {
    let payload = verify_audience_bundle(env, keys, &VerifyOptions::at(now))?;
    let audience = &payload.content.audience;
    let name = audience_floor_name(&audience.name);
    let next = next_audience_floor(floor.get(&audience.namespace, &name).as_ref(), &payload)?;
    floor.set(&audience.namespace, &name, next)?;
    Ok(payload)
}
