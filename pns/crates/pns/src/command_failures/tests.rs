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
        "pns --agent posture --state failed --channel testpath"
    );
}

/// A leg with no state or route says only what it has, rather than emitting a
/// flag with an empty value that would not reproduce the send.
#[test]
fn a_leg_with_no_state_or_route_leaves_those_flags_out_entirely() {
    let mut bare = stored(1, pns_domain::retry::DeliveryOutcome::NoResponse);
    bare.state = String::new();
    bare.route = String::new();
    assert_eq!(command(&bare), "pns --agent posture");
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
    unsafe { std::env::set_var("PNS_HERMES_URL", "http://127.0.0.1:9999/webhooks/pns") };
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
