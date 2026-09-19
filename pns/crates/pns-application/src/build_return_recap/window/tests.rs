mod tests {
    use super::super::*;

    #[test]
    fn a_recap_window_is_two_plain_counts_in_either_order_and_nothing_else() {
        let bounds = |words: &[&str]| {
            recap_bounds(
                &words
                    .iter()
                    .map(|word| word.to_string())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            bounds(&["--since-epoch", "1756499000", "--until-epoch", "1756500000"]),
            Some((1_756_499_000, 1_756_500_000))
        );
        // Either order, because the spawner writes one and a hand run writes
        // whichever it likes.
        assert_eq!(
            bounds(&["--until-epoch", "1756500000", "--since-epoch", "1756499000"]),
            Some((1_756_499_000, 1_756_500_000))
        );
        // A window of one instant is a window: nothing happened in it, and the
        // body says so rather than the parser refusing to describe it.
        assert_eq!(
            bounds(&["--since-epoch", "5", "--until-epoch", "5"]),
            Some((5, 5))
        );
    }

    #[test]
    fn every_recap_window_this_will_not_vouch_for_is_refused_rather_than_defaulted() {
        // A RECAP OVER A WINDOW NOBODY ASKED FOR IS WORSE THAN NONE, so there
        // is no default half and no silent fallthrough: a missing bound, a
        // bound that is not a plain count, a window that runs backwards, a
        // repeated flag and any word this does not serve are each a refusal.
        let bounds = |words: &[&str]| {
            recap_bounds(
                &words
                    .iter()
                    .map(|word| word.to_string())
                    .collect::<Vec<_>>(),
            )
        };
        for refused in [
            vec![],
            // THE OLD SPELLING IS A WORD LIKE ANY OTHER NOW: `--since` and
            // `--until` say nothing about a point in time, so they refuse
            // exactly like an unknown flag.
            vec!["--since", "1756499000", "--until", "1756500000"],
            vec!["--since-epoch", "1756499000"],
            vec!["--until-epoch", "1756500000"],
            vec!["--since-epoch", "1756500000", "--until-epoch", "1756499000"],
            vec!["--since-epoch", "yesterday", "--until-epoch", "1756500000"],
            vec!["--since-epoch", "-5", "--until-epoch", "1756500000"],
            vec!["--since-epoch", "1756499000", "--since-epoch", "1756499500"],
            vec![
                "--since-epoch",
                "1756499000",
                "--until-epoch",
                "1756500000",
                "--now",
            ],
            vec!["--since-epoch", "1756499000", "--until-epoch"],
            vec!["1756499000", "1756500000"],
        ] {
            assert_eq!(bounds(&refused), None, "case: {refused:?}");
        }
    }
}
