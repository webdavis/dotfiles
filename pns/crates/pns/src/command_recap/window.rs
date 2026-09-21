//! One typed invocation resolved to two epoch seconds.
//!
//! THE ZONE IS READ HERE AND NOWHERE ELSE in the recap path: the domain's
//! window arithmetic answers in local civil moments and `local_epoch` is what
//! turns each into a second. A window whose bounds the system cannot state is
//! a refusal rather than a window quietly an hour out.

use super::{OPEN, Options, Span, window_words};
use pns_application::recap_bounds;
use pns_domain::recap::window::{
    LocalCivilTime, Window, most_recently_ended, parse_window, resolve as resolve_window,
};

/// A window the engine can be handed: what it is called, when it ran, and
/// whether it was stepped back.
pub(crate) struct Resolved {
    pub name: Option<String>,
    pub previous: bool,
    pub since: u64,
    pub until: u64,
}

/// The window one invocation names, or the sentence saying why it names none.
pub(crate) fn resolve(
    options: &Options,
    recap: &pns_adapters::Recap,
    now: u64,
) -> Result<Resolved, String> {
    match &options.span {
        // `open` HAS NO WINDOW, and its bounds are the retention the store
        // keeps: a session blocked three days ago is still blocked, so the
        // list is everything the table still holds rather than one period.
        Span::Open => Ok(Resolved {
            name: Some(OPEN.to_string()),
            previous: false,
            since: now.saturating_sub(recap.retain.as_secs()),
            until: now,
        }),
        Span::Bounds(bounds) => bounded(bounds, now),
        Span::Duration(written) => bounded(&["--since".to_string(), written.clone()], now),
        Span::MostRecentlyEnded => {
            let minutes = pns_adapters::local_minutes_since_midnight(now)
                .ok_or_else(|| UNREADABLE_CLOCK.to_string())?;
            named(
                most_recently_ended(&recap.periods, u32::from(minutes)),
                false,
                recap,
                now,
            )
        }
        Span::Named { window, previous } => {
            let (window, implied) = parse_window(window).ok_or_else(|| {
                format!(
                    "`{window}` is no recap window; the windows are {}",
                    window_words()
                )
            })?;
            named(window, implied || *previous, recap, now)
        }
    }
}

/// One named window's bounds, as the zone states them.
fn named(
    window: Window,
    previous: bool,
    recap: &pns_adapters::Recap,
    now: u64,
) -> Result<Resolved, String> {
    let (civil, weekday) = pns_adapters::local_civil(now).ok_or(UNREADABLE_CLOCK)?;
    let (since, until) = resolve_window(
        window,
        previous,
        &recap.periods,
        recap.week_starts_on,
        civil,
        weekday,
    );
    Ok(Resolved {
        name: Some(window.as_str().to_string()),
        previous,
        since: epoch(since)?,
        until: epoch(until)?,
    })
}

/// One local civil moment as a second, or the refusal saying the system could
/// not place it.
fn epoch(civil: LocalCivilTime) -> Result<u64, String> {
    pns_adapters::local_epoch(
        civil.year,
        civil.month,
        civil.day,
        civil.hour,
        civil.minute,
        civil.second,
    )
    .ok_or_else(|| UNREADABLE_CLOCK.to_string())
}

/// The `--since`/`--until` family, through the one parser that owns those
/// spellings.
fn bounded(bounds: &[String], now: u64) -> Result<Resolved, String> {
    let (since, until) = recap_bounds(bounds, now, |civil| {
        pns_adapters::local_epoch(
            civil.year,
            civil.month,
            civil.day,
            civil.hour,
            civil.minute,
            civil.second,
        )
    })
    .ok_or_else(|| "the window that was asked for is not one this can state".to_string())?;
    Ok(Resolved {
        name: None,
        previous: false,
        since,
        until,
    })
}

/// What a window whose bounds the system cannot place is told. A recap an
/// hour out is worse than a refusal the operator can read.
const UNREADABLE_CLOCK: &str = "the local clock could not be read, so no window can be stated";
