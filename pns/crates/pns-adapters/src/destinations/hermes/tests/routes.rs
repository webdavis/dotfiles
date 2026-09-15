use super::super::{DEFAULT_HERMES_URL, channel_url};
use crate::destinations::Delivery;
use pns_application::NotificationDestination;
use pns_domain::routing::ReportMode;
use pns_domain::safety::route_name_is_usable;
use pns_hermes::{PostOutcome, sign, skipped_line};

#[test]
fn one_rule_judges_a_route_name_wherever_it_is_read() {
    // THE PREDICATE IS THE SHARED HALF: the config read that resolves a
    // route by name and the URL swap that spends it must agree about what
    // a name is, or a value the config waved through becomes a URL the
    // swap refuses (or worse, the other way around).
    for usable in [
        "priority",
        "unattended-upgrades",
        "pns-events",
        "log_2",
        "A9",
    ] {
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

// --- one key per route ---------------------------------------------------

/// THE WHOLE POINT, positively: the signature a route posts under is computed
/// from that route's own key and no other.
///
/// TWO ROUTES OFF ONE SETTINGS TABLE, because a channel that read the first
/// key in the table, or the only key in it, would pass a single-route case.
#[test]
fn each_route_signs_with_its_own_key_off_one_settings_table() {
    const SETTINGS: &str = "[keys]\npns-events = \"for-pns-events\"\npriority = \"for-priority\"\n";
    for (route, key) in [
        ("pns-events", "for-pns-events"),
        ("priority", "for-priority"),
    ] {
        let channel = super::channel_for_route(route, SETTINGS, PostOutcome::Status(200));
        let event = super::event();
        let request = super::delivery_request(&event, ReportMode::ReportOutcome);
        assert_eq!(
            channel.deliver(&request),
            Delivery::Delivered("posted HTTP 200".into())
        );
        let posts = channel.post.posts.lock().unwrap();
        let (_, body, signature, _, _) = &posts[0];
        assert_eq!(
            signature.as_str(),
            sign(key, body).expect("a non-empty key signs").as_str(),
            "the {route} route did not sign with its own key"
        );
    }
}

/// AND THE FAIL-CLOSED HALF: a route the config named no key for sends
/// nothing and says which key is missing, rather than borrowing one of the
/// keys it does have.
#[test]
fn a_route_with_no_key_of_its_own_posts_nothing_and_names_the_missing_key() {
    let channel = super::channel_for_route(
        "posture-pages",
        "[keys]\npns-events = \"for-pns-events\"\npriority = \"for-priority\"\n",
        PostOutcome::Status(200),
    );
    let event = super::event();
    assert_eq!(
        channel.deliver(&super::delivery_request(&event, ReportMode::ReportOutcome)),
        Delivery::Failed(skipped_line("posture-pages"))
    );
    assert!(
        channel.post.posts.lock().unwrap().is_empty(),
        "a route with no key must put nothing on the wire"
    );
}
