mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;

    #[test]
    fn a_held_record_that_is_absent_holds_nothing_and_one_that_will_not_read_holds_everything() {
        // TWO DIFFERENT FACTS, and collapsing them into an empty list is what
        // let a blink write straight over a lamp that was breathing. The
        // ORDINARY case is a machine holding nothing at all, which is an absent
        // file; a file that exists and cannot be read says nothing about which
        // lamps are held, and the gate that reads it decides whether a pulse
        // fires over one.
        let state = scratch("held-record-absent-or-unreadable");
        assert_eq!(
            held_lamps(&state),
            Some(Vec::new()),
            "no file at all is a house holding nothing"
        );
        std::fs::create_dir(state.join(LIGHTS_HELD)).expect("a directory where the record goes");
        assert_eq!(
            held_lamps(&state),
            None,
            "and one nobody can read is unknown"
        );
    }

    #[test]
    fn a_held_records_phase_round_trips_through_remember_held_and_read_held() {
        // ONE PARSER, ONE RENDERER, so a phase written by `remember_held`
        // reads back exactly through `read_held`, and `held_lamps` (the three
        // bare-path consumers' own read) sees the same path with the phase
        // silently dropped.
        let state = scratch("held-record-phase-round-trip");
        let phased = pns::lights::HeldEntry {
            path: LAMP_PATH.to_string(),
            resume: Some(pns::lights::Phase {
                end_unix_ms: 1_700_000_000_123,
                landed_on: 100,
                held: pns::lights::Held::Blocked,
            }),
        };
        remember_held(&state, std::slice::from_ref(&phased)).expect("the write lands");
        assert_eq!(
            read_held(&state),
            Some(vec![phased]),
            "the phase round-trips through the same file"
        );
        assert_eq!(
            held_lamps(&state),
            Some(vec![LAMP_PATH.to_string()]),
            "and the bare consumers see only the path"
        );
    }

    #[test]
    fn a_bare_token_on_disk_still_reads_as_a_held_lamp_with_no_phase() {
        // THE FORMAT A HAND-WRITTEN OR OLDER-BUILD RECORD USES, and every test
        // above that writes `LAMP_PATH\n` directly to the file: a bare token
        // is a lamp this record holds with no phase, never an unreadable
        // record.
        let state = scratch("held-record-bare-token");
        std::fs::write(state.join(LIGHTS_HELD), format!("{LAMP_PATH}\n")).expect("the record");
        assert_eq!(
            read_held(&state),
            Some(vec![pns::lights::HeldEntry::bare(LAMP_PATH)])
        );
        assert_eq!(held_lamps(&state), Some(vec![LAMP_PATH.to_string()]));
    }
    #[test]
    fn a_complaint_that_cleared_is_forgotten_so_its_return_is_news_again() {
        // THE FORGET ARM IS THE ONE THAT NEEDS ITS OWN PIN: `say` decides it,
        // but only this wiring removes the memory, and a memory that outlives
        // its complaint keeps the same complaint silent when it comes back.
        let state = scratch("lights-said-forget");
        let marker = state.join(LIGHTS_SAID);
        say_lights_once(
            &state,
            &["lights: `HCL9` (lamp) is not on the bridge".to_string()],
            LIGHTS_SAID,
        );
        assert!(marker.exists(), "the first complaint is remembered");
        say_lights_once(&state, &[], LIGHTS_SAID);
        assert!(
            !marker.exists(),
            "a clear tick forgets, or the same complaint returning would never \
             be said again"
        );
    }
    #[test]
    fn the_news_record_is_written_for_a_finished_or_a_dead_turn_and_read_back_as_it_was() {
        // THE WIRING, not the rule. `unread_arming` is pure and total and has no
        // file of its own, so a record invented at the call site leaves every one
        // of its unit tests green while the lamp never arms on a real machine.
        // This is the seam that costs the whole state, pinned against real files.
        let state = scratch("news-record");
        assert_eq!(
            read_news(&state),
            pns::lights::News::default(),
            "a machine that has seen nothing yet has no news"
        );
        record_news(&state, pns::config::Behaviour::Done, Some(1_000));
        assert_eq!(
            read_news(&state),
            pns::lights::News {
                done_at: Some(1_000),
                failed_at: None
            },
        );
        record_news(&state, pns::config::Behaviour::Failed, Some(1_200));
        assert_eq!(
            read_news(&state),
            pns::lights::News {
                done_at: Some(1_000),
                failed_at: Some(1_200)
            },
            "the second kind moves its own epoch and leaves the first where it was"
        );
        record_news(&state, pns::config::Behaviour::Blocked, Some(1_400));
        assert_eq!(
            read_news(&state).done_at,
            Some(1_000),
            "and a wait is not news, so it changes nothing"
        );
        // AND THE RECORD IS TAKEN BY RENAME TO MERGE IT, so two runs recording
        // at once cannot each publish a whole line built from the same stale
        // read. What that leaves behind is nothing: a claim outliving its run
        // would be a second file holding a stale copy nothing reads.
        assert_eq!(
            std::fs::read_dir(&state)
                .expect("the state directory")
                .flatten()
                .filter(|entry| entry.file_name().to_string_lossy().contains(".claim."))
                .count(),
            0,
            "a claim was left behind in {}",
            state.display()
        );
        // FAIL TO DARK. A record some other hand rewrote arms no lamp rather
        // than arming one about news nobody can name.
        std::fs::write(state.join(LIGHTS_NEWS), "not a record\n").expect("a garbled record");
        assert_eq!(read_news(&state), pns::lights::News::default());
        // AND A CLOCK NOBODY CAN READ WRITES NOTHING, never an epoch of zero:
        // zero is 1970, which is older than every interaction there has been.
        std::fs::remove_file(state.join(LIGHTS_NEWS)).expect("the record goes");
        record_news(&state, pns::config::Behaviour::Done, None);
        assert!(!state.join(LIGHTS_NEWS).exists());
    }
}
