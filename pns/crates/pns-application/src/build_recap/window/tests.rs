mod tests {
    use super::super::*;

    #[test]
    fn a_recap_window_is_two_plain_counts_in_either_order_and_nothing_else() {
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
    fn a_bound_is_a_local_date_a_local_date_time_or_a_duration_ago() {
        // A DATE IS ITS OWN MIDNIGHT in the operator's zone, which is the one
        // reading that makes `--since 2026-09-19` the whole of that day.
        assert_eq!(
            bounds(&["--since", "2026-09-19", "--until", "2026-09-20"]),
            Some((MIDNIGHT, NEXT_MIDNIGHT))
        );
        assert_eq!(
            bounds(&[
                "--since",
                "2026-09-19T08:00",
                "--until",
                "2026-09-19T08:30:45"
            ]),
            Some((EIGHT, EIGHT_THIRTY))
        );
        // A duration is counted back from now, so the same words name a
        // different window every time they are typed.
        assert_eq!(
            bounds(&["--since", "2h", "--until", "30m"]),
            Some((NOW - 7_200, NOW - 1_800))
        );
        assert_eq!(bounds(&["--since", "3d"]), Some((NOW - 259_200, NOW)));
        // AN OMITTED `--until` IS NOW, which is what makes `--since 2h` the
        // last two hours rather than half a window.
        assert_eq!(bounds(&["--since", "2026-09-19"]), Some((MIDNIGHT, NOW)));
    }

    #[test]
    fn every_recap_window_this_will_not_vouch_for_is_refused_rather_than_defaulted() {
        // A RECAP OVER A WINDOW NOBODY ASKED FOR IS WORSE THAN NONE, so there
        // is no default half and no silent fallthrough: a missing bound, a
        // bound that is not a moment this parses, a window that runs
        // backwards, a repeated flag and any word this does not serve are each
        // a refusal.
        for refused in [
            vec![],
            // A BARE COUNT IS NOT A MOMENT: `--since` takes a date, a
            // date-time or a duration, and an epoch has its own flag.
            vec!["--since", "1756499000", "--until", "1756500000"],
            // THE EPOCH PAIR IS ALL OR NOTHING: the machine that spawns this
            // writes both, so a lone half is a typo rather than a default.
            vec!["--since-epoch", "1756499000"],
            vec!["--until-epoch", "1756500000"],
            vec!["--until", "2026-09-19"],
            vec!["--since-epoch", "1756500000", "--until-epoch", "1756499000"],
            vec!["--since", "2026-09-20", "--until", "2026-09-19"],
            vec!["--since-epoch", "yesterday", "--until-epoch", "1756500000"],
            vec!["--since-epoch", "-5", "--until-epoch", "1756500000"],
            vec!["--since", "yesterday"],
            vec!["--since", "2026-9-19"],
            vec!["--since", "2026-13-19"],
            vec!["--since", "2026-09-19T08"],
            vec!["--since", "2026-09-19 08:00"],
            vec!["--since", "2h30m"],
            vec!["--since", "2"],
            vec!["--since-epoch", "1756499000", "--since-epoch", "1756499500"],
            // ONE BOUND, ONE SPELLING: an epoch and a date for the same end
            // are two windows and only one can be answered.
            vec!["--since-epoch", "1756499000", "--since", "2026-09-19"],
            vec![
                "--since-epoch",
                "1756499000",
                "--until-epoch",
                "1756500000",
                "--now",
            ],
            vec!["--since-epoch", "1756499000", "--until-epoch"],
            vec!["--since", "2026-09-19", "--until"],
            vec!["1756499000", "1756500000"],
        ] {
            assert_eq!(bounds(&refused), None, "case: {refused:?}");
        }
    }

    #[test]
    fn the_usage_names_the_two_spellings_a_bound_may_be_written_in() {
        assert!(RECAP_USAGE.contains("--since <date|date-time|duration-ago>"));
        assert!(RECAP_USAGE.contains("--since-epoch <epoch>"));
    }

    #[test]
    fn the_usage_lists_every_window_a_recap_may_name() {
        let windows = format!(
            "pns: windows: {}\n",
            pns_domain::recap::window::WINDOW_WORDS.join(", ")
        );
        assert!(RECAP_USAGE.contains(&windows), "{RECAP_USAGE}");
    }

    fn bounds(words: &[&str]) -> Option<(u64, u64)> {
        recap_bounds(
            &words
                .iter()
                .map(|word| word.to_string())
                .collect::<Vec<_>>(),
            NOW,
            zone_seven_hours_behind,
        )
    }

    /// The zone these tests run in, PINNED HERE rather than read off the
    /// machine: which second a local date starts at is the caller's answer,
    /// and the parser's own behavior is which fields it hands over.
    fn zone_seven_hours_behind(civil: LocalCivilTime) -> Option<u64> {
        match (
            civil.year,
            civil.month,
            civil.day,
            civil.hour,
            civil.minute,
            civil.second,
        ) {
            (2026, 9, 19, 0, 0, 0) => Some(MIDNIGHT),
            (2026, 9, 19, 8, 0, 0) => Some(EIGHT),
            (2026, 9, 19, 8, 30, 45) => Some(EIGHT_THIRTY),
            (2026, 9, 20, 0, 0, 0) => Some(NEXT_MIDNIGHT),
            _ => None,
        }
    }

    /// A now well past every moment below, so a duration ago stays positive.
    const NOW: u64 = 1_790_000_000;
    /// 2026-09-19T00:00:00 in the zone above.
    const MIDNIGHT: u64 = 1_789_801_200;
    /// 2026-09-19T08:00:00 there.
    const EIGHT: u64 = 1_789_830_000;
    /// 2026-09-19T08:30:45 there.
    const EIGHT_THIRTY: u64 = 1_789_831_845;
    /// 2026-09-20T00:00:00 there.
    const NEXT_MIDNIGHT: u64 = 1_789_887_600;
}
