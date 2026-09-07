mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;

    #[test]
    fn an_unreadable_lights_quiet_complains_and_an_absent_one_says_nothing() {
        // THE DIFFERENCE BETWEEN "NOBODY EVER RAN THE COMMAND" AND "THIS FILE
        // CANNOT BE READ", which both readers of the ad-hoc quiet depend on:
        // every read failure mutes nothing, so the second one has to be said
        // out loud or the operator believes a mute is on while every lamp goes
        // loud at 3am.
        let state = scratch("muted-state");
        assert_eq!(
            muted_state(&state),
            (Vec::new(), Vec::new()),
            "no file at all is the ordinary case and says nothing"
        );
        let file = state.join("lights-quiet");
        std::fs::create_dir(&file).expect("a directory standing where the file goes");
        let (entries, complaints) = muted_state(&state);
        assert!(
            entries.is_empty() && complaints.len() == 1,
            "a directory mutes nothing and is complained about once: \
             {entries:?} {complaints:?}"
        );
        assert!(
            complaints[0].starts_with("pns: state error (lights-quiet could not be read:"),
            "and the complaint names the file and what went wrong: {}",
            complaints[0]
        );
        std::fs::remove_dir(&file).expect("the directory goes");
        std::fs::write(&file, [0x66, 0xff, 0xfe]).expect("bytes that are not UTF-8");
        let (entries, complaints) = muted_state(&state);
        assert!(
            entries.is_empty() && complaints.len() == 1,
            "and so does a file that is not text: {entries:?} {complaints:?}"
        );
        // AND WHAT AN UNREADABLE ONE MUTES IS EVERYTHING, which is the fail
        // direction on a lamp path and the opposite of what it used to do: a
        // record nobody can parse says nothing about which places are quiet,
        // and read as an empty list it was a house with every lamp loud.
        assert_eq!(
            ad_hoc_quiet(&state, Some(1_000)).0,
            pns::channels::hue::Muting::Everything
        );
        std::fs::write(&file, "9999999999 3F - Studio\n").expect("a file it can read");
        assert_eq!(
            muted_state(&state).1,
            Vec::<String>::new(),
            "the control: a file it can read complains about nothing"
        );
        assert_eq!(
            ad_hoc_quiet(&state, Some(1_000)),
            (
                pns::channels::hue::Muting::Places(vec!["3F - Studio".to_string()]),
                Vec::new()
            ),
            "and it mutes exactly the place the file names"
        );
        // A CLOCK THAT WILL NOT ANSWER GOES THE SAME WAY. Nothing can judge a
        // mute live without one, and the direction is dark rather than loud.
        //
        // THE LITERAL SENTENCE, never the constant: a mutation that renamed
        // or emptied `NO_CLOCK_FOR_THE_MUTE` and every reader of it together
        // would still pass a comparison against itself.
        let (muting, complaints) = ad_hoc_quiet(&state, None);
        assert_eq!(muting, pns::channels::hue::Muting::Everything);
        assert_eq!(
            complaints,
            vec![
                "pns lights: the clock cannot be read, so no mute can be judged \
                 live; every lamp is quiet until it can"
                    .to_string()
            ]
        );
    }

    #[test]
    fn only_a_word_no_declaration_accounts_for_is_worth_a_bridge_listing() {
        // THE MUTE'S VOCABULARY IS BOTH SOURCES, and the bridge half costs a
        // human three round trips while they stand at a terminal. A place the
        // config already declares can be enforced whatever the bridge says, so
        // the ordinary bedtime mute must not pay for a listing that cannot
        // change the answer.
        let declared = vec!["3F - Studio".to_string()];
        let typed = |words: &[&str]| -> Vec<String> {
            words.iter().map(|word| (*word).to_string()).collect()
        };
        assert!(!asks_the_bridge(&declared, &typed(&[])), "the bare report");
        assert!(!asks_the_bridge(&declared, &typed(&["3F - Studio"])));
        assert!(!asks_the_bridge(&declared, &typed(&["3F - Studio", "2h"])));
        assert!(
            !asks_the_bridge(&declared, &typed(&["3F - Nowhere", "off"])),
            "`off` is allowed over any name, so no listing could change it"
        );
        // AND THE ONE CASE A LISTING DECIDES: a name no declaration holds may
        // still be a real lamp, room or zone, which is the whole grammar.
        assert!(asks_the_bridge(&declared, &typed(&["3F - Studio - HCL1"])));
        assert!(asks_the_bridge(
            &declared,
            &typed(&["3F - Studio - HCL1", "2h"])
        ));
    }
}
