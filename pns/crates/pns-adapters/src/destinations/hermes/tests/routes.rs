use super::super::{DEFAULT_HERMES_URL, channel_url};
use pns_domain::safety::route_name_is_usable;

#[test]
fn one_rule_judges_a_route_name_wherever_it_is_read() {
    // THE PREDICATE IS THE SHARED HALF: the config read that resolves a
    // route by name and the URL swap that spends it must agree about what
    // a name is, or a value the config waved through becomes a URL the
    // swap refuses (or worse, the other way around).
    for usable in ["priority", "unattended-upgrades", "pns", "log_2", "A9"] {
        assert!(route_name_is_usable(usable), "case: {usable:?}");
    }
    // Every form `channel_url` refuses, refused here too AND still refused
    // through it: the extraction is only worth anything if the caller kept
    // asking.
    for hostile in [
        "", "a/b", "../x", "a b", "a?x=1", "a#f", ".", "a\nb", "%2e%2e", "café",
    ] {
        assert!(!route_name_is_usable(hostile), "case: {hostile:?}");
        assert_eq!(
            channel_url(DEFAULT_HERMES_URL, hostile),
            None,
            "case: {hostile:?}"
        );
    }
}

#[test]
fn a_route_name_swaps_the_default_urls_final_segment() {
    assert_eq!(
        channel_url(DEFAULT_HERMES_URL, "unattended-upgrades").as_deref(),
        Some("http://127.0.0.1:8644/webhooks/unattended-upgrades")
    );
}

#[test]
fn a_name_that_could_not_be_a_path_segment_is_refused_not_glued() {
    // The name is about to become part of a URL, so this is a trust
    // boundary like the site id's: nothing traversal-shaped passes.
    for hostile in ["", "a/b", "../x", "a b", "a?x=1", "a#f", "."] {
        assert_eq!(
            channel_url(DEFAULT_HERMES_URL, hostile),
            None,
            "case: {hostile:?}"
        );
    }
}

#[test]
fn a_base_without_a_path_yields_nothing_rather_than_a_bogus_url() {
    assert_eq!(channel_url("no-slashes-here", "log"), None);
}
