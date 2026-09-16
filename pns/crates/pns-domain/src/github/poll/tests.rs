use super::*;

const NOW: u64 = 1_700_000_000;

fn answer(identities: &[&str]) -> Answer {
    Answer {
        identities: identities.iter().map(|word| (*word).to_string()).collect(),
        last_modified: "Thu, 25 Oct 2026 15:16:27 GMT".to_string(),
        interval_secs: Some(60),
    }
}

fn seen(state: &PollState) -> Vec<&str> {
    state
        .seen
        .iter()
        .map(|seen| seen.identity.as_str())
        .collect()
}

#[test]
fn a_first_answer_reports_every_identity_oldest_first() {
    // THE API ANSWERS NEWEST FIRST, and a channel is read top down: the run
    // that finished first belongs at the top.
    let (fresh, state) = advance(
        &PollState::default(),
        &answer(&["newest", "middle", "oldest"]),
        NOW,
    );
    assert_eq!(fresh, vec!["oldest", "middle", "newest"]);
    assert_eq!(seen(&state), vec!["oldest", "middle", "newest"]);
}

#[test]
fn an_identity_already_reported_is_not_reported_again() {
    // THE MUTANT THIS PINS: the seen-set dropped. Notifications are never
    // marked read, so every unread thread comes back on every poll forever;
    // without this the operator gets the same card once a minute.
    let (_, first) = advance(&PollState::default(), &answer(&["a", "b"]), NOW);
    let (fresh, second) = advance(&first, &answer(&["c", "a", "b"]), NOW + 60);
    assert_eq!(fresh, vec!["c"], "only the one nobody has seen");
    assert_eq!(seen(&second), vec!["b", "a", "c"]);
}

#[test]
fn the_same_answer_twice_reports_nothing_the_second_time() {
    let (_, first) = advance(&PollState::default(), &answer(&["a", "b"]), NOW);
    let (fresh, _) = advance(&first, &answer(&["a", "b"]), NOW + 60);
    assert!(fresh.is_empty(), "{fresh:?}");
}

#[test]
fn one_answer_listing_the_same_identity_twice_reports_it_once() {
    // A duplicate inside ONE listing, which paging across a boundary can
    // produce: the filter has to read what it has already taken from this
    // very answer, not only what the durable state held.
    let (fresh, state) = advance(&PollState::default(), &answer(&["a", "a", "b"]), NOW);
    assert_eq!(fresh, vec!["b", "a"]);
    assert_eq!(seen(&state), vec!["b", "a"]);
}

#[test]
fn an_identity_past_the_window_is_forgotten_and_reported_once_more() {
    // THE MUTANT THIS PINS: no expiry, which is a state file that grows
    // without bound and is read on every tick.
    let (_, first) = advance(&PollState::default(), &answer(&["a"]), NOW);
    let (nothing, _) = advance(&first, &answer(&["a"]), NOW + SEEN_FOR_SECS - 1);
    assert!(nothing.is_empty(), "inside the window it is still known");
    let (again, state) = advance(&first, &answer(&["a"]), NOW + SEEN_FOR_SECS);
    assert_eq!(again, vec!["a"], "at the window it is forgotten");
    assert_eq!(
        state.seen.first().map(|seen| seen.first_seen),
        Some(NOW + SEEN_FOR_SECS),
        "and remembered again from now, not from when it was first seen"
    );
}

#[test]
fn the_state_never_holds_more_identities_than_the_ceiling() {
    // The window is a promise about TIME; this is the one about SIZE, which a
    // storm inside one day would otherwise break.
    let many: Vec<String> = (0..SEEN_MAX + 50).map(|n| format!("id-{n}")).collect();
    let listed: Vec<&str> = many.iter().map(String::as_str).collect();
    let (fresh, state) = advance(&PollState::default(), &answer(&listed), NOW);
    assert_eq!(fresh.len(), SEEN_MAX + 50, "every one of them is reported");
    assert_eq!(state.seen.len(), SEEN_MAX);
    // `many` is listed the way the API lists, newest first, so `id-2049` is
    // the oldest and the first reported. The oldest REPORTED are what the
    // ceiling drops, which leaves the newest arrivals deduplicated.
    assert_eq!(
        state.seen.first().map(|seen| seen.identity.as_str()),
        Some("id-1999")
    );
    assert_eq!(
        state.seen.last().map(|seen| seen.identity.as_str()),
        Some("id-0")
    );
}

// --- the cursor and the interval ------------------------------------------

#[test]
fn the_cursor_and_the_interval_are_taken_from_the_answer() {
    let (_, state) = advance(&PollState::default(), &answer(&[]), NOW);
    assert_eq!(state.last_modified, "Thu, 25 Oct 2026 15:16:27 GMT");
    assert_eq!(state.interval_secs, 60);
}

#[test]
fn an_answer_stating_no_cursor_keeps_the_one_already_held() {
    // THE MUTANT THIS PINS: the cursor overwritten with an empty string,
    // which turns every later tick into a full listing and spends the rate
    // limit the 304 exists to save.
    let (_, first) = advance(&PollState::default(), &answer(&["a"]), NOW);
    let (_, second) = advance(
        &first,
        &Answer {
            identities: vec!["b".to_string()],
            last_modified: String::new(),
            interval_secs: None,
        },
        NOW + 60,
    );
    assert_eq!(second.last_modified, "Thu, 25 Oct 2026 15:16:27 GMT");
    assert_eq!(second.interval_secs, 60, "and so is the interval");
}

#[test]
fn a_server_raising_its_interval_is_obeyed() {
    // "In times of high server load, the time may increase. Please obey the
    // header."
    let (_, state) = advance(
        &PollState::default(),
        &Answer {
            interval_secs: Some(300),
            ..answer(&[])
        },
        NOW,
    );
    assert_eq!(state.interval_secs, 300);
}
