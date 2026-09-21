use crate::*;
use pns_adapters::{CalendarSource, QuietCalendar, SqliteStore};
use pns_domain::mute::calendar::{CalendarState, Event, Move};
use std::time::Duration;

/// `pns mute calendar`: one read of the calendar the config names, and
/// whatever that means for the mute.
///
/// THE CLOCK RUNS IT. It is registered as a leased job like the room sensor
/// and the GitHub poll, so it runs as a child of the daemon rather than
/// inside its loop: a calendar command that stalls holds up nothing but its
/// own deadline.
///
/// IT IS READ-ONLY ABOUT THE CALENDAR and says nothing about what it read.
/// The only lines it writes name what happened to the mute, never an event's
/// subject, its attendees or its identifier, because these reach a log the
/// operator is not the only reader of.
pub(crate) fn mute_calendar_mode() -> i32 {
    let home = std::env::var("HOME").unwrap_or_default();
    let calendar = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config.quiet_calendar,
        Ok(LoadOutcome::Missing) => QuietCalendar::default(),
        Err(error) => {
            eprintln!(
                "pns mute: the config could not be read ({}); the calendar was not read",
                error.detail()
            );
            return 1;
        }
    };
    let state = state_dir();
    let records = SqliteStore::for_records(state.clone());
    let held = pns_adapters::read_calendar_state(&state);
    let now = now_secs();
    let outcome = poll(
        &calendar,
        now,
        crate::command_mute::read_mute_expiry(&records),
        held,
        &mut |source, deadline| {
            pns_adapters::read_calendar(source, &state, now.unwrap_or_default(), deadline)
        },
    );
    let (chosen, next) = match outcome {
        Poll::Off => return 0,
        Poll::Unread(complaint) => {
            eprintln!("pns mute: {complaint}; the mute was left as it was");
            return 1;
        }
        Poll::Decided(chosen, next) => (chosen, next),
    };
    let written = match chosen {
        Move::Arm(until) => records.set_mute_expiry(Some(until)),
        Move::Clear => records.set_mute_expiry(None),
        Move::Leave => Ok(()),
    };
    if let Err(error) = written {
        // AND THE STATE IS NOT RECORDED, so the next poll tries the same move
        // again rather than believing it landed.
        eprintln!("pns mute: state error (quiet-until could not be written: {error})");
        return 1;
    }
    if next != held
        && let Err(error) = pns_adapters::write_calendar_state(&state, &next)
    {
        eprintln!("pns mute: state error (the calendar state could not be written: {error})");
        return 1;
    }
    match chosen {
        Move::Arm(_) => println!("pns: muted for a calendar event"),
        Move::Clear => println!("pns: unmuted, the calendar event has ended"),
        Move::Leave => {}
    }
    0
}

/// What one poll came to, before anything is written.
enum Poll {
    /// The feature is off, unconfigured, or this machine has no clock. THE
    /// COMMAND IS NOT RUN, which is what makes the shipped default cost
    /// nothing at all.
    Off,
    Unread(String),
    Decided(Move, CalendarState),
}

/// The poll's whole decision, with the calendar read injected so a test can
/// state what the command answered without running one.
fn poll(
    calendar: &QuietCalendar,
    now: Option<u64>,
    expiry: Option<u64>,
    held: CalendarState,
    read: &mut impl FnMut(&CalendarSource, Duration) -> Result<Vec<Event>, String>,
) -> Poll {
    // THE SWITCH IS READ BEFORE THE CLOCK AND BEFORE THE COMMAND: an off
    // feature spawns nothing and reads nothing.
    let (Some(_), Some(now)) = (calendar.armed(), now) else {
        return Poll::Off;
    };
    match read(&calendar.source, calendar.deadline()) {
        Err(complaint) => Poll::Unread(complaint),
        Ok(events) => {
            let (chosen, next) = pns_domain::mute::calendar::decide(&events, now, expiry, held);
            Poll::Decided(chosen, next)
        }
    }
}

#[cfg(test)]
mod tests;
