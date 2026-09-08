use super::*;
use crate::{CanaryEpoch, canary_freshness};

#[test]
fn captured_fresh() {
    let text = heartbeat_text(
        Some(canary_freshness(10_000, CanaryEpoch::parse("9970"), 1800)),
        "2026-09-07",
        1800,
    );
    assert_eq!(text.title, "✅ osquery pipeline healthy · 2026-09-07");
    assert_eq!(
        text.detail,
        "- The root osqueryd daemon produced a scheduled heartbeat canary 30s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear."
    );
}

#[test]
fn captured_past_boundary() {
    let text = heartbeat_text(
        Some(canary_freshness(10_000, CanaryEpoch::parse("8200"), 1800)),
        "2026-09-07",
        1800,
    );
    assert_eq!(text.title, "✅ osquery pipeline healthy · 2026-09-07");
    assert_eq!(
        text.detail,
        "- The root osqueryd daemon produced a scheduled heartbeat canary 1800s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear."
    );
}

#[test]
fn captured_stale() {
    let text = heartbeat_text(
        Some(canary_freshness(10_000, CanaryEpoch::parse("8199"), 1800)),
        "2026-09-07",
        1800,
    );
    assert_eq!(text.title, "⚠️ osquery heartbeat · 2026-09-07");
    assert_eq!(
        text.detail,
        "- osqueryd scheduled heartbeat canary is STALE (last 1801s ago, over 1800s). The root daemon is not producing scheduled results (stopped or wedged). The uptime watchdog pages on this; this note is the silent daily record."
    );
}

#[test]
fn captured_future() {
    let text = heartbeat_text(
        Some(canary_freshness(10_000, CanaryEpoch::parse("10120"), 1800)),
        "2026-09-07",
        1800,
    );
    assert_eq!(text.title, "✅ osquery pipeline healthy · 2026-09-07");
    assert_eq!(
        text.detail,
        "- The root osqueryd daemon produced a scheduled heartbeat canary 0s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear."
    );
}

#[test]
fn captured_future_boundary() {
    let text = heartbeat_text(
        Some(canary_freshness(10_000, CanaryEpoch::parse("11800"), 1800)),
        "2026-09-07",
        1800,
    );
    assert_eq!(text.title, "✅ osquery pipeline healthy · 2026-09-07");
    assert_eq!(
        text.detail,
        "- The root osqueryd daemon produced a scheduled heartbeat canary 0s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear."
    );
}

#[test]
fn captured_implausible() {
    let text = heartbeat_text(
        Some(canary_freshness(10_000, CanaryEpoch::parse("11801"), 1800)),
        "2026-09-07",
        1800,
    );
    assert_eq!(text.title, "⚠️ osquery heartbeat · 2026-09-07");
    assert_eq!(
        text.detail,
        "- osqueryd scheduled heartbeat canary timestamp is IMPLAUSIBLE (1801s in the future, over 1800s). This is clock skew or a bad row, not a trustworthy liveness signal. The uptime watchdog pages on a real outage; this note is the silent daily record."
    );
}

#[test]
fn captured_missing() {
    let text = heartbeat_text(
        Some(canary_freshness(10_000, None, 1800)),
        "2026-09-07",
        1800,
    );
    assert_eq!(text.title, "⚠️ osquery heartbeat · 2026-09-07");
    assert_eq!(
        text.detail,
        "- osqueryd scheduled heartbeat canary is MISSING (no canary snapshot found). The root daemon is not producing scheduled results, or has never run the schedule. The uptime watchdog pages on this; this note is the silent daily record."
    );
}

#[test]
fn captured_clock_unknown() {
    let text = heartbeat_text(None, "2026-09-07", 1800);
    assert_eq!(text.title, "⚠️ osquery heartbeat · time unknown");
    assert_eq!(
        text.detail,
        "- The heartbeat cannot determine the current time (the system clock read failed), so it cannot judge whether osqueryd is producing scheduled results. Treat this as unverified, not healthy. The uptime watchdog pages on a real outage; this note is the silent daily record."
    );
}

#[test]
fn captured_send_failure() {
    let text = heartbeat_text(
        Some(canary_freshness(10_000, CanaryEpoch::parse("9970"), 1800)),
        "2026-09-07",
        1800,
    );
    assert_eq!(text.title, "✅ osquery pipeline healthy · 2026-09-07");
    assert_eq!(
        text.detail,
        "- The root osqueryd daemon produced a scheduled heartbeat canary 30s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear."
    );
}

#[test]
fn captured_invalid_bound() {
    let text = heartbeat_text(
        Some(canary_freshness(10_000, CanaryEpoch::parse("8199"), 1800)),
        "2026-09-07",
        1800,
    );
    assert_eq!(text.title, "⚠️ osquery heartbeat · 2026-09-07");
    assert_eq!(
        text.detail,
        "- osqueryd scheduled heartbeat canary is STALE (last 1801s ago, over 1800s). The root daemon is not producing scheduled results (stopped or wedged). The uptime watchdog pages on this; this note is the silent daily record."
    );
}
