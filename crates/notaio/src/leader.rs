//! Leader election on a Kubernetes Lease (ADR-0011).
//!
//! Every replica serves health and metrics; only the one holding the Lease reconciles, signs and
//! publishes. Whether to create, renew, take over or wait is `decide`, a pure function of the
//! Lease, this replica's identity and `now`. Every write carries the Lease's `resourceVersion`, so
//! when two replicas race for an expired Lease, the API server lets exactly one win.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use k8s_openapi::api::coordination::v1::{Lease, LeaseSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::MicroTime;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::jiff::Timestamp;
use kube::api::{Api, PostParams};
use kube::Client;

/// The Lease's name, in the controller's namespace.
pub const LEASE_NAME: &str = "notaio";
/// How long a Lease is valid without a renewal.
pub const LEASE_SECONDS: i64 = 15;
/// How often the leader renews.
pub const RENEW_EVERY: std::time::Duration = std::time::Duration::from_secs(5);
/// A leader that has not renewed for this long gives up, before its Lease can expire.
pub const RENEW_DEADLINE: Duration = Duration::seconds(10);
/// How often a follower checks whether the Lease is free.
pub const RETRY_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

/// What a Lease says, as `decide` needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub holder: Option<String>,
    pub renewed: Option<DateTime<Utc>>,
    pub duration: Duration,
}

/// What a replica does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// There is no Lease: create it, holding it.
    Create,
    /// This replica holds it: renew.
    Renew,
    /// Nobody holds it, or the holder let it expire: take it over.
    TakeOver,
    /// Another replica holds a live Lease.
    Wait { holder: String },
}

/// The next step for replica `me`, given the current Lease (None when there is none).
pub fn decide(current: Option<&Held>, me: &str, now: DateTime<Utc>) -> Step {
    let Some(h) = current else {
        return Step::Create;
    };
    match h.holder.as_deref() {
        Some(holder) if holder == me => Step::Renew,
        Some(holder) if !holder.is_empty() => {
            // A Lease with no renew time has never been renewed: treat it as expired.
            let live = h.renewed.is_some_and(|r| now < r + h.duration);
            if live {
                Step::Wait {
                    holder: holder.to_string(),
                }
            } else {
                Step::TakeOver
            }
        }
        _ => Step::TakeOver,
    }
}

fn to_chrono(t: &MicroTime) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp_millis(t.0.as_millisecond())
}

fn to_micro(t: DateTime<Utc>) -> Option<MicroTime> {
    Timestamp::from_millisecond(t.timestamp_millis())
        .ok()
        .map(MicroTime)
}

/// The fields of `lease` that `decide` reads.
pub fn held(lease: &Lease) -> Held {
    let spec = lease.spec.clone().unwrap_or_default();
    Held {
        holder: spec.holder_identity,
        renewed: spec.renew_time.as_ref().and_then(to_chrono),
        duration: Duration::seconds(i64::from(
            spec.lease_duration_seconds.unwrap_or(LEASE_SECONDS as i32),
        )),
    }
}

fn is_conflict(e: &kube::Error) -> bool {
    matches!(e, kube::Error::Api(s) if s.code == 409)
}

/// Contends for, holds and releases the Lease for one replica.
pub struct Elector {
    api: Api<Lease>,
    me: String,
    leading: Arc<AtomicBool>,
}

impl Elector {
    /// An elector for replica `me` (its pod name) on the Lease in `namespace`.
    pub fn new(client: Client, namespace: &str, me: &str) -> Self {
        Elector {
            api: Api::namespaced(client, namespace),
            me: me.to_string(),
            leading: Arc::new(AtomicBool::new(false)),
        }
    }

    /// True while this replica holds the Lease. Shared with the reconcilers and the metrics.
    pub fn leading(&self) -> Arc<AtomicBool> {
        self.leading.clone()
    }

