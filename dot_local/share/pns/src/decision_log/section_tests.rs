use super::fixtures::*;

#[test]
fn a_section_over_no_contents_says_no_decision_has_been_recorded_and_names_nothing() {
    // THE PARENTHESIS IS THE HONEST HALF. The write is fail-quiet, so an
    // absent log cannot be told from an unused one, and a line claiming
    // "no event has run" alone would be a guess presented as a finding.
    assert_eq!(
        section(None, Some(1_756_500_000)),
        vec![
            "pns doctor: no decision has been recorded yet (no event has run since this \
             was installed, or none could be written)."
        ]
    );
    // A FILE THAT EXISTS AND HOLDS NOTHING is the same state and says the
    // same thing rather than printing an empty heading over no entries.
    assert_eq!(section(Some(""), None), section(None, None));
    assert_eq!(section(Some("\n\n"), None), section(None, None));
}

/// Seven decisions, oldest first, the order an append leaves them in.
const SEVEN: &str = "1756400000 a/one surface=Desk\n\
                     1756490000 b/two surface=Desk\n\
                     1756499000 c/three surface=Desk\n\
                     1756499900 d/four surface=Desk\n\
                     1756499970 e/five surface=Desk\n\
                     1756499990 f/six surface=Desk\n\
                     1756500000 g/seven surface=Desk\n";

const HEADING_TAIL: &str = " newest first (why a card did or did not fire). No actionId \
     is recorded: moshi mints it inside the approval round trip and never hands it back.";

#[test]
fn a_section_renders_the_newest_entry_first_capped_at_the_kept_count_with_each_ones_age() {
    // NEWEST FIRST IS THE READING ORDER: the operator came to look at the
    // card that just did or did not arrive, and the ring is written by
    // append, so the file's own order is the opposite of the useful one.
    assert_eq!(
        section(Some(SEVEN), Some(1_756_500_000)),
        vec![
            format!("pns doctor: the last {KEPT} decisions,{HEADING_TAIL}"),
            "  0s ago: g/seven surface=Desk".to_string(),
            "  10s ago: f/six surface=Desk".to_string(),
            "  30s ago: e/five surface=Desk".to_string(),
            "  1m ago: d/four surface=Desk".to_string(),
            "  16m ago: c/three surface=Desk".to_string(),
        ],
        "the two oldest are gone and the newest leads"
    );
}

#[test]
fn a_section_counts_the_entries_it_actually_shows_rather_than_the_cap() {
    // A heading claiming five over one entry would be the report inventing
    // four decisions nobody took.
    assert_eq!(
        section(
            Some("1756499000 c/three surface=Desk\n"),
            Some(1_756_500_000)
        ),
        vec![
            format!("pns doctor: the last decision,{HEADING_TAIL}"),
            "  16m ago: c/three surface=Desk".to_string(),
        ]
    );
    assert_eq!(
        section(Some(SEVEN), Some(1_756_500_000)).len(),
        KEPT + 1,
        "one heading over the kept count"
    );
}

#[test]
fn a_section_ages_an_entry_in_the_largest_unit_that_still_reads_as_a_count() {
    // Hours, because a five-deep ring on a machine used a few times a week
    // holds day-old entries, and "4320m ago" makes the reader do the
    // arithmetic the report exists to save them.
    for (recorded, expected) in [
        (1_756_499_999_u64, "1s ago"),
        (1_756_499_941, "59s ago"),
        (1_756_499_940, "1m ago"),
        (1_756_496_401, "59m ago"),
        (1_756_496_400, "1h ago"),
        (1_756_400_000, "27h ago"),
    ] {
        assert_eq!(
            section(
                Some(&format!("{recorded} a/one x=1\n")),
                Some(1_756_500_000)
            )[1],
            format!("  {expected}: a/one x=1"),
            "recorded at {recorded}"
        );
    }
}

