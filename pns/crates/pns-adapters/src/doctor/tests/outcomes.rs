use super::*;

// --- the report ----------------------------------------------------------

#[test]
fn a_line_names_its_plugin_and_its_outcome_and_a_failure_quotes_the_channel() {
    let hermes = Check {
        plugin: "hermes",
        kind: CheckKind::Send,
    };
    assert_eq!(
        line(&hermes, &Outcome::Sent("posted HTTP 200".to_string())),
        "hermes: sent, posted HTTP 200"
    );
    assert_eq!(
        line(
            &hermes,
            &Outcome::Failed("post FAILED HTTP 401".to_string())
        ),
        "hermes: FAILED, post FAILED HTTP 401",
        "the channel's own sentence, verbatim: a doctor that paraphrased \
             would be a second wording of one answer"
    );
    assert_eq!(
        line(&hermes, &Outcome::SentUnreported),
        "hermes: sent, this channel reports no outcome"
    );
    let router = Check {
        plugin: "router",
        kind: CheckKind::Skipped(A_SENSOR),
    };
    assert_eq!(
        line(&router, &Outcome::Skipped(A_SENSOR)),
        "router: skipped, a sensor and never a delivery destination"
    );
}

#[test]
fn the_pulse_line_claims_neither_a_flash_nor_a_cause_it_cannot_know() {
    let hue = Check {
        plugin: "hue",
        kind: CheckKind::Pulse,
    };
    assert_eq!(
        line(&hue, &Outcome::Signalled(2)),
        "hue: signalled 2 rooms (watch for the flash; the bridge acknowledges no write)"
    );
    assert_eq!(
        line(&hue, &Outcome::Signalled(1)),
        "hue: signalled 1 room (watch for the flash; the bridge acknowledges no write)"
    );
    assert_eq!(
        line(&hue, &Outcome::Signalled(0)),
        "hue: FAILED, signalled no rooms \
             (no room listing from the bridge, or no configured room name matched)",
        "zero names both causes rather than choosing one, and no count claims the \
             lights actually flashed"
    );
}

#[test]
fn the_summary_counts_every_check_exactly_once() {
    let outcomes = [
        Outcome::Skipped(A_SENSOR),
        Outcome::Sent("posted HTTP 200".to_string()),
        Outcome::SentUnreported,
        Outcome::Failed("post FAILED HTTP 401".to_string()),
        Outcome::Signalled(2),
        Outcome::Signalled(0),
        Outcome::Skipped(NOT_ENABLED),
    ];
    let summarized = summary(&outcomes);
    assert_eq!(summarized, "pns doctor: 3 sent, 2 failed, 2 skipped");
    let counted: usize = summarized
        .split_whitespace()
        .filter_map(|word| word.parse::<usize>().ok())
        .sum();
    assert_eq!(
        counted,
        outcomes.len(),
        "a check that fell into no bucket is a plugin the summary lost"
    );
}

// --- the exit contract ---------------------------------------------------

#[test]
fn only_a_run_that_sent_something_and_failed_nothing_exits_zero() {
    // THE SENDS ALONE, which is what the inert pairing below holds fixed:
    // a report that could not be checked moves nothing, so every case here
    // is decided by its outcomes exactly as it was before the pairing
    // check existed.
    let no_pairing_answer = pairing_report(None, None);
    assert_eq!(
        exit_code(
            &[
                Outcome::Sent("posted HTTP 200".to_string()),
                Outcome::Skipped(NOT_ENABLED),
            ],
            &no_pairing_answer
        ),
        0
    );
    assert_eq!(
        exit_code(&[Outcome::SentUnreported], &no_pairing_answer),
        0,
        "a channel that reports no outcome was still handed the event"
    );
    assert_eq!(exit_code(&[Outcome::Signalled(3)], &no_pairing_answer), 0);
    assert_eq!(
        exit_code(
            &[
                Outcome::Sent("posted HTTP 200".to_string()),
                Outcome::Failed("post FAILED HTTP 401".to_string()),
            ],
            &no_pairing_answer
        ),
        1,
        "one failure is enough, however much else worked"
    );
    assert_eq!(
        exit_code(&[Outcome::Signalled(0)], &no_pairing_answer),
        1,
        "a pulse that reached no room reached nothing"
    );
    assert_eq!(
        exit_code(
            &[Outcome::Skipped(NOT_ENABLED), Outcome::Skipped(A_SENSOR)],
            &no_pairing_answer
        ),
        1,
        "a run with nothing to check must never report green"
    );
    assert_eq!(
        exit_code(&[], &no_pairing_answer),
        1,
        "and neither must an empty one"
    );
}
