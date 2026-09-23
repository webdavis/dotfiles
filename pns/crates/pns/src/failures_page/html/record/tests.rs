use super::*;
use pns_domain::retry::TransportOutcome;

const NOW: u64 = 1_790_133_900; // 2026-09-23T03:25:00Z

fn retrying() -> (Failure, ListingRow) {
    let failure = Failure {
        id: 7999,
        destination: "phone".to_string(),
        route: String::new(),
        address: "http://127.0.0.1:9090".to_string(),
        agent: "codex".to_string(),
        command: "pns send --producer codex --state done".to_string(),
        outcome: TransportOutcome::NoResponse,
        retries: 12,
        max_attempts: 20,
    };
    let row = ListingRow {
        id: 7999,
        when: "2026-09-23 03:20Z".to_string(),
        status: "no response".to_string(),
        route: String::new(),
        agent: "codex".to_string(),
        destination: "phone".to_string(),
        retries: 12,
        gave_up: false,
    };
    (failure, row)
}

fn dead_lettered() -> (Failure, ListingRow) {
    let failure = Failure {
        id: 42,
        destination: "discord".to_string(),
        route: "pns-events".to_string(),
        address: "http://127.0.0.1:8644".to_string(),
        agent: "posture".to_string(),
        command: "pns send --producer posture --route pns-events".to_string(),
        outcome: TransportOutcome::NoStatus,
        retries: 0,
        max_attempts: 20,
    };
    let row = ListingRow {
        id: 42,
        when: "2026-09-20 00:11Z".to_string(),
        status: "bad URL".to_string(),
        route: "pns-events".to_string(),
        agent: "posture".to_string(),
        destination: "discord".to_string(),
        retries: 0,
        gave_up: true,
    };
    (failure, row)
}

/// Every value `fields()` carries for this failure appears on the record
/// page, plus the headline `headline()` gives it.
#[test]
fn the_record_carries_every_fields_value_and_the_headline() {
    let (failure, row) = retrying();
    let retry = Some(RetryFacts {
        due: NOW + 420,
        started: 1_790_000_000,
    });
    let rendered = record_page(&failure, &row, retry, NOW);
    for (label, value) in failure::fields(&failure) {
        // "status" is the one field the record shows in a different spelling:
        // the timing box's "Attempt N · <status word>" reads the SHORT,
        // capitalized word (matching the headline's own rule), not the long
        // `meaning::status` form `fields()` carries for a registered code.
        if value.is_empty() || label == "status" {
            continue;
        }
        assert!(
            rendered.contains(&escaped(&value)),
            "missing {value:?}: {rendered}"
        );
    }
    assert!(
        rendered.contains("Phone didn\u{2019}t respond"),
        "{rendered}"
    );
}

/// A retrying leg's timing box counts down to the ledger's own due time and
/// names the next attempt's number.
#[test]
fn a_retrying_legs_timing_box_counts_down_to_the_due_time() {
    let (failure, row) = retrying();
    let retry = Some(RetryFacts {
        due: NOW + 420,
        started: 1_790_000_000,
    });
    let rendered = record_page(&failure, &row, retry, NOW);
    assert!(rendered.contains("In 7 minutes"), "{rendered}");
    assert!(rendered.contains("Attempt 13"), "{rendered}"); // the last attempt
    assert!(rendered.contains("Attempt 14"), "{rendered}"); // the next one
    assert!(rendered.contains("03:32"), "{rendered}");
}

/// A dead-lettered leg's next attempt reads "None", with "pns gave up" as
/// the meta underneath rather than a schedule that no longer exists.
#[test]
fn a_dead_lettered_legs_next_attempt_says_none_and_that_pns_gave_up() {
    let (failure, row) = dead_lettered();
    let rendered = record_page(&failure, &row, None, NOW);
    assert!(rendered.contains(">None<"), "{rendered}");
    assert!(rendered.contains("pns gave up"), "{rendered}");
    assert!(rendered.contains("Not delivered"), "{rendered}");
}

/// An empty webhook route reads "none", muted, never a blank cell.
#[test]
fn an_empty_webhook_route_reads_none() {
    let (failure, row) = retrying();
    let rendered = record_page(&failure, &row, None, NOW);
    assert!(rendered.contains(">none<"), "{rendered}");
}

/// The back link to the listing sits above the headline.
#[test]
fn a_back_link_points_at_the_listing() {
    let (failure, row) = retrying();
    let rendered = record_page(&failure, &row, None, NOW);
    assert!(rendered.contains("href=\"/\">Failures</a>"), "{rendered}");
}

/// The page is dark always on the record too.
#[test]
fn the_record_is_dark_only() {
    let (failure, row) = dead_lettered();
    let rendered = record_page(&failure, &row, None, NOW);
    assert!(
        rendered.contains("<meta name=\"color-scheme\" content=\"dark\">"),
        "{rendered}"
    );
    assert!(!rendered.contains("light-dark("), "{rendered}");
    assert!(!rendered.contains("prefers-color-scheme"), "{rendered}");
}
