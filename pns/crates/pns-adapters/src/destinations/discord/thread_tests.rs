//! One thread per (session, channel) pair: opening it, reusing it, and
//! recovering when it is gone.

use super::double::*;
use super::*;
use pns_domain::retry::DeliveryOutcome;

const SESSION: &str = "0199c0de-face-7000-8000-abcdef012345";

/// An event of a session, which is what earns a thread.
fn session_event() -> Event {
    Event {
        session: SESSION.to_string(),
        ..event()
    }
}

/// A message post that succeeds and answers the id a thread opens on, then a
/// thread creation that succeeds and answers the thread's own id.
fn opens_a_thread(message_id: &str, thread_id: &str) -> Vec<(DeliveryOutcome, String)> {
    vec![
        (DeliveryOutcome::Status(200), object(message_id)),
        (DeliveryOutcome::Status(201), object(thread_id)),
    ]
}

fn armed_with(
    answers: Vec<(DeliveryOutcome, String)>,
    threads: &Remembered,
) -> DiscordChannel<Recorder> {
    holding(
        Recorder::scripted(answers),
        "",
        channels(&[("default", "9001"), ("dotfiles", "dotfiles-dev")]),
        threads.clone(),
    )
}

#[test]
fn the_first_event_of_a_pair_posts_a_message_and_opens_a_thread_on_it() {
    // THE MUTANT THIS PINS: the thread creation dropped, so a session posts
    // every event at channel level forever and the pair never gets a row.
    let threads = Remembered::default();
    let channel = armed_with(opens_a_thread("m-1", "t-1"), &threads);
    assert!(matches!(
        delivered_about(&channel, &session_event()),
        Delivery::Delivered(_)
    ));
    let seen = channel.post.seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "one message, then one thread");
    assert!(
        seen[1].url.ends_with("/messages/m-1/threads"),
        "the thread opens on the message that just landed"
    );
    let opened: serde_json::Value = serde_json::from_str(&seen[1].body).expect("JSON");
    assert_eq!(opened["name"], "dotfiles · feat/x · blocked");
    assert_eq!(opened["auto_archive_duration"], 1440);
    assert_eq!(
        threads.rows(),
        vec![(
            (SESSION.to_string(), "dotfiles-dev".to_string()),
            "t-1".to_string()
        )]
    );
}

#[test]
fn a_later_event_of_the_same_pair_posts_into_the_stored_thread_and_opens_nothing() {
    // THE MUTANT THIS PINS: the stored row ignored, which opens a fresh
    // thread per event and buries the channel in one-message threads.
    let threads = Remembered::default();
    threads.remember(SESSION, "dotfiles-dev", "t-1");
    let channel = armed_with(Vec::new(), &threads);
    assert!(matches!(
        delivered_about(&channel, &session_event()),
        Delivery::Delivered(_)
    ));
    assert_eq!(channel.post.seen.lock().unwrap().len(), 1, "no second call");
    assert_eq!(addressed(&channel, 0), "t-1");
}

#[test]
fn the_same_session_in_a_second_channel_opens_a_second_thread() {
    // THE MUTANT THIS PINS: the row keyed on the session alone, which posts a
    // second repository's events into the first repository's channel.
    let threads = Remembered::default();
    threads.remember(SESSION, "dotfiles-dev", "t-1");
    let mut elsewhere = session_event();
    elsewhere.project = "netpulse".to_string();
    let channel = armed_with(opens_a_thread("m-2", "t-2"), &threads);
    assert!(matches!(
        delivered_about(&channel, &elsewhere),
        Delivery::Delivered(_)
    ));
    assert_eq!(addressed(&channel, 0), "9001", "the catch-all channel");
    assert_eq!(
        threads.rows(),
        vec![
            ((SESSION.to_string(), "9001".to_string()), "t-2".to_string()),
            (
                (SESSION.to_string(), "dotfiles-dev".to_string()),
                "t-1".to_string()
            ),
        ]
    );
}

#[test]
fn a_thread_that_is_gone_is_dropped_and_reopened_before_the_verdict_is_answered() {
    // THE MUTANT THIS PINS: the refusal handed up as it stands, which
    // dead-letters every later event of a session whose thread somebody
    // deleted or a moderator locked. The verdict `deliver` answers is what
    // the ledger records, so a Delivered here IS the proof that the recovery
    // happened before anything was written.
    for code in [10003, 50083, 160005] {
        let threads = Remembered::default();
        threads.remember(SESSION, "dotfiles-dev", "t-gone");
        let mut answers = vec![(DeliveryOutcome::Status(404), refusal(code))];
        answers.extend(opens_a_thread("m-3", "t-3"));
        let channel = armed_with(answers, &threads);
        let verdict = delivered_about(&channel, &session_event());
        assert!(
            matches!(verdict, Delivery::Delivered(_)),
            "code {code} delivered: {verdict:?}"
        );
        assert_eq!(addressed(&channel, 0), "t-gone", "the stored thread first");
        assert_eq!(addressed(&channel, 1), "dotfiles-dev", "then the channel");
        assert_eq!(
            threads.rows(),
            vec![(
                (SESSION.to_string(), "dotfiles-dev".to_string()),
                "t-3".to_string()
            )],
            "code {code} replaced the dropped row"
        );
    }
}

#[test]
fn a_recap_posts_to_the_channel_and_stores_no_row() {
    // THE MUTANT THIS PINS: the recap threaded like an event, which buries
    // the one message a day the operator most wants at channel level.
    let threads = Remembered::default();
    let mut recap = session_event();
    recap.state = "recap".to_string();
    let channel = armed_with(opens_a_thread("m-4", "t-4"), &threads);
    assert!(matches!(
        delivered_about(&channel, &recap),
        Delivery::Delivered(_)
    ));
    assert_eq!(channel.post.seen.lock().unwrap().len(), 1, "no thread call");
    assert!(threads.rows().is_empty());
}

#[test]
fn an_event_with_no_session_posts_to_the_channel() {
    // THE MUTANT THIS PINS: a thread opened per event on an empty session
    // key, which collects every sessionless event of every project in one
    // thread.
    let threads = Remembered::default();
    let channel = armed_with(opens_a_thread("m-5", "t-5"), &threads);
    assert!(matches!(
        delivered_about(&channel, &event()),
        Delivery::Delivered(_)
    ));
    assert_eq!(channel.post.seen.lock().unwrap().len(), 1, "no thread call");
    assert!(threads.rows().is_empty());
}
