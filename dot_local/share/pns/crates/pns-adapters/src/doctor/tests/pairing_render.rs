use super::pairing_read::{PAIRED_JSON, PAIRED_PLAIN, UNPAIRED_JSON};
use super::*;

/// What a relayed line is addressed as, so a test can strip it back off.
const RELAY_OPENING: &str = "pns doctor: moshi says: ";

#[test]
fn a_relayed_value_carrying_a_newline_or_a_control_byte_cannot_forge_a_report_line() {
    // THE WHOLE POINT OF THE FILTER. This is third-party text going
    // straight to a terminal, and an unfiltered newline in it would print
    // a second `pns doctor:` line that the operator would read as pns's
    // own verdict. A report that can be made to lie about itself is worse
    // than no relay at all.
    let forged = PairingReport {
        pairing: Pairing::Unpaired,
        server: Some(
            "attached\npns doctor: 9 sent, 0 failed, 0 skipped\r\u{1b}[2Kok\u{7}".to_string(),
        ),
    };
    let lines = pairing_lines(&forged);
    assert_eq!(lines.len(), 2, "the relay forged a line: {lines:?}");
    assert_eq!(
        lines[1], "pns doctor: moshi says: attachedpns doctor: 9 sent, 0 failed, 0 skipped[2Kok",
        "the newline, the carriage return, the escape and the bell are all \
             gone, and what is left is visibly inside one relayed line"
    );

    // AND THE SAME THROUGH THE READING PATH. A carriage return is the one
    // that survives being split into lines, and on a terminal it returns
    // the cursor to column zero for whatever follows to overwrite the
    // report's own prefix with.
    let read = pairing_report(None, Some("server:       up\rpns doctor: forged\n"));
    let lines = pairing_lines(&read);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        !lines[1].contains('\r'),
        "a carriage return reached the terminal: {:?}",
        lines[1]
    );
}

#[test]
fn an_identity_moshi_named_cannot_forge_a_report_line_either() {
    // THE IDENTITY FIELDS ARE THIRD-PARTY TEXT TOO, and they reach the
    // same terminal by the same route. MEASURED: a `displayName` carrying
    // a newline and a forged summary printed
    // `pns doctor: 9 sent, 0 failed, 0 skipped` as its own line inside the
    // real report, which is the exact forgery the relay filter exists to
    // stop one field to the left.
    let forged = "{\"paired\":true,\
             \"displayName\":\"dresd\u{e9}n\\npns doctor: 9 sent, 0 failed, 0 skipped\\r\\u001b[2K\",\
             \"hostId\":\"host_1\\nforged\"}";
    let lines = pairing_lines(&pairing_report(Some(forged), None));

    assert_eq!(
        lines.iter().flat_map(|line| line.lines()).count(),
        1,
        "an identity forged a report line: {lines:?}"
    );
    assert_eq!(
        lines[0],
        "pns doctor: moshi pairing: paired as dresdnpns doctor: 9 sent, 0 failed, \
             0 skipped[2K (host_1forged).",
        "BOTH fields go through the SAME filter the relayed sentence does: \
             the newline, the carriage return and the escape are gone, the \
             non-ASCII character is gone with them, and what is left is \
             visibly inside the one line pns wrote"
    );
    assert!(
        !lines[0].chars().any(char::is_control),
        "a control byte reached the terminal: {:?}",
        lines[0]
    );
}

#[test]
fn an_over_long_relayed_value_stops_at_the_cap() {
    let report = PairingReport {
        pairing: Pairing::Unpaired,
        server: Some("x".repeat(500)),
    };
    let lines = pairing_lines(&report);
    let relayed = lines[1]
        .strip_prefix(RELAY_OPENING)
        .unwrap_or_else(|| panic!("{:?}", lines[1]));
    assert_eq!(
        relayed.chars().count(),
        200,
        "an unbounded relay is an unbounded line in somebody else's report"
    );

    // COUNTED IN CHARACTERS AND FILTERED FIRST, so the cap can never land
    // inside a multi-byte sequence: a character outside printable ASCII is
    // gone before anything is counted.
    let multibyte = PairingReport {
        pairing: Pairing::Unpaired,
        server: Some("\u{e9}".repeat(300)),
    };
    assert_eq!(
        pairing_lines(&multibyte).len(),
        1,
        "nothing printable survived, so there is nothing to relay"
    );
}

