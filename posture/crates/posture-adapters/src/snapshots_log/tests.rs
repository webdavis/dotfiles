use super::*;

#[test]
fn captured_empty() {
    assert_eq!(newest(&b""[..]).unwrap().map(CanaryEpoch::seconds), None);
}

#[test]
fn captured_compact() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_spaced() {
    assert_eq!(
        newest(&b"{ \"name\" : \"heartbeat_canary\", \"unixTime\" : 9970 }\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_other_name() {
    assert_eq!(
        newest(&b"{\"name\":\"other\",\"unixTime\":9999}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_last_not_maximum() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9999}\n{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n"[..]).unwrap().map(CanaryEpoch::seconds), Some(9970));
}

#[test]
fn captured_invalid_last_masks_prior() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n{\"name\":\"heartbeat_canary\",\"unixTime\":\"bad\"}\n"[..]).unwrap().map(CanaryEpoch::seconds), None);
}

#[test]
fn captured_null_last_retains_prior() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n{\"name\":\"heartbeat_canary\",\"unixTime\":null}\n"[..]).unwrap().map(CanaryEpoch::seconds), Some(9970));
}

#[test]
fn captured_false_last_retains_prior() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n{\"name\":\"heartbeat_canary\",\"unixTime\":false}\n"[..]).unwrap().map(CanaryEpoch::seconds), Some(9970));
}

#[test]
fn captured_torn_before_and_after_valid() {
    assert_eq!(newest(&b"not json\n{\"name\":\n{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n{\"name\":\n"[..]).unwrap().map(CanaryEpoch::seconds), Some(9970));
}

#[test]
fn captured_no_final_newline() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970}"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_fallback() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"snapshot\":[{\"unix_time\":\"9970\"}]}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_false_fallback() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":false,\"snapshot\":[{\"unix_time\":\"9970\"}]}\n"[..]).unwrap().map(CanaryEpoch::seconds), Some(9970));
}

#[test]
fn captured_prefer_envelope() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970,\"snapshot\":[{\"unix_time\":\"1\"}]}\n"[..]).unwrap().map(CanaryEpoch::seconds), Some(9970));
}

#[test]
fn captured_zero() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":0}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(0)
    );
}

#[test]
fn captured_ten_digit_maximum() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9999999999}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9999999999)
    );
}

#[test]
fn captured_eleven_digits() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":10000000000}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_leading_zero_string() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":\"09999999999\"}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_leading_zero_number() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":00012}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(12)
    );
}

#[test]
fn captured_numeric_exponent() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":1e3}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_numeric_fraction() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":12.0}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_negative_zero() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":-0}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_positive_number() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":+12}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(12)
    );
}

#[test]
fn captured_nan() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":NaN}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_infinity() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":Infinity}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_inert_nan() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970,\"other\":NaN}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_inert_low_surrogate() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970,\"other\":\"\\udc00\"}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_inert_high_surrogate() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970,\"other\":\"\\ud800\"}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_invalid_utf8_inert() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970,\"other\":\"\xff\"}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_duplicate_timestamp() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":1,\"unixTime\":9970}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_duplicate_name() {
    assert_eq!(
        newest(&b"{\"name\":\"other\",\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_string_newline_last_value() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":\"bad\\n9970\"}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_string_newline_invalid_tail() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":\"9970\\nbad\"}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_string_trailing_newline() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":\"9970\\n\"}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        None
    );
}

#[test]
fn captured_string_nul() {
    assert_eq!(
        newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":\"99\\u000070\"}\n"[..])
            .unwrap()
            .map(CanaryEpoch::seconds),
        Some(9970)
    );
}

#[test]
fn captured_empty_string_masks_prior() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n{\"name\":\"heartbeat_canary\",\"unixTime\":\"\"}\n"[..]).unwrap().map(CanaryEpoch::seconds), None);
}

#[test]
fn captured_array_masks_prior() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n{\"name\":\"heartbeat_canary\",\"unixTime\":[]}\n"[..]).unwrap().map(CanaryEpoch::seconds), None);
}

#[test]
fn captured_object_masks_prior() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n{\"name\":\"heartbeat_canary\",\"unixTime\":{}}\n"[..]).unwrap().map(CanaryEpoch::seconds), None);
}

#[test]
fn captured_scalar_between_rows() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":1}\n5\n{\"name\":\"heartbeat_canary\",\"unixTime\":9970}\n"[..]).unwrap().map(CanaryEpoch::seconds), Some(9970));
}

#[test]
fn captured_deep_inert() {
    assert_eq!(newest(&b"{\"name\":\"heartbeat_canary\",\"unixTime\":9970,\"other\":[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[0]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]}\n"[..]).unwrap().map(CanaryEpoch::seconds), Some(9970));
}

mod files;
