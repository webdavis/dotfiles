use super::*;
fn epoch(value: &str) -> Option<CanaryEpoch> {
    CanaryEpoch::parse(value)
}
#[test]
fn decimal_epoch_accepts_zero_and_ten_digits_only() {
    assert_eq!(epoch("0").map(CanaryEpoch::seconds), Some(0));
    assert_eq!(
        epoch("9999999999").map(CanaryEpoch::seconds),
        Some(9_999_999_999)
    );
    for value in [
        "",
        "00",
        "09999999999",
        "10000000000",
        "18446744073709561616",
        "-1",
        "+1",
        "1.0",
        "1E+3",
        "1\n",
        "１２",
        "$(touch PWNED)",
    ] {
        assert_eq!(epoch(value), None, "{value:?}");
    }
}
#[test]
fn past_boundary_and_both_neighbors() {
    assert_eq!(
        canary_freshness(10_000, epoch("8201"), 1800),
        CanaryFreshness::Fresh { age: 1799 }
    );
    assert_eq!(
        canary_freshness(10_000, epoch("8200"), 1800),
        CanaryFreshness::Fresh { age: 1800 }
    );
    assert_eq!(
        canary_freshness(10_000, epoch("8199"), 1800),
        CanaryFreshness::Stale { age: 1801 }
    );
}
#[test]
fn future_boundary_and_both_neighbors_clamp_age_to_zero() {
    assert_eq!(
        canary_freshness(10_000, epoch("11799"), 1800),
        CanaryFreshness::Fresh { age: 0 }
    );
    assert_eq!(
        canary_freshness(10_000, epoch("11800"), 1800),
        CanaryFreshness::Fresh { age: 0 }
    );
    assert_eq!(
        canary_freshness(10_000, epoch("11801"), 1800),
        CanaryFreshness::Implausible { skew: 1801 }
    );
}
#[test]
fn missing_is_distinct_from_a_real_stale_epoch() {
    assert_eq!(
        canary_freshness(10_000, None, 1800),
        CanaryFreshness::Missing
    );
    assert_eq!(
        canary_freshness(10_000, epoch("0"), 1800),
        CanaryFreshness::Stale { age: 10_000 }
    );
}
#[test]
fn zero_window_requires_equality() {
    assert_eq!(
        canary_freshness(10, epoch("10"), 0),
        CanaryFreshness::Fresh { age: 0 }
    );
    assert_eq!(
        canary_freshness(10, epoch("9"), 0),
        CanaryFreshness::Stale { age: 1 }
    );
    assert_eq!(
        canary_freshness(10, epoch("11"), 0),
        CanaryFreshness::Implausible { skew: 1 }
    );
}
#[test]
fn clock_distance_does_not_wrap_into_freshness() {
    assert_eq!(
        canary_freshness(u64::MAX, epoch("0"), 1800),
        CanaryFreshness::Stale { age: u64::MAX }
    );
    assert_eq!(
        canary_freshness(0, epoch("9999999999"), 1800),
        CanaryFreshness::Implausible {
            skew: 9_999_999_999
        }
    );
}
