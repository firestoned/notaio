use chrono::TimeZone;

use super::*;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0)
        .single()
        .unwrap()
}

fn held_by(holder: Option<&str>, renewed_ago: Option<i64>) -> Held {
    Held {
        holder: holder.map(str::to_string),
        renewed: renewed_ago.map(|s| now() - Duration::seconds(s)),
        duration: Duration::seconds(LEASE_SECONDS),
    }
}

#[test]
fn no_lease_is_created() {
    assert_eq!(decide(None, "a", now()), Step::Create);
}

#[test]
fn the_holder_renews_even_when_late() {
    // Renewing an expired Lease we still hold is fine: nobody else has taken it.
    assert_eq!(
        decide(Some(&held_by(Some("a"), Some(1))), "a", now()),
        Step::Renew
    );
    assert_eq!(
        decide(Some(&held_by(Some("a"), Some(60))), "a", now()),
        Step::Renew
    );
}

#[test]
fn a_live_lease_held_by_another_is_waited_on() {
    assert_eq!(
        decide(Some(&held_by(Some("b"), Some(14))), "a", now()),
        Step::Wait { holder: "b".into() }
    );
}

#[test]
fn an_expired_lease_is_taken_over_at_exactly_its_end() {
    assert_eq!(
        decide(Some(&held_by(Some("b"), Some(LEASE_SECONDS))), "a", now()),
        Step::TakeOver
    );
    assert_eq!(
        decide(Some(&held_by(Some("b"), Some(600))), "a", now()),
        Step::TakeOver
    );
}

#[test]
fn a_released_or_never_renewed_lease_is_taken_over() {
    assert_eq!(
        decide(Some(&held_by(None, Some(1))), "a", now()),
        Step::TakeOver
    );
    assert_eq!(
        decide(Some(&held_by(Some(""), Some(1))), "a", now()),
        Step::TakeOver
    );
    assert_eq!(
        decide(Some(&held_by(Some("b"), None)), "a", now()),
        Step::TakeOver
    );
}

#[test]
fn the_leader_gives_up_before_its_lease_can_expire() {
    // ADR-0011, decision 3: a follower can only take over after LEASE_SECONDS, and the leader stops
    // at RENEW_DEADLINE, so on agreeing clocks the two never overlap.
    assert!(RENEW_DEADLINE < Duration::seconds(LEASE_SECONDS));
    assert!(chrono::Duration::from_std(RENEW_EVERY).unwrap() * 2 <= RENEW_DEADLINE);
}

#[test]
fn a_lease_is_read_back_as_written() {
    let t = Timestamp::from_millisecond(now().timestamp_millis()).unwrap();
    let lease = Lease {
        metadata: ObjectMeta::default(),
        spec: Some(LeaseSpec {
            holder_identity: Some("notaio-abc".into()),
            renew_time: Some(MicroTime(t)),
            lease_duration_seconds: Some(15),
            ..Default::default()
        }),
    };
    assert_eq!(
        held(&lease),
        Held {
            holder: Some("notaio-abc".into()),
            renewed: Some(now()),
            duration: Duration::seconds(15),
        }
    );
    // A Lease written by something else, with no duration, gets ours.
    let bare = Lease::default();
    assert_eq!(held(&bare).duration, Duration::seconds(LEASE_SECONDS));
}
