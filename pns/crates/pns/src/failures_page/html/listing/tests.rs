use super::*;
use pns_domain::retry::TransportOutcome;

const NOW: u64 = 1_790_133_900; // 2026-09-23T03:25:00Z

fn leg(
    id: u64,
    destination: &str,
    agent: &str,
    outcome: TransportOutcome,
    failed_at: u64,
) -> StoredFailure {
    StoredFailure {
        id,
        destination: destination.to_string(),
        route: "pns-events".to_string(),
        agent: agent.to_string(),
        state: "failed".to_string(),
        outcome,
        failed_at,
        retries: 3,
        deadlettered: !matches!(outcome, TransportOutcome::NoResponse),
    }
}

/// One retrying leg today, a three-leg dead-lettered burst on an earlier day,
/// and one single dead-lettered leg earlier the same day as the burst but
/// with a different status word, which is what keeps it its own entry rather
/// than joining the burst.
fn fixture() -> Vec<StoredFailure> {
    vec![
        leg(
            201,
            "phone",
            "codex",
            TransportOutcome::NoResponse,
            1_790_133_600,
        ),
        leg(
            150,
            "discord",
            "posture",
            TransportOutcome::NoStatus,
            1_789_864_920,
        ),
        leg(
            149,
            "phone",
            "claude",
            TransportOutcome::NoStatus,
            1_789_864_200,
        ),
        leg(
            148,
            "discord",
            "codex",
            TransportOutcome::NoStatus,
            1_789_863_060,
        ),
        leg(
            140,
            "discord",
            "karlmdavis",
            TransportOutcome::Status(500),
            1_789_862_700,
        ),
    ]
}

fn no_retry_facts(_: u64) -> Option<RetryFacts> {
    None
}

fn retrying_due_in_seven_minutes(id: u64) -> Option<RetryFacts> {
    (id == 201).then_some(RetryFacts {
        due: NOW + 420,
        started: 1_790_000_000,
    })
}

/// Every `ListingRow` value the fixture holds appears verbatim in the
/// listing, and the two days it spans appear as headings, newest first.
#[test]
fn every_rows_value_appears_and_the_days_are_headed_newest_first() {
    let rendered = listing_page(&fixture(), &retrying_due_in_seven_minutes, NOW);
    assert!(rendered.contains("Today"), "{rendered}");
    assert!(rendered.contains("Sep 20"), "{rendered}");
    let today_at = rendered.find("Today").unwrap();
    let sep20_at = rendered.find("Sep 20").unwrap();
    assert!(today_at < sep20_at, "Today must come before Sep 20");
    // The single retrying leg: destination capitalized in the headline.
    assert!(
        rendered.contains("Phone didn\u{2019}t respond"),
        "{rendered}"
    );
    // The burst and the lone dead leg both carry their sender, verbatim.
    for sender in ["posture", "claude", "codex", "karlmdavis"] {
        assert!(rendered.contains(sender), "{sender} missing: {rendered}");
    }
}

/// Consecutive same-day, same-status, same-gave-up legs collapse into one
/// entry, counted and spanned from the raw `failed_at` seconds.
#[test]
fn the_three_leg_burst_is_one_entry_with_its_count_and_span() {
    let rendered = listing_page(&fixture(), &no_retry_facts, NOW);
    assert!(rendered.contains("3 failed deliveries"), "{rendered}");
    assert!(rendered.contains("over 31 minutes"), "{rendered}");
    // The lone dead leg (a different status word) is its own entry, not
    // folded into the burst's count.
    assert!(!rendered.contains("4 failed deliveries"), "{rendered}");
}

/// Every burst Records line links the leg's own detail page.
#[test]
fn burst_records_link_every_legs_own_id() {
    let rendered = listing_page(&fixture(), &no_retry_facts, NOW);
    for id in [148, 149, 150] {
        assert!(
            rendered.contains(&format!("<a href=\"/{id}\">{id}</a>")),
            "leg {id} not linked: {rendered}"
        );
    }
}

/// The state chip: "Retrying" with `fh-active` for a live leg, "Not
/// delivered" with no such class for a dead one.
#[test]
fn the_state_chip_marks_a_live_leg_active_and_a_dead_one_not() {
    let rendered = listing_page(&fixture(), &retrying_due_in_seven_minutes, NOW);
    assert!(rendered.contains("fh-active"), "{rendered}");
    assert!(rendered.contains("Retrying"), "{rendered}");
    assert!(rendered.contains("Not delivered"), "{rendered}");
}

/// A single retrying leg's next line counts down to the ledger's own due
/// time, never a re-derived one.
#[test]
fn a_single_retrying_legs_next_line_counts_down_to_the_ledgers_due_time() {
    let rendered = listing_page(&fixture(), &retrying_due_in_seven_minutes, NOW);
    assert!(rendered.contains("Next try in 7 minutes"), "{rendered}");
}

/// A route holding markup reaches the page escaped, never live HTML. Only a
/// burst's Records list shows a route at all (a single leg's Details does
/// not), so the burst is where this has to be exercised.
#[test]
fn a_route_containing_markup_is_escaped() {
    let mut failures = fixture();
    failures[1].route = "<b>evil</b>".to_string();
    let rendered = listing_page(&failures, &no_retry_facts, NOW);
    assert!(!rendered.contains("<b>evil</b>"), "{rendered}");
    assert!(rendered.contains("&lt;b&gt;evil&lt;/b&gt;"), "{rendered}");
}

/// An empty ledger says so in one line, inside the same card shell.
#[test]
fn an_empty_ledger_says_nothing_is_failing() {
    let rendered = listing_page(&[], &no_retry_facts, NOW);
    assert!(
        rendered.contains("Nothing is failing to deliver."),
        "{rendered}"
    );
    assert!(rendered.contains("Failures"), "{rendered}");
    assert!(rendered.contains("All times UTC"), "{rendered}");
}

/// Both appearances resolve: the color-scheme meta and a `light-dark()`
/// ground are both present.
#[test]
fn both_appearances_are_wired() {
    let rendered = listing_page(&fixture(), &no_retry_facts, NOW);
    assert!(
        rendered.contains("<meta name=\"color-scheme\" content=\"light dark\">"),
        "{rendered}"
    );
    assert!(rendered.contains("light-dark("), "{rendered}");
}

/// Distinct destinations join with "and"; one item needs no joiner at all.
#[test]
fn destinations_join_with_and() {
    assert_eq!(joined_with_and(&["Phone".to_string()]), "Phone");
    assert_eq!(
        joined_with_and(&["Discord".to_string(), "Phone".to_string()]),
        "Discord and Phone"
    );
    assert_eq!(
        joined_with_and(&["A".to_string(), "B".to_string(), "C".to_string()]),
        "A, B, and C"
    );
}
