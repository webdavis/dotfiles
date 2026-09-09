//! The lamps, pinned: unread.

use super::fixtures::*;

// --- the news record ----------------------------------------------------

#[test]
fn the_news_record_survives_as_one_line_and_anything_else_is_no_news() {
    let both = News {
        done_at: Some(1_000),
        failed_at: Some(1_200),
    };
    assert_eq!(render_news(&both), "1000 1200");
    assert_eq!(parse_news("1000 1200"), Some(both));
    // ZERO IS "NOT YET", both ways round, so the record round-trips through
    // a state file that has only ever seen one kind of event.
    let only_done = News {
        done_at: Some(1_000),
        failed_at: None,
    };
    assert_eq!(render_news(&only_done), "1000 0");
    assert_eq!(parse_news("1000 0"), Some(only_done));
    assert_eq!(parse_news("0 0"), Some(News::default()));
    // REFUSED, NEVER GUESSED AT, and the fail direction is dark: a file some
    // other hand rewrote yields no news, so nothing arms.
    for garbled in [
        "",
        "1000",
        "1000 1200 1400",
        "x 1200",
        "1000 x",
        " 1000 1200",
    ] {
        assert_eq!(parse_news(garbled), None, "{garbled:?} is not news");
    }
}

#[test]
fn the_news_record_only_ever_moves_an_epoch_forward() {
    // TWO PROCESSES WRITE THIS RECORD, and they are two events landing
    // together: an agent that finished beside one that died. Each reads,
    // changes its own field and publishes the whole line, so the slower
    // reader can put an OLDER second back over a newer one. What that costs
    // is the unread lamp's colour: a failure recorded at the newer second
    // and then overwritten with the older one is red the lamp never shows,
    // or a success armed five minutes before it should be.
    let held = News {
        done_at: Some(2_000),
        failed_at: Some(2_100),
    };
    assert_eq!(
        news_after(held, Behaviour::Done, 1_000),
        Some(held),
        "a run publishing late leaves the newer second where it is"
    );
    assert_eq!(
        news_after(held, Behaviour::Failed, 1_000),
        Some(held),
        "and so does the other kind"
    );
    assert_eq!(
        news_after(held, Behaviour::Done, 2_000),
        Some(held),
        "the same second is not forward either, so a repeat writes nothing new"
    );
}

#[test]
fn only_a_finished_or_a_dead_turn_is_news_and_a_wait_is_not() {
    let held = News {
        done_at: Some(1_000),
        failed_at: Some(1_100),
    };
    assert_eq!(
        news_after(held, Behaviour::Done, 2_000),
        Some(News {
            done_at: Some(2_000),
            failed_at: Some(1_100)
        }),
        "a finished turn moves its own epoch and leaves the other where it was"
    );
    assert_eq!(
        news_after(held, Behaviour::Failed, 2_000),
        Some(News {
            done_at: Some(1_000),
            failed_at: Some(2_000)
        }),
        "and a dead one moves the other"
    );
    // A WAIT IS NOT NEWS. It is a question still on screen, which is the
    // blocked lamp's own business; recording it here would arm the unread
    // lamp about something nobody has missed.
    for not_news in [Behaviour::Blocked, Behaviour::Unread, Behaviour::Looping] {
        assert_eq!(
            news_after(held, not_news, 2_000),
            None,
            "{not_news:?} is not news"
        );
    }
}

// --- the unread lamp ----------------------------------------------------

const AFTER: u64 = 300;

fn news(done_ago: Option<u64>, failed_ago: Option<u64>) -> News {
    News {
        done_at: done_ago.map(|ago| NOW - ago),
        failed_at: failed_ago.map(|ago| NOW - ago),
    }
}

#[test]
fn unread_arms_on_news_the_operator_has_not_been_back_for_and_on_nothing_else() {
    const IDLE: bool = false;
    const BUSY: bool = true;
    let long_ago = Some(NOW - 5_000);
    assert_eq!(
        unread_arming(&news(Some(AFTER), None), long_ago, IDLE, NOW, AFTER),
        Some(Unread::Success),
        "news newer than the last interaction, with nothing running: the lamp arms"
    );
    assert_eq!(
        unread_arming(&news(Some(AFTER), None), long_ago, BUSY, NOW, AFTER),
        None,
        "the same news with something working is the loop lamp's business"
    );
    assert_eq!(
        unread_arming(
            &news(Some(AFTER), None),
            Some(NOW - AFTER + 1),
            IDLE,
            NOW,
            AFTER
        ),
        None,
        "an interaction AFTER the news is the operator having seen it"
    );
    assert_eq!(
        unread_arming(
            &news(Some(AFTER), None),
            Some(NOW - AFTER),
            IDLE,
            NOW,
            AFTER
        ),
        None,
        "news exactly AT the interaction edge is not newer than it; dark on a tie"
    );
    assert_eq!(
        unread_arming(&news(Some(AFTER), None), None, IDLE, NOW, AFTER),
        None,
        "no interaction at all is no proof the news is unseen, so the lamp stays dark"
    );
    assert_eq!(
        unread_arming(&News::default(), long_ago, IDLE, NOW, AFTER),
        None,
        "and a record with nothing in it arms nothing"
    );
}

