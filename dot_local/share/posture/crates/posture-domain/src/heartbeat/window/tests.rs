use super::*;
use crate::{CanaryEpoch, canary_freshness, heartbeat_text};

#[test]
fn literal_bounds_preserve_valid_octal_text_and_refuse_invalid_or_overflow_values() {
    {
        let window = HeartbeatWindow::from_override(None);
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "✅ osquery pipeline healthy · 2026-09-08",
            "defaults"
        );
        assert_eq!(
            text.detail,
            "- The root osqueryd daemon produced a scheduled heartbeat canary 17s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear.",
            "defaults"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("bad"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "✅ osquery pipeline healthy · 2026-09-08",
            "invalid-bound"
        );
        assert_eq!(
            text.detail,
            "- The root osqueryd daemon produced a scheduled heartbeat canary 17s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear.",
            "invalid-bound"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some(""));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "✅ osquery pipeline healthy · 2026-09-08",
            "empty-bound"
        );
        assert_eq!(
            text.detail,
            "- The root osqueryd daemon produced a scheduled heartbeat canary 17s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear.",
            "empty-bound"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("0"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "⚠️ osquery heartbeat · 2026-09-08",
            "zero-bound"
        );
        assert_eq!(
            text.detail,
            "- osqueryd scheduled heartbeat canary is STALE (last 17s ago, over 0s). The root daemon is not producing scheduled results (stopped or wedged). The uptime watchdog pages on this; this note is the silent daily record.",
            "zero-bound"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("020"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "⚠️ osquery heartbeat · 2026-09-08",
            "octal-bound"
        );
        assert_eq!(
            text.detail,
            "- osqueryd scheduled heartbeat canary is STALE (last 17s ago, over 020s). The root daemon is not producing scheduled results (stopped or wedged). The uptime watchdog pages on this; this note is the silent daily record.",
            "octal-bound"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("08"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "✅ osquery pipeline healthy · 2026-09-08",
            "invalid-octal"
        );
        assert_eq!(
            text.detail,
            "- The root osqueryd daemon produced a scheduled heartbeat canary 17s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear.",
            "invalid-octal"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("00"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "⚠️ osquery heartbeat · 2026-09-08",
            "bound-double-zero"
        );
        assert_eq!(
            text.detail,
            "- osqueryd scheduled heartbeat canary is STALE (last 17s ago, over 00s). The root daemon is not producing scheduled results (stopped or wedged). The uptime watchdog pages on this; this note is the silent daily record.",
            "bound-double-zero"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("07"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "⚠️ osquery heartbeat · 2026-09-08",
            "bound-octal-seven"
        );
        assert_eq!(
            text.detail,
            "- osqueryd scheduled heartbeat canary is STALE (last 17s ago, over 07s). The root daemon is not producing scheduled results (stopped or wedged). The uptime watchdog pages on this; this note is the silent daily record.",
            "bound-octal-seven"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("010"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "⚠️ osquery heartbeat · 2026-09-08",
            "bound-octal-eight"
        );
        assert_eq!(
            text.detail,
            "- osqueryd scheduled heartbeat canary is STALE (last 17s ago, over 010s). The root daemon is not producing scheduled results (stopped or wedged). The uptime watchdog pages on this; this note is the silent daily record.",
            "bound-octal-eight"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("021"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "✅ osquery pipeline healthy · 2026-09-08",
            "bound-octal-edge"
        );
        assert_eq!(
            text.detail,
            "- The root osqueryd daemon produced a scheduled heartbeat canary 17s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear.",
            "bound-octal-edge"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("9223372036854775808"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "✅ osquery pipeline healthy · 2026-09-08",
            "bound-negative-wrap"
        );
        assert_eq!(
            text.detail,
            "- The root osqueryd daemon produced a scheduled heartbeat canary 17s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear.",
            "bound-negative-wrap"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("18446744073709551615"));
        assert_eq!(window.seconds(), u64::MAX);
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "✅ osquery pipeline healthy · 2026-09-08",
            "bound-unsigned-edge"
        );
        assert_eq!(
            text.detail,
            "- The root osqueryd daemon produced a scheduled heartbeat canary 17s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear.",
            "bound-unsigned-edge"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("18446744073709551616"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("9983"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "✅ osquery pipeline healthy · 2026-09-08",
            "bound-wrap-zero"
        );
        assert_eq!(
            text.detail,
            "- The root osqueryd daemon produced a scheduled heartbeat canary 17s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear.",
            "bound-wrap-zero"
        );
    }
    {
        let window = HeartbeatWindow::from_override(Some("08"));
        let freshness = canary_freshness(10000, CanaryEpoch::parse("10017"), window.seconds());
        let text = heartbeat_text(Some(freshness), "2026-09-08", window.display());
        assert_eq!(
            text.title, "✅ osquery pipeline healthy · 2026-09-08",
            "bound-future-invalid"
        );
        assert_eq!(
            text.detail,
            "- The root osqueryd daemon produced a scheduled heartbeat canary 0s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear.",
            "bound-future-invalid"
        );
    }
}

#[test]
fn invalid_literals_request_a_diagnostic_but_non_numeric_overrides_keep_quiet_default() {
    for raw in ["08", "09", "18446744073709551616"] {
        let window = HeartbeatWindow::from_override(Some(raw));
        assert!(window.invalid_literal(), "{raw}");
        assert_eq!(window.seconds(), 1800);
        assert_eq!(window.display(), "1800");
    }
    for raw in ["bad", "", "-1", "1.5", " 12", "１２"] {
        let window = HeartbeatWindow::from_override(Some(raw));
        assert!(!window.invalid_literal(), "{raw}");
        assert_eq!(window.seconds(), 1800);
        assert_eq!(window.display(), "1800");
    }
    for raw in ["0", "00", "07", "010", "18446744073709551615"] {
        let window = HeartbeatWindow::from_override(Some(raw));
        assert!(!window.invalid_literal(), "{raw}");
        assert_eq!(window.display(), raw);
    }
}
