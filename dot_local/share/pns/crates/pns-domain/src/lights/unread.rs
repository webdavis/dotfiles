//! News nobody has seen yet, and when the unread lamp arms on it.

/// The two epochs the unread lamp is armed from: when a turn last finished, and
/// when one last died.
///
/// TWO FIELDS AND NOT A QUEUE, because the question is not what happened but
/// whether anything has happened since the operator last touched the machine.
/// A queue would answer the same question with a file that grows.
///
/// `None` IS "NOTHING OF THAT KIND YET", never an epoch of zero. Zero is 1970,
/// which is older than every interaction there has ever been, so a zero read as
/// a real epoch simply never arms and a zero WRITTEN as one would arm forever
/// against an unreadable interaction clock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct News {
    pub done_at: Option<u64>,
    pub failed_at: Option<u64>,
}
/// The record after one event, or None for an event that is not news.
///
/// THE TWO PULSE BEHAVIOURS AND NOTHING ELSE. A wait is the blocked lamp's
/// business and is not news the operator has missed: it is a question still on
/// screen. Reusing `pulse::state_behaviour`'s answer rather than re-reading the
/// state word is what keeps the lamp that flashes and the record that arms the
/// unread lamp from disagreeing about one event.
///
/// IT IS WRITTEN WHATEVER THE DELIVERY DID. A card that was suppressed, muted
/// or dropped is exactly the news this lamp exists to carry, so the record is
/// not a function of whether anything was delivered.
///
/// AND AN EPOCH ONLY EVER MOVES FORWARD. Two events land together often enough
/// (an agent that finished beside one that died), each reads the record and
/// publishes the whole line, so a run that was slow to publish would otherwise
/// put an OLDER second back over a newer one. What that costs is the lamp's
/// colour: a failure recorded and then overwritten is red the operator never
/// sees, and a success pushed backwards arms its lamp before it should.
pub fn news_after(
    held: News,
    behaviour: crate::lamps::config::Behaviour,
    now: u64,
) -> Option<News> {
    let forward = |at: Option<u64>| at.max(Some(now));
    match behaviour {
        crate::lamps::config::Behaviour::Done => Some(News {
            done_at: forward(held.done_at),
            ..held
        }),
        crate::lamps::config::Behaviour::Failed => Some(News {
            failed_at: forward(held.failed_at),
            ..held
        }),
        crate::lamps::config::Behaviour::Blocked
        | crate::lamps::config::Behaviour::Unread
        | crate::lamps::config::Behaviour::Looping => None,
    }
}
/// Which of the unread lamp's two colours is showing.
///
/// TWO FLAVOURS OF ONE BEHAVIOUR, never two routable behaviours: a config
/// carries `unread` or it does not, and both colours ride the lamp that carries
/// it. That is the operator's own routing map read literally, where the two are
/// always listed together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    Failure,
    Success,
}
/// Whether the unread lamp is armed, and in which colour.
///
/// THE QUESTION IS "IS THERE NEWS THE OPERATOR HAS NOT BEEN BACK FOR", and the
/// edge is their LAST INTERACTION of any kind: a key at the desk, input from the
/// phone, or the deliberate phone marker. One rule over every input rather than
/// one rule per input, which is the operator's own wording.
///
/// NOTHING WORKING, which is the other half of the condition. Work in progress
/// is the loop lamp's business and a lamp cannot be both; news that arrives
/// while a run is still going is not news anybody has missed yet.
///
/// NO INTERACTION AT ALL IS NO LAMP, never an edge at epoch zero. A machine
/// that cannot prove the operator was ever here cannot prove this news is
/// unseen either, and dark is the direction every unreadable reading on this
/// path takes.
///
/// RED WINS WHEN BOTH ARE PENDING (operator ruling): a run that died outranks
/// one that finished, and showing the calmer of the two would hide the one that
/// needs answering.
///
/// FAILURE ARMS AT ONCE AND SUCCESS WAITS. A result the operator is still
/// looking at should not light a lamp about itself, so success news has to be
/// `after_secs` old; a failure has no such grace, because the sooner they know
/// the better.
///
/// THE AGE TEST IS CLOSED AND THE EDGE TEST IS NOT, which is two different
/// questions taking the crate's two standing conventions. News exactly
/// `after_secs` old HAS waited that long (`session_was_long`'s rule), and news
/// exactly AT the interaction edge is not newer than it (`marker_is_live`'s
/// sibling rule, and the direction that leaves a lamp dark on a tie).
pub fn unread_arming(
    news: &News,
    last_interaction: Option<u64>,
    working: bool,
    now: u64,
    after_secs: u64,
) -> Option<Unread> {
    if working {
        return None;
    }
    let edge = last_interaction?;
    // NEWS FROM THE FUTURE IS NEWS NOBODY CAN JUDGE, and it arms nothing of
    // either flavour. A clock that stepped backwards leaves an epoch ahead of
    // now, and the record only ever moves FORWARD, so nothing later will pull it
    // back: read as ordinary news it is newer than every interaction there will
    // ever be, and the lamp would hold red until wall time caught up with it.
    // The success flavour has always taken this direction through its age test;
    // this is the same rule said once for both.
    let unseen = |at: Option<u64>| at.filter(|at| *at > edge && *at <= now);
    if unseen(news.failed_at).is_some() {
        return Some(Unread::Failure);
    }
    unseen(news.done_at)
        .filter(|at| now.checked_sub(*at).is_some_and(|age| age >= after_secs))
        .map(|_| Unread::Success)
}
/// When the operator last touched the machine, from the three roads' own
/// readings: the desk clock's idle age, and the two phone epochs.
///
/// THE FRESHEST OF THE THREE, which is the operator's "any input, one clear
/// rule". Taking the stalest would arm the unread lamp about news they had
/// already seen through whichever road they were actually using.
///
/// THE DESK READING IS AN AGE AND THE OTHER TWO ARE EPOCHS, which is why it is
/// subtracted here rather than compared: an idle clock counts back from now,
/// and the saturation is for an idle age longer than the clock itself, which is
/// an interaction at the epoch rather than a wrapped one in the far future.
///
/// NONE WHEN NONE OF THEM CAN BE READ, never an epoch of zero. A machine that
/// cannot prove the operator was ever here cannot prove any news is unseen
/// either, and dark is the direction every unreadable reading on this path
/// takes.
pub fn last_interaction(
    desk_idle_secs: Option<u64>,
    phone_input_at: Option<u64>,
    phone_marker_at: Option<u64>,
    now: u64,
) -> Option<u64> {
    let desk = desk_idle_secs.map(|idle| now.saturating_sub(idle));
    [desk, phone_input_at, phone_marker_at]
        .into_iter()
        .flatten()
        .max()
}
