use super::*;

fn stored(id: u64, outcome: pns_domain::retry::DeliveryOutcome) -> StoredFailure {
    StoredFailure {
        id,
        destination: failure::DESTINATION_HERMES.to_string(),
        route: "testpath".to_string(),
        agent: "posture".to_string(),
        state: "failed".to_string(),
        outcome,
        failed_at: 1_757_000_000,
        retries: 1,
        deadlettered: true,
    }
}

/// The command is RECONSTRUCTED from the routing facts, and it is the string
/// the reader searches for, so it has to be the one they would actually find.
#[test]
fn the_shown_command_is_the_routing_flags_that_produced_this_leg() {
    let failure = compose(&stored(47, pns_domain::retry::DeliveryOutcome::Status(404)));
    assert_eq!(
        failure.command,
        "pns send --producer posture --state failed --route testpath"
    );
}

/// A leg with no state or route says only what it has, rather than emitting a
/// flag with an empty value that would not reproduce the send.
#[test]
fn a_leg_with_no_state_or_route_leaves_those_flags_out_entirely() {
    let mut bare = stored(1, pns_domain::retry::DeliveryOutcome::NoResponse);
    bare.state = String::new();
    bare.route = String::new();
    assert_eq!(command(&bare), "pns send --producer posture");
}

/// An internal state word (one `pns send --state` refuses) is left out of the
/// printed command entirely, so the suggestion the reader is shown always runs.
#[test]
fn an_internal_state_word_is_left_out_of_the_printed_command() {
    let mut recap = stored(2, pns_domain::retry::DeliveryOutcome::NoResponse);
    recap.state = "recap".to_string();
    recap.route = String::new();
    assert_eq!(command(&recap), "pns send --producer posture");
}

/// The listing column is scanned, so it carries the bare code; the registered
/// name is what teaches and belongs in the full form.
#[test]
fn the_listing_status_is_the_code_alone_and_names_each_silence() {
    assert_eq!(
        short_status(&stored(1, pns_domain::retry::DeliveryOutcome::Status(404))),
        "HTTP 404"
    );
    assert_eq!(
        short_status(&stored(1, pns_domain::retry::DeliveryOutcome::NoResponse)),
        "no response"
    );
    assert_eq!(
        short_status(&stored(1, pns_domain::retry::DeliveryOutcome::NoStatus)),
        "bad URL"
    );
}

/// Cut at the minute BY LENGTH. Matching the seconds would silently do nothing
/// for 59 seconds out of every 60, which is the kind of bug that only shows up
/// in a screenshot.
#[test]
fn the_listing_clock_is_cut_at_the_minute_whatever_the_seconds_are() {
    // Exactly 2025-09-04T15:33:00Z, so the whole minute runs to +59 and the
    // next second belongs to a different one.
    let minute = 1_756_999_980;
    assert_eq!(when(minute), "2025-09-04 15:33Z");
    assert_eq!(when(minute + 1), when(minute));
    assert_eq!(when(minute + 59), when(minute));
    assert_eq!(when(minute + 60), "2025-09-04 15:34Z");
}

/// The address is where the reader would type it, and it follows the override
/// the gateway itself honours, so the message names the gateway THIS machine
/// posts to rather than the shipped default.
#[test]
fn the_address_follows_the_gateway_override_the_channel_itself_reads() {
    // SAFETY: single-threaded test process; the variable is restored below.
    let previous = std::env::var("PNS_HERMES_URL").ok();
    unsafe {
        std::env::set_var(
            "PNS_HERMES_URL",
            "http://127.0.0.1:9999/webhooks/pns-events",
        )
    };
    assert_eq!(
        address(failure::DESTINATION_HERMES, "testpath"),
        "http://127.0.0.1:9999/webhooks/testpath"
    );
    unsafe { std::env::remove_var("PNS_HERMES_URL") };
    assert!(address(failure::DESTINATION_HERMES, "testpath").ends_with("/testpath"));
    if let Some(previous) = previous {
        unsafe { std::env::set_var("PNS_HERMES_URL", previous) };
    }
}

/// This binary's own path, never a bare name: a click has no PATH to resolve
/// with, and a machine mid-upgrade can have two pns on disk.
#[test]
fn the_view_runs_this_binary_rather_than_whatever_a_path_would_find() {
    let path = pns_path();
    assert!(path.starts_with('/'), "{path}");
    assert_ne!(path, "pns");
}

/// The whole of what a banner click is for, end to end through the domain: the
/// argv opens THIS binary's detail view for the id the banner carried.
#[test]
fn the_inferred_view_opens_this_binarys_detail_view_for_that_id() {
    let argv = ClickView::inferred(false)
        .argv(47, "/bin/herdr", &pns_path())
        .unwrap();
    assert_eq!(argv[0], "/usr/bin/open");
    assert_eq!(argv.last().unwrap(), &format!("{} failures 47", pns_path()));
}

/// The stored click command names the verb that opens the view, so a banner
/// raised now is clickable by the binary that raised it.
#[test]
fn the_banners_stored_click_command_names_the_open_verb() {
    assert_eq!(
        failure::click_command(&pns_path(), 47),
        format!("{} failures {OPEN_VERB} 47", pns_path())
    );
}

/// A usage line names both spellings of the id argument, so an operator
/// diagnosing a click that did nothing is told what it wanted.
#[test]
fn the_usage_line_names_the_open_verb_and_its_argument() {
    assert!(FAILURES_USAGE.contains("open <id>"), "{FAILURES_USAGE}");
}
