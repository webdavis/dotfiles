//! The operator's quiet hours: when they run, and what a bad one says.

/// The hours the lights stay dark, in minutes since local midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuietWindow {
    pub start: u16,
    pub end: u16,
}
impl QuietWindow {
    /// The minute of the local day this window ends at, which is the one thing
    /// a BARE `pns lights quiet` needs from it.
    pub fn ends_at(&self) -> u16 {
        self.end
    }
}
/// Whether the lights are inside the window at a given minute of the local
/// day.
pub fn quiet_now(window: Option<&QuietWindow>, minutes_now: Option<u16>) -> bool {
    // NO WINDOW IS NEVER QUIET, whatever the clock says: an operator who
    // configured no quiet hours keeps the pulse an unreadable clock would
    // otherwise cost them.
    let Some(window) = window else {
        return false;
    };
    // A CONFIGURED window and no clock FAILS CLOSED, the direction the pulse
    // already takes on an unreadable reading: a missed pulse costs nothing and
    // a flash at 3am is what the window was set to prevent.
    let Some(now) = minutes_now else {
        return true;
    };
    if window.start > window.end {
        // A window that wraps midnight is the two ends of the day joined, so
        // the halves are an OR: past its start tonight, or before its end
        // tomorrow.
        return now >= window.start || now < window.end;
    }
    now >= window.start && now < window.end
}
/// `HH:MM` as minutes since midnight. Two digits each, and in range: an hour
/// of 24 or a minute of 60 names no time of day.
pub(super) fn minute_of_day(clock: &str) -> Option<u16> {
    let (hours, minutes) = clock.split_once(':')?;
    let (hours, minutes) = (two_digits(hours)?, two_digits(minutes)?);
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}
pub(super) fn window_refusal(lamp: &str, stated: &str) -> String {
    format!(
        "lights: `{lamp}` has dim_window {stated:?}, which is not a HH:MM-HH:MM \
         window; that lamp stays dark"
    )
}

/// `HH:MM-HH:MM` and nothing else.
pub fn parse_window(text: &str) -> Option<QuietWindow> {
    let (start, end) = text.split_once('-')?;
    Some(QuietWindow {
        start: minute_of_day(start)?,
        end: minute_of_day(end)?,
    })
}
/// Exactly two ASCII digits, so a sign, a space or a lone digit is not a
/// clock reading that happens to parse.
pub(super) fn two_digits(text: &str) -> Option<u16> {
    if text.len() != 2 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}