#[test]
fn a_section_quotes_an_entry_it_cannot_read_and_still_renders_its_readable_neighbours() {
    // DROPPING IT SILENTLY is how a log loses the one entry that mattered,
    // and it is also how a truncated write disappears without a trace.
    let mixed = "1756499000 a/one surface=Desk\n\
                 no-space-anywhere\n\
                 1756499900 b/two surface=Desk\n\
                 notanepoch c/three surface=Desk\n";
    assert_eq!(
        section(Some(mixed), Some(1_756_500_000)),
        vec![
            format!("pns doctor: the last 4 decisions,{HEADING_TAIL}"),
            "  unreadable entry: \"notanepoch c/three surface=Desk\"".to_string(),
            "  1m ago: b/two surface=Desk".to_string(),
            "  unreadable entry: \"no-space-anywhere\"".to_string(),
            "  16m ago: a/one surface=Desk".to_string(),
        ]
    );
}

#[test]
fn an_unreadable_entry_is_quoted_short_and_with_its_control_bytes_escaped() {
    // The report goes to a terminal, and a file this never wrote can hold
    // anything: a hand edit, a truncated write, another program's output.
    let rendered = section(Some("\u{1b}[31mred\tand\u{7}long\n"), Some(1_756_500_000));
    assert_eq!(
        rendered[1], "  unreadable entry: \"\\u{1b}[31mred\\tand\\u{7}long\"",
        "escaped rather than executed by the terminal"
    );
    // AND BOUNDED, so a file of garbage cannot fill the report.
    let long = format!("{}\n", "z".repeat(500));
    assert_eq!(
        section(Some(&long), Some(1_756_500_000))[1],
        format!("  unreadable entry: {:?}", "z".repeat(60))
    );
}

#[test]
fn a_parsed_entrys_body_is_escaped_by_the_same_rule_an_unreadable_one_is() {
    // ONE ESCAPE RULE FOR BOTH ARMS. The body of a PARSED entry used to be
    // printed verbatim, so a hand-edited ring holding an escape sequence
    // reached the terminal raw from `pns doctor` as long as its epoch
    // parsed. The bytes are the point: what comes out is the characters
    // that spell the escape, not the escape.
    let rendered = section(
        Some("1756500000 a/one \u{1b}[31mred\u{7}\tand\u{8}back\n"),
        Some(1_756_500_000),
    );
    assert_eq!(
        rendered[1],
        "  0s ago: a/one \\u{1b}[31mred\\u{7}\\tand\\u{8}back"
    );
    for raw in ['\u{1b}', '\u{7}', '\u{8}', '\t'] {
        assert!(
            !rendered[1].contains(raw),
            "{raw:?} reached the terminal: {:?}",
            rendered[1]
        );
    }
}

#[test]
fn a_section_invents_no_age_for_an_entry_or_a_reader_that_had_no_clock() {
    // TWO DIFFERENT MISSING CLOCKS, and neither may become a number. The
    // dash is a RECOGNIZED value, so an entry written without a clock is
    // rendered as itself rather than quoted back as unreadable.
    assert_eq!(
        section(Some("- a/one surface=Away\n"), Some(1_756_500_000)),
        vec![
            format!("pns doctor: the last decision,{HEADING_TAIL}"),
            "  age unknown: a/one surface=Away".to_string(),
        ]
    );
    // THE READER'S OWN CLOCK, absent: every entry is unaged, and none is
    // dropped or complained about.
    assert_eq!(
        section(Some(SEVEN), None),
        vec![
            format!("pns doctor: the last {KEPT} decisions,{HEADING_TAIL}"),
            "  age unknown: g/seven surface=Desk".to_string(),
            "  age unknown: f/six surface=Desk".to_string(),
            "  age unknown: e/five surface=Desk".to_string(),
            "  age unknown: d/four surface=Desk".to_string(),
            "  age unknown: c/three surface=Desk".to_string(),
        ]
    );
}
