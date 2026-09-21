//! The `open` section: what is still waiting on a person.
//!
//! IT HAS NO WINDOW, which is the whole point of it. A session blocked three
//! days ago is still blocked, and an apply owed last week is still owed, so
//! the two commands that fill it run with nothing to substitute and the
//! sessions are read out of the window's own events.

use crate::{DeadLetteredLegs, SourceCommands};
use pns_domain::recap::{Recap, activity::Event, sections::Open};

/// What is open, from the three places that know.
///
/// A SESSION IS OPEN WHEN ITS LAST WORD WAS A WAITING ONE. The events arrive
/// oldest first, so the last state per session is the one that counts, and a
/// later `resolved` for the same session takes it off the list without a rule
/// of its own.
pub(super) fn gather(
    commands: &impl SourceCommands,
    failures: &impl DeadLetteredLegs,
    recap: &Recap,
    events: &[Event],
) -> Open {
    Open {
        sessions: waiting(events),
        pull_requests: unbounded(commands, recap.sources.pull_requests.as_deref()),
        applies: unbounded(commands, recap.sources.applies.as_deref()),
        dead_lettered: failures.dead_lettered(),
    }
}

/// One line per session whose last state is a waiting one.
fn waiting(events: &[Event]) -> Vec<String> {
    let mut last: Vec<(&str, &Event)> = Vec::new();
    for event in events {
        match last
            .iter_mut()
            .find(|(session, _)| *session == event.session)
        {
            Some(held) => held.1 = event,
            None => last.push((&event.session, event)),
        }
    }
    last.into_iter()
        .filter(|(_, event)| pns_domain::missed::NEEDS_YOU.contains(&event.state.as_str()))
        .map(|(_, event)| pns_domain::recap::night::described(event))
        .collect()
}

/// One command's rows with no window given to it, or nothing at all when it
/// is not configured or would not answer. A FAILED COMMAND ADDS NO LINE HERE,
/// because its own section already says the exit code, and the open list is
/// the one place a diagnostic must not crowd out what is waiting.
fn unbounded(commands: &impl SourceCommands, argv: Option<&[String]>) -> Vec<String> {
    argv.map(|argv| commands.run(argv, None, None).rows())
        .unwrap_or_default()
}

/// The one line `pns recap open` is headed by: the pane the operator was last
/// in, and what was on it.
///
/// OFF THE NEWEST EVENT, which is what the store already knows. The herdr
/// workspace LABEL is not stored and is not asked for here; the id is what
/// the page carries, because resolving it means spawning a CLI on the one
/// form an unlock automation runs.
pub(super) fn where_last(events: &[Event]) -> String {
    let Some(last) = events.last() else {
        return NOWHERE.to_string();
    };
    let mut line = String::from("Where you were");
    for field in [&last.workspace, &last.pane, &last.project, &last.branch] {
        if !field.is_empty() {
            line.push_str(": ");
            line.push_str(field);
        }
    }
    match line.contains(':') {
        true => line,
        false => NOWHERE.to_string(),
    }
}

/// What the header says when nothing in the store places the operator
/// anywhere. Said rather than left blank, for `NOTHING_OPEN`'s reason.
const NOWHERE: &str = "Where you were: nothing recorded";
