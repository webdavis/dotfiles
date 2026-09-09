use super::*;

// --- the moshi pairing check ---------------------------------------------

#[test]
fn a_pairing_built_from_no_answer_claims_neither_paired_nor_unpaired() {
    let report = pairing_report(None, None);
    assert_eq!(
        report.pairing,
        Pairing::NoAnswer,
        "no answer is its own state, never a guess at one"
    );
    assert_eq!(report.server, None, "and there is nothing to relay either");
}

/// `moshi-hook status --json` on this machine, moshi-hook 0.3.3, healthy.
///
/// The three values the capture elided are elided here too (`hooks`,
/// `logPath`, `socketPath`): NOTHING READS THEM, and `hooks` in particular
/// is deliberately out of scope, because on this machine it reports the
/// claude and codex hooks as stale BY DESIGN under the single-submitter
/// rule, so a check that graded it would page a permanent false alarm.
pub(super) const PAIRED_JSON: &str = r#"{"baseUrl":"https://api.getmoshi.app/api/v1",
        "displayName":"dresden","hooks":[],
        "hostId":"host_b14dd2bb0b1f45899d9eaa81a71ff874","logPath":"...",
        "paired":true,"platform":"macos","secretStore":"keychain","socketPath":"..."}"#;

/// The same call measured with `HOME` pointed at an empty directory: the
/// answer is `paired: false` and carries no host id at all.
pub(super) const UNPAIRED_JSON: &str = r#"{"baseUrl":"https://api.getmoshi.app/api/v1","hooks":[],
        "logPath":"...","paired":false,"platform":"macos","secretStore":"keychain",
        "socketPath":"..."}"#;

#[test]
fn a_paired_answer_carries_back_the_host_id_and_display_name_moshi_named() {
    assert_eq!(
        pairing_report(Some(PAIRED_JSON), None).pairing,
        Pairing::Paired {
            host_id: "host_b14dd2bb0b1f45899d9eaa81a71ff874".to_string(),
            display_name: "dresden".to_string(),
        },
        "both come back VERBATIM: a doctor that abbreviated moshi's own \
             identifiers would be a second spelling of one answer, and the host \
             id is the thing an operator compares against the phone"
    );

    // A SHAPE NOBODY HAS SEEN. Every measured `paired: true` carries both
    // names, so this is the fallback rather than a case to design around,
    // and it says the identifier is missing instead of rendering an empty
    // parenthesis the operator would read as a host id they misread.
    assert_eq!(
        pairing_report(Some(r#"{"paired":true}"#), None).pairing,
        Pairing::Paired {
            host_id: "not reported".to_string(),
            display_name: "not reported".to_string(),
        }
    );
}

#[test]
fn an_unpaired_answer_is_unpaired_rather_than_unreadable() {
    assert_eq!(
        pairing_report(Some(UNPAIRED_JSON), None).pairing,
        Pairing::Unpaired,
        "an answer naming no host is still an ANSWER: reading it as \
             unreadable would make the one state that earns an exit 1 inert"
    );
}

#[test]
fn json_that_will_not_parse_or_names_no_paired_key_claims_neither() {
    for answer in [
        "",
        "not json at all",
        "{",
        r#"{"displayName":"dresden"}"#,
        r#"{"paired":"yes"}"#,
    ] {
        assert_eq!(
            pairing_report(Some(answer), None).pairing,
            Pairing::Unreadable,
            "answer: {answer:?}"
        );
    }
}

/// `moshi-hook status` (plain), healthy, on this machine. This shape is the
/// only one carrying a server verdict at all: the JSON answer above is
/// local-only and measured to perform no network I/O.
pub(super) const PAIRED_PLAIN: &str = "status:       paired\n\
         host id:      host_b14dd2bb0b1f45899d9eaa81a71ff874\n\
         display name: dresden\n\
         server:       Moshi Pro attached (usage scope: license)\n";

#[test]
fn the_server_line_is_relayed_as_moshis_own_words_with_the_label_removed() {
    assert_eq!(
        pairing_report(Some(PAIRED_JSON), Some(PAIRED_PLAIN))
            .server
            .as_deref(),
        Some("Moshi Pro attached (usage scope: license)"),
        "moshi's own sentence, VERBATIM. pns has no stable way to tell this \
             apart from a host that does not belong to the user token, and any \
             match on the prose would fail in the dangerous direction the day \
             moshi rewords it: a healthy machine failing its doctor, or a real \
             break going unreported"
    );
}

#[test]
fn only_a_server_line_at_column_zero_is_relayed() {
    // moshi's own output indents continuation and detail lines, so a
    // relay anchored on a substring would quote whichever of them said
    // the word first and attribute it to the server.
    let indented_first = "status:       paired\n  server: an indented line\n\
             server:       the server line\n";
    assert_eq!(
        pairing_report(None, Some(indented_first)).server.as_deref(),
        Some("the server line"),
        "the label is a line PREFIX, never a substring anywhere in the line"
    );
    let only_indented = "status:       paired\n  server: an indented line\n";
    assert_eq!(
        pairing_report(None, Some(only_indented)).server,
        None,
        "and an indented line alone is no server verdict at all"
    );
}

/// The label the relayed line carries, which is how the report attributes
/// the sentence to moshi rather than to pns.
pub(super) const MOSHI_SAYS: &str = "moshi says";

#[test]
fn plain_output_with_no_server_line_relays_nothing_rather_than_an_empty_line() {
    // AN UNPAIRED HOST PRINTS NO `server:` LINE AT ALL, measured, and a
    // future moshi that renamed or dropped the line would print none
    // either. That degradation is the SAFE direction: no relay, and
    // nothing else about the report moves.
    let unpaired_plain = "status:       unpaired\n";
    let report = pairing_report(Some(PAIRED_JSON), Some(unpaired_plain));
    assert_eq!(report.server, None);
    assert!(
        !pairing_lines(&report)
            .iter()
            .any(|line| line.contains(MOSHI_SAYS)),
        "a relay with nothing to relay is an absent line, never a labelled \
             blank one: {:?}",
        pairing_lines(&report)
    );

    // And a label with nothing after it is not a verdict either.
    let empty_value = "status:       paired\nserver:       \n";
    let report = pairing_report(Some(PAIRED_JSON), Some(empty_value));
    assert_eq!(report.server, None);
    assert!(
        !pairing_lines(&report)
            .iter()
            .any(|line| line.contains(MOSHI_SAYS)),
        "{:?}",
        pairing_lines(&report)
    );
}
