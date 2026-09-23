use super::*;

#[test]
fn the_recap_card_lists_the_waits_still_open_and_names_the_recaps_route() {
    let mut recorder = Recorder::new(Some(claim_of(Some(1_000), Vec::new())));
    recorder.entries = vec![entry(1_100), entry(1_200)];
    recorder.open = vec![pns_domain::missed::OpenWait {
        agent: "codex".to_string(),
        state: "blocked".to_string(),
        project: "dotfiles".to_string(),
        asks: "Bash: git push".to_string(),
        count: 8,
    }];
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), true);
    assert!(
        recorder
            .steps()
            .contains(&"open_waits(1000,2000)".to_string()),
        "the open waits were asked for over the absence: {:?}",
        recorder.steps()
    );
    assert_eq!(
        recorder.handed.borrow()[0].0,
        "codex · blocked · dotfiles ×8: Bash: git push. 2 events. recap in #logbook"
    );
}

#[test]
fn a_card_with_no_recap_behind_it_names_no_route() {
    let mut recorder = Recorder::new(Some(claim_of(Some(1_000), Vec::new())));
    recorder.entries = vec![entry(1_100), entry(1_200)];
    recorder.posted = false;
    ports(&recorder).run(&returning(vec![leg(true)]), policy(), true);
    assert_eq!(*recorder.delivered.borrow(), ["2 events"]);
}
