mod funnel_fixture;

macro_rules! cases {
    ($($name:ident),* $(,)?) => {$(
        #[test]
        fn $name() { funnel_fixture::compare(stringify!($name)); }
    )*};
}
cases!(
    allow_nan,
    deep_active,
    baseline_command_text,
    status_timeout,
    timeout_leading_space,
    timeout_hex,
    inactive_first,
    active_first,
    steady_active,
    closed,
    opened,
    corrupt_idle,
    corrupt_active,
    pretty_active,
    multiple_baselines,
    false_value,
    null_value,
    false_entries,
    nested_active,
    wrong_shape,
    wrong_entry,
    invalid_wins,
    stream_documents,
    whitespace_only,
    empty,
    malformed,
    null_document,
    duplicate_values,
    duplicate_entry,
    low_surrogate,
    high_surrogate,
    nonnumeric_extension,
    hostile_keys,
    command_failure,
    missing_binary,
    gap_covered,
    gap_recovered,
    persist_distrust,
    refused_exposure,
    refused_read_gap,
    refused_corruption_warning,
    publish_failure,
    failed_close,
    persist_gap_covered,
);

#[test]
fn oversized_exposure_reports_omission_without_advancing_the_baseline() {
    funnel_fixture::compare("oversized_exposure");
}
