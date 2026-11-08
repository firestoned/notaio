# ADR-0018: Tightening latency: a shorter, bounded bundle lifetime

- **Status:** Proposed
- **Date:** 2026-11-07
- **Deciders:** Erick Bourgeois
- **Related:** Resolves ADR-0002 open decision 3; ADR-0003 (distribution: mediatore watches the
  bundle ConfigMaps), ADR-0008 (the consumer's version floor), ADR-0010 (metrics and alerts),
  ADR-0004 (the signer); `ROADMAPS.md` M4; threat model T5.4

Proposed: the design is here for review before any code. Nothing below is built.

## Context

ADR-0002 left open how long a tightening takes to land, with two options: a shorter bundle
lifetime, or a version floor pushed to mediatore. Since then the consumers have been designed and
partly built, and the question looks different by path:

| What happens to the policy | What notaio does | How long a consumer honours the old bundle |
| --- | --- | --- |
| Tightened (still valid) | signs version N+1 at once | a watching consumer (mediatore, ADR-0003) takes N+1 within seconds and its floor refuses N from then on (ADR-0008, built in mediatore's phase 1). The lifetime does not bound this |
| Turns invalid (a lint error, a ceiling that no longer covers it, `L020`) | publishes nothing new, so version N stops being refreshed | until N's `notAfter`: up to one lifetime |
| Deleted | the ConfigMap is garbage-collected with it (owner reference) | a watching consumer sees the deletion at once; what it then does is its own decision |
| Any of these, for a consumer that does not watch (the guest, phase 3) | | until it is handed a newer bundle, or until `notAfter` |

So the "pushed floor" already exists: the watch is the push, and mediatore's floor is the floor. What
remains bounded only by the lifetime is a policy that stops being valid, and any consumer that does
not watch. The default lifetime is an hour.

Two more facts:

- **The lifetime is also the signer's outage budget.** A bundle is refreshed at half its
  lifetime; a signer that is down from then on lets it expire at the end. With an hour, OpenBao
  can be down for 30 to 60 minutes before sandboxes lose their policy; ADR-0004's static seal
  brings a restarted OpenBao back in seconds, which is what makes a shorter budget affordable.
- **The setting fails silently.** `NOTAIO_BUNDLE_LIFETIME_SECONDS` that does not parse as an
  integer (`15m`) falls back to the default without a word, and zero or a negative value is used
  as given: bundles valid for nothing, or expired before they are issued. And the
  `NotaioBundleNearExpiry` alert in the observability guide compares with a fixed 600 seconds,
  which a lifetime under 20 minutes would keep permanently firing.

## Decision

1. **The default lifetime becomes 15 minutes** (900 seconds), refreshed at half, as now. A policy
   that turns invalid stops being honoured within 15 minutes instead of an hour; the signer's
   outage budget becomes 7.5 to 15 minutes.
2. **The lifetime is bounded, and a bad value stops the controller.** Between 5 minutes and 24
   hours. A value that does not parse or is out of bounds is a startup error naming the variable,
   never a silent fallback. Below 5 minutes, refreshes would dominate the signer's audit log and a
   brief signer restart would already expire bundles; above a day, an invalid policy stays
   honoured long after anyone has stopped watching for it.
3. **No pushed floor beyond the watch.** The tightened path is already immediate for watching
   consumers. Open decision 3 is resolved by decision 1 plus what ADR-0003 and ADR-0008 already
   give. The guest's half is consumer work: mediatore relays the newest bundle it has verified
   (roadmap 00, phase 3).
4. **An invalid policy still publishes nothing.** notaio does not sign a "revoked" or empty bundle
   for it (Alternatives).
5. **Alerts follow the lifetime.** A new gauge, `notaio_bundle_lifetime_seconds`, reports the
   configured lifetime; `NotaioBundleNearExpiry` becomes "under a quarter of the lifetime left",
   which means the refresh at half was missed, whatever the lifetime is. `notaio lint` compiles
   with the controller's default instead of its own copy of the number.

## Consequences

- A policy that turns invalid stops being honoured four times sooner. A grant that is deleted or
  expires is unaffected: that changes the content, so it is the tightened path, immediate.
- Four times as many signatures: one per policy every 7.5 minutes. For a hundred policies, about
  13 a minute, each an OpenBao audit log entry.
- A signer outage now has 7.5 to 15 minutes before sandboxes lose their policy, and the alerts have
  less time to reach someone. `NotaioSigningFailing` (10 minutes) stays inside that budget; sites
  that cannot react that fast set a longer lifetime, which the bounds allow.
- **An upgrade changes behaviour without a manifest change.** Sites that relied on the hour should
  set `NOTAIO_BUNDLE_LIFETIME_SECONDS=3600` in the Deployment; the release notes say so.
- **CALM step deferred:** notaio has no CALM model yet; `docs/architecture.md`'s known gaps say
  the new bound.

## Alternatives considered

- **Keep an hour, rely on the watch.** It covers tightening but leaves an invalid policy honoured for
  up to an hour; the hour was a starting value, not a decision.
- **5 minutes.** Tightening within minutes, but a signer restart or a short network fault would
  expire bundles in the time it takes to notice. 15 is the shortest lifetime whose outage budget a
  person can act within.
- **30 minutes.** Halves the signing and doubles the outage budget, for twice the latency. A
  reasonable site choice; the bounds allow it.
- **Sign a revocation (an empty bundle at N+1) when a policy turns invalid.** Immediate, but a
  mistaken pull request would cut off every running sandbox under that policy at once, and "empty"
  is itself a policy nobody reviewed. The lifetime bound is the deliberate trade.
- **A push channel from notaio to consumers.** A new listener or outbound path on the controller,
  which ADR-0010 and CLAUDE.md rule out, for what the watch already does.
