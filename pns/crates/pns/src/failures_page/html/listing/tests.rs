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

fn no_retry_facts(_: u64) -> Result<Option<RetryFacts>, ()> {
    Ok(None)
}

fn unreadable_retry_facts(_: u64) -> Result<Option<RetryFacts>, ()> {
    Err(())
}

fn retrying_due_in_seven_minutes(id: u64) -> Result<Option<RetryFacts>, ()> {
    Ok((id == 201).then_some(RetryFacts {
        due: NOW + 420,
        started: 1_790_000_000,
    }))
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
    // Every row's own id is linked, burst members and singles alike.
    for id in [201, 148, 149, 150, 140] {
        assert!(
            rendered.contains(&format!("/failures/{id}")),
            "leg {id} missing: {rendered}"
        );
    }
    // The burst's own title, and the lone dead leg's own headline (a
    // different status word, which is what keeps it out of the burst).
    assert!(rendered.contains("Bad URL"), "{rendered}");
    assert!(rendered.contains("Discord answered HTTP 500"), "{rendered}");
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

/// Every burst Records line, and a single leg's own Delivery line, link the
/// leg's detail page under `/failures/<id>`: the bare `/<id>` this page used
/// to serve is retired.
#[test]
fn every_leg_links_its_own_id_under_failures() {
    let rendered = listing_page(&fixture(), &no_retry_facts, NOW);
    for id in [201, 148, 149, 150] {
        assert!(
            rendered.contains(&format!("<a href=\"/failures/{id}\">{id}</a>")),
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

/// Every entry's own `class` attribute, in rendered order: `fh-active`
/// belongs to a live entry alone, and `fh-last` belongs to the last entry
/// alone. A page-wide `contains("fh-last")` cannot tell the class from the
/// same word in the page's own `<style>` selector, so this reads each
/// entry's attribute directly.
#[test]
fn only_live_entries_are_active_and_only_the_last_entry_is_fh_last() {
    let rendered = listing_page(&fixture(), &no_retry_facts, NOW);
    let classes: Vec<&str> = rendered
        .match_indices("<article class=\"")
        .map(|(start, _)| {
            let after = &rendered[start + "<article class=\"".len()..];
            &after[..after.find('"').expect("a closing quote")]
        })
        .collect();
    // Leg 201 (live), the dead-lettered burst, leg 140 (dead): three entries.
    assert_eq!(classes.len(), 3, "{classes:?}");
    assert!(classes[0].contains("fh-active"), "{classes:?}");
    assert!(!classes[1].contains("fh-active"), "{classes:?}");
    assert!(!classes[2].contains("fh-active"), "{classes:?}");
    assert!(!classes[0].contains("fh-last"), "{classes:?}");
    assert!(!classes[1].contains("fh-last"), "{classes:?}");
    assert!(classes[2].contains("fh-last"), "{classes:?}");
}

/// A single retrying leg's next line counts down to the ledger's own due
/// time, never a re-derived one.
#[test]
fn a_single_retrying_legs_next_line_counts_down_to_the_ledgers_due_time() {
    let rendered = listing_page(&fixture(), &retrying_due_in_seven_minutes, NOW);
    assert!(rendered.contains("Next try in 7 minutes"), "{rendered}");
}

/// A retry-facts read failure reads "unknown", never a false "now": the two
/// outcomes look identical once `Result` is flattened, and one of them
/// tells the reader a retry is imminent when the page simply does not know.
#[test]
fn an_unreadable_retry_schedule_says_unknown_never_now() {
    let rendered = listing_page(&fixture(), &unreadable_retry_facts, NOW);
    assert!(rendered.contains("Next try unknown"), "{rendered}");
    assert!(!rendered.contains("Next try now"), "{rendered}");
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

/// The Source cell reaches the page escaped too, on a single leg's own
/// Details and on a burst's Records line alike.
#[test]
fn an_agent_containing_markup_is_escaped() {
    let mut failures = fixture();
    failures[0].agent = "<i>evil</i>".to_string(); // leg 201, a single entry
    let rendered = listing_page(&failures, &no_retry_facts, NOW);
    assert!(!rendered.contains("<i>evil</i>"), "{rendered}");
    assert!(rendered.contains("&lt;i&gt;evil&lt;/i&gt;"), "{rendered}");
}

/// A single leg's Retry deadline reads from the ledger's own first-attempt
/// `started`, not the leg's `failed_at`: the two differ by more than a day
/// in this fixture, so a deadline computed from the wrong one lands on the
/// wrong date.
#[test]
fn a_single_legs_retry_deadline_reads_from_started_not_failed_at() {
    let rendered = listing_page(&fixture(), &retrying_due_in_seven_minutes, NOW);
    assert!(
        rendered.contains("September 28, 2026 at 14:13 UTC"),
        "{rendered}"
    );
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

/// The listing footer carries the `fh-footer` class its CSS styles; a bare
/// `<footer>` loses its rule, muted color, size and flex layout.
#[test]
fn the_footer_carries_the_fh_footer_class() {
    let rendered = listing_page(&fixture(), &no_retry_facts, NOW);
    assert!(
        rendered.contains("<footer class=\"fh-footer\">"),
        "{rendered}"
    );
}

/// A burst's time gutter reads oldest first, then "to" the newest, matching
/// the supplied mockup; the Window line in its own details reads the same
/// direction.
#[test]
fn a_bursts_gutter_reads_oldest_then_to_newest() {
    let rendered = listing_page(&fixture(), &no_retry_facts, NOW);
    let oldest_at = rendered.find("00:11").unwrap();
    let to_newest_at = rendered.find("to 00:42").unwrap();
    assert!(
        oldest_at < to_newest_at,
        "the oldest time must render before \"to\" the newest: {rendered}"
    );
}

/// The page is dark always: the color-scheme meta says so and neither
/// `light-dark()` nor `prefers-color-scheme` appears anywhere in the output.
#[test]
fn the_listing_is_dark_only() {
    let rendered = listing_page(&fixture(), &no_retry_facts, NOW);
    assert!(
        rendered.contains("<meta name=\"color-scheme\" content=\"dark\">"),
        "{rendered}"
    );
    assert!(!rendered.contains("light-dark("), "{rendered}");
    assert!(!rendered.contains("prefers-color-scheme"), "{rendered}");
}

/// A burst's subtitle reads in sentence case, matching the brief and the
/// mockup: only the phrase's own first letter is capitalized, not every
/// destination joined into it.
#[test]
fn a_bursts_subtitle_reads_in_sentence_case() {
    let rendered = listing_page(&fixture(), &no_retry_facts, NOW);
    assert!(rendered.contains("Discord and phone"), "{rendered}");
    assert!(!rendered.contains("Discord and Phone"), "{rendered}");
}

/// A headline that already names the destination ("Phone didn’t respond",
/// "Discord answered HTTP 500") gets no subtitle underneath repeating it;
/// only the burst, whose headline is a bare status word, needs one.
#[test]
fn a_headline_that_already_names_the_destination_has_no_repeated_subtitle() {
    let rendered = listing_page(&fixture(), &no_retry_facts, NOW);
    assert_eq!(
        rendered.matches("<div class=\"fh-sub\">").count(),
        1,
        "{rendered}"
    );
}

/// A single leg whose headline is a bare status word (nothing to say which
/// destination) still gets a subtitle naming it.
#[test]
fn a_single_legs_headline_naming_no_destination_still_gets_a_subtitle() {
    let solo = vec![leg(1, "phone", "codex", TransportOutcome::NoStatus, NOW)];
    let rendered = listing_page(&solo, &no_retry_facts, NOW);
    assert!(
        rendered.contains("<div class=\"fh-sub\">Phone</div>"),
        "{rendered}"
    );
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