#[test]
fn success_news_waits_out_its_delay_and_failure_news_does_not() {
    let long_ago = Some(NOW - 5_000);
    assert_eq!(
        unread_arming(&news(Some(AFTER - 1), None), long_ago, false, NOW, AFTER),
        None,
        "one second under the delay, a result the operator may still be looking at"
    );
    assert_eq!(
        unread_arming(&news(Some(AFTER), None), long_ago, false, NOW, AFTER),
        Some(Unread::Success),
        "exactly at it, it arms: news that old HAS waited that long"
    );
    // FAILURE HAS NO DELAY AT ALL, which is the operator's own ruling: the
    // sooner they know a run died, the better.
    assert_eq!(
        unread_arming(&news(None, Some(0)), long_ago, false, NOW, AFTER),
        Some(Unread::Failure),
        "a failure this second arms this second"
    );
    // RED WINS WHEN BOTH ARE PENDING, whichever is fresher, because showing
    // the calmer of the two would hide the one that needs answering.
    assert_eq!(
        unread_arming(&news(Some(AFTER), Some(0)), long_ago, false, NOW, AFTER),
        Some(Unread::Failure),
        "a failure outranks a success that has waited out its whole delay"
    );
    assert_eq!(
        unread_arming(&news(Some(0), Some(AFTER)), long_ago, false, NOW, AFTER),
        Some(Unread::Failure),
        "and it still outranks it when the success is the fresher of the two"
    );
    // A CLOCK BEHIND THE NEWS HAS NO AGE IN IT, so a machine whose clock
    // stepped back does not read a huge age through a wrapping subtraction.
    assert_eq!(
        unread_arming(
            &News {
                done_at: Some(NOW + 500),
                failed_at: None
            },
            long_ago,
            false,
            NOW,
            AFTER
        ),
        None,
        "a now before the news has no elapsed time in it"
    );
    // AND A FAILURE FROM THE FUTURE ARMS NOTHING EITHER, which is the same
    // rule for the flavour that has no age test of its own. The record only
    // ever moves FORWARD, so a clock that stepped backwards leaves an epoch
    // nothing later will pull back: read as ordinary news it is newer than
    // every interaction there will ever be, and the lamp would hold red
    // until wall time caught up with it.
    assert_eq!(
        unread_arming(
            &News {
                done_at: None,
                failed_at: Some(NOW + 500)
            },
            long_ago,
            false,
            NOW,
            AFTER
        ),
        None,
        "a failure the clock says has not happened yet arms no lamp"
    );
    // AND STILL NOT WITH NO DELAY AT ALL. `after_secs` may be zero, and a
    // saturated age of zero passes a zero threshold, so this edge is where
    // "no elapsed time" and "an elapsed time of zero" stop agreeing.
    assert_eq!(
        unread_arming(
            &News {
                done_at: Some(NOW + 500),
                failed_at: None
            },
            long_ago,
            false,
            NOW,
            0
        ),
        None,
        "a stepped-back clock cannot arm through a zero threshold"
    );
}

#[test]
fn the_interaction_edge_is_the_freshest_of_the_three_roads() {
    // THE FRESHEST WINS, whichever road it is. The stalest would arm the
    // unread lamp about news the operator already saw through the road they
    // were actually using.
    assert_eq!(
        last_interaction(Some(100), Some(9_500), Some(9_000), NOW),
        Some(NOW - 100),
        "the desk's idle age counts back from now, and here it is freshest"
    );
    assert_eq!(
        last_interaction(Some(2_000), Some(9_500), Some(9_600), NOW),
        Some(9_600),
        "and here the phone marker is"
    );
    assert_eq!(
        last_interaction(None, Some(9_500), None, NOW),
        Some(9_500),
        "one readable road is enough"
    );
    assert_eq!(
        last_interaction(None, None, None, NOW),
        None,
        "and no road at all proves nothing, so the lamp stays dark"
    );
    assert_eq!(
        last_interaction(Some(NOW + 5_000), None, None, NOW),
        Some(0),
        "an idle age longer than the clock is an interaction at the epoch, \
         never a wrapped one in the far future"
    );
}