    /// Waits until this replica holds the Lease.
    pub async fn acquire(&self) {
        loop {
            match self.try_once().await {
                Ok(true) => {
                    self.leading.store(true, Ordering::Release);
                    tracing::info!(identity = %self.me, "acquired the Lease: leading");
                    return;
                }
                Ok(false) => {}
                Err(e) => tracing::warn!(error = %e, "Lease check failed"),
            }
            tokio::time::sleep(RETRY_EVERY).await;
        }
    }

    /// Keeps the Lease renewed. Returns only when it is lost, with the reason; the caller then
    /// stops leading (ADR-0011, decision 4).
    pub async fn hold(&self) -> String {
        let mut last = Utc::now();
        loop {
            tokio::time::sleep(RENEW_EVERY).await;
            match self.try_once().await {
                Ok(true) => last = Utc::now(),
                Ok(false) => {
                    self.leading.store(false, Ordering::Release);
                    return "another replica holds the Lease".to_string();
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Lease renewal failed");
                    if Utc::now() - last >= RENEW_DEADLINE {
                        self.leading.store(false, Ordering::Release);
                        return format!("could not renew the Lease for {RENEW_DEADLINE}: {e}");
                    }
                }
            }
        }
    }

    /// Gives the Lease up, so a follower takes over at once. Best effort: on failure the Lease
    /// simply expires.
    pub async fn release(&self) {
        self.leading.store(false, Ordering::Release);
        let Ok(Some(mut lease)) = self.api.get_opt(LEASE_NAME).await else {
            return;
        };
        let spec = lease.spec.get_or_insert_with(LeaseSpec::default);
        if spec.holder_identity.as_deref() != Some(self.me.as_str()) {
            return;
        }
        spec.holder_identity = None;
        if let Err(e) = self
            .api
            .replace(LEASE_NAME, &PostParams::default(), &lease)
            .await
        {
            tracing::warn!(error = %e, "could not release the Lease; it will expire");
        } else {
            tracing::info!("released the Lease");
        }
    }

    /// One round of `decide` and its write. True when this replica holds the Lease after it.
    async fn try_once(&self) -> Result<bool, kube::Error> {
        let now = Utc::now();
        let current = self.api.get_opt(LEASE_NAME).await?;
        let step = decide(current.as_ref().map(held).as_ref(), &self.me, now);
        let stamp = to_micro(now);
        let result = match (step, current) {
            (Step::Wait { .. }, _) => return Ok(false),
            (Step::Create, _) => {
                let lease = Lease {
                    metadata: ObjectMeta {
                        name: Some(LEASE_NAME.to_string()),
                        ..Default::default()
                    },
                    spec: Some(LeaseSpec {
                        holder_identity: Some(self.me.clone()),
                        acquire_time: stamp.clone(),
                        renew_time: stamp,
                        lease_duration_seconds: Some(LEASE_SECONDS as i32),
                        lease_transitions: Some(0),
                        ..Default::default()
                    }),
                };
                self.api.create(&PostParams::default(), &lease).await
            }
            (Step::Renew, Some(mut lease)) => {
                let spec = lease.spec.get_or_insert_with(LeaseSpec::default);
                spec.renew_time = stamp;
                spec.lease_duration_seconds = Some(LEASE_SECONDS as i32);
                self.api
                    .replace(LEASE_NAME, &PostParams::default(), &lease)
                    .await
            }
            (Step::TakeOver, Some(mut lease)) => {
                let spec = lease.spec.get_or_insert_with(LeaseSpec::default);
                spec.holder_identity = Some(self.me.clone());
                spec.acquire_time = stamp.clone();
                spec.renew_time = stamp;
                spec.lease_duration_seconds = Some(LEASE_SECONDS as i32);
                spec.lease_transitions = Some(spec.lease_transitions.unwrap_or(0) + 1);
                self.api
                    .replace(LEASE_NAME, &PostParams::default(), &lease)
                    .await
            }
            // Renew or take over without a Lease cannot come out of `decide`.
            (_, None) => return Ok(false),
        };
        match result {
            Ok(_) => Ok(true),
            // Another replica wrote first: it won this round.
            Err(e) if is_conflict(&e) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
#[path = "leader_tests.rs"]
mod tests;
