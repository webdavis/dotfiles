use super::*;
#[test]
fn linear_retry_delays_preserve_exact_counts_and_saturate_unsigned_time() {
    let backoff = RetryBackoff {
        base_secs: 7,
        random_secs: 0,
    };
    assert_eq!(backoff.retry_at(10, 1, 65535), 17);
    assert_eq!(backoff.retry_at(10, 2, 65535), 24);
    assert_eq!(backoff.retry_at(u64::MAX - 5, 1, 0), u64::MAX);
    assert_eq!(backoff.retry_at(10, u64::MAX, 0), u64::MAX);
}
#[test]
fn retry_jitter_is_inclusive_nonnegative_and_preserves_the_legacy_sample_width() {
    let backoff = RetryBackoff {
        base_secs: 7,
        random_secs: 3,
    };
    assert_eq!(backoff.retry_at(10, 2, 0), 24);
    assert_eq!(backoff.retry_at(10, 2, 3), 27);
    assert_eq!(backoff.retry_at(10, 2, 4), 24);
    let wide = RetryBackoff {
        base_secs: 0,
        random_secs: u64::MAX,
    };
    assert_eq!(wide.retry_at(0, 1, u16::MAX), 32767);
}