#[test]
fn the_paired_line_names_the_host_and_claims_nothing_about_approvals() {
    let lines = pairing_lines(&pairing_report(Some(PAIRED_JSON), Some(PAIRED_PLAIN)));
    assert_eq!(
        lines,
        [
            "pns doctor: moshi pairing: paired as dresden \
                 (host_b14dd2bb0b1f45899d9eaa81a71ff874).",
            "pns doctor: moshi says: Moshi Pro attached (usage scope: license)",
        ]
    );
    // IT SAYS WHO THIS HOST IS PAIRED AS AND STOPS THERE. A re-pair mints
    // a new host id while the live daemon keeps serving the old one, and
    // an approval only really round trips when a human taps a card:
    // neither is visible from here, so neither may be implied.
    for overclaim in ["approvals work", "working", "will reach", "healthy"] {
        assert!(
            !lines[0].contains(overclaim),
            "the line claims {overclaim:?}, which this check cannot see: {:?}",
            lines[0]
        );
    }
}

#[test]
fn the_unpaired_line_says_the_cards_are_dead_and_names_the_command_that_fixes_it() {
    let lines = pairing_lines(&pairing_report(Some(UNPAIRED_JSON), None));
    assert_eq!(
        lines,
        [
            "pns doctor: moshi pairing: this host is NOT paired, so every \
              approval card is dead until `moshi-hook pair` runs."
        ],
        "the remedy is IN THE LINE: this is the state the whole check \
             exists for, and the census reports the moshi channel green over \
             its webhook the entire time"
    );
}

#[test]
fn the_no_answer_line_offers_both_explanations_and_commits_to_neither() {
    let lines = pairing_lines(&pairing_report(None, None));
    assert_eq!(
        lines,
        ["pns doctor: moshi pairing: moshi-hook did not answer (not \
              installed, or it did not answer in time), so the approval path \
              could not be checked."],
        "the bounded spawn cannot tell an absent binary from one that hung \
             or exited non-zero, so the line names two explanations and picks \
             neither"
    );

    // The fourth state, and the last one with a line of its own: moshi
    // answered, and the answer was a shape this does not recognize.
    assert_eq!(
        pairing_lines(&pairing_report(Some("{"), None)),
        [
            "pns doctor: moshi pairing: moshi-hook answered something this \
              cannot read."
        ]
    );
}

#[test]
fn an_unpaired_host_alone_earns_the_exit_code_a_one() {
    // THE JUDGEMENT CALL. It only fires on a machine moshi-hook is
    // installed and answering on, which is a machine that set moshi up,
    // and on one of those an unregistered host means every approval card
    // is dead while the census reports the moshi channel green over its
    // webhook. That gap is the entire reason this check exists apart from
    // the census.
    let every_send_green = [Outcome::Sent("posted HTTP 200".to_string())];
    assert_eq!(
        exit_code(&every_send_green, &pairing_report(Some(PAIRED_JSON), None)),
        0,
        "the control: the same sends with a healthy pairing"
    );
    assert_eq!(
        exit_code(
            &every_send_green,
            &pairing_report(Some(UNPAIRED_JSON), None)
        ),
        1,
        "the pairing ALONE moved it, with nothing else changed"
    );
}

#[test]
fn a_no_answer_or_unreadable_pairing_leaves_a_green_run_exiting_zero() {
    // A MACHINE THAT DOES NOT USE MOSHI MUST NOT FAIL ITS DOCTOR FOREVER,
    // and neither must one whose moshi answered a shape this cannot read:
    // both are "could not check", and a check that could not run is not a
    // failure it found.
    let every_send_green = [Outcome::Sent("posted HTTP 200".to_string())];
    for could_not_check in [pairing_report(None, None), pairing_report(Some("{"), None)] {
        assert_eq!(
            exit_code(&every_send_green, &could_not_check),
            0,
            "{could_not_check:?}"
        );
    }
}

#[test]
fn a_failed_send_still_exits_one_when_the_pairing_is_healthy() {
    // NEITHER READER OVERRIDES THE OTHER. A healthy pairing cannot mask a
    // send that failed, and it cannot turn a run with nothing to check
    // green either.
    let healthy = pairing_report(Some(PAIRED_JSON), Some(PAIRED_PLAIN));
    assert_eq!(
        exit_code(
            &[
                Outcome::Sent("posted HTTP 200".to_string()),
                Outcome::Failed("post FAILED HTTP 401".to_string()),
            ],
            &healthy
        ),
        1
    );
    assert_eq!(
        exit_code(&[Outcome::Skipped(NOT_ENABLED)], &healthy),
        1,
        "a run with nothing to check must never report green, whatever \
             the pairing says"
    );
}
