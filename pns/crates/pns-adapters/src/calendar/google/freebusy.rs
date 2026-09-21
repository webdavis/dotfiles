//! The freeBusy call: the window asked for, and the intervals that come back.

use super::rfc3339::{format_rfc3339, parse_rfc3339};
use super::{GOOGLE_BODY_CAP, GOOGLE_WINDOW_SECS, GoogleCalendarSource};
use pns_domain::mute::calendar::Event;

/// ONE SENTENCE FOR EVERY WAY THE CALL PRODUCES NOTHING, the token
/// exchange's own rule: a non-2xx, a body that would not read, a body that is
/// not the document. The body is never quoted.
pub(super) const FREEBUSY_REFUSED: &str = "the freeBusy call was refused, or answered something other than \
     {\"calendars\": {<id>: {\"busy\": [{\"start\": <time>, \"end\": <time>}]}}}";

/// A CALENDAR THAT REPORTED AN ERROR IS THE WHOLE ANSWER REFUSED, never the
/// calendar dropped: a dropped calendar is a meeting that silently does not
/// mute, and it looks exactly like a clear day.
pub(super) const FREEBUSY_ERRORS: &str =
    "the freeBusy answer carries an `errors` entry for a configured calendar";

/// A CONFIGURED CALENDAR THE ANSWER SAYS NOTHING ABOUT, refused for the same
/// reason.
pub(super) const FREEBUSY_MISSING: &str =
    "the freeBusy answer names no entry for a configured calendar";

/// AN INTERVAL WHOSE BOUNDS DO NOT READ, refused for the same reason.
pub(super) const FREEBUSY_TIME: &str =
    "the freeBusy answer carries a `start` or `end` that is not an RFC 3339 time";

impl GoogleCalendarSource {
    /// The busy intervals over every configured calendar, in epoch seconds.
    pub(super) fn busy(&self, access_token: &str, now: u64) -> Result<Vec<Event>, String> {
        let answer = self
            .agent
            .post(&self.freebusy_endpoint)
            .header("authorization", &format!("Bearer {access_token}"))
            .header("content-type", "application/json")
            .send(request_body(&self.settings.calendars, now).as_str())
            .ok()
            .and_then(|mut answer| {
                answer
                    .body_mut()
                    .with_config()
                    .limit(GOOGLE_BODY_CAP)
                    .read_to_string()
                    .ok()
            })
            .ok_or(FREEBUSY_REFUSED)?;
        parse_freebusy(&answer, &self.settings.calendars)
    }
}

/// The request: the next hour, over every configured calendar.
pub(super) fn request_body(calendars: &[String], now: u64) -> String {
    let items: Vec<serde_json::Value> = calendars
        .iter()
        .map(|id| serde_json::json!({ "id": id }))
        .collect();
    serde_json::json!({
        "timeMin": format_rfc3339(now),
        "timeMax": format_rfc3339(now.saturating_add(GOOGLE_WINDOW_SECS)),
        "items": items,
    })
    .to_string()
}

/// The answer as busy events, the union over every configured calendar.
///
/// FAIL-CLOSED, EXACTLY AS THE COMMAND SOURCE'S PARSER IS. Anything the
/// answer does not state cleanly refuses the whole answer rather than
/// dropping an interval.
pub(super) fn parse_freebusy(answer: &str, calendars: &[String]) -> Result<Vec<Event>, String> {
    let document: serde_json::Value = serde_json::from_str(answer).map_err(|_| FREEBUSY_REFUSED)?;
    let reported = document
        .get("calendars")
        .and_then(serde_json::Value::as_object)
        .ok_or(FREEBUSY_REFUSED)?;
    let mut events = Vec::new();
    for id in calendars {
        let calendar = reported.get(id).ok_or(FREEBUSY_MISSING)?;
        if calendar.get("errors").is_some() {
            return Err(FREEBUSY_ERRORS.to_string());
        }
        let busy = calendar
            .get("busy")
            .and_then(serde_json::Value::as_array)
            .ok_or(FREEBUSY_REFUSED)?;
        for interval in busy {
            let bound = |name: &str| {
                interval
                    .get(name)
                    .and_then(serde_json::Value::as_str)
                    .ok_or(FREEBUSY_REFUSED)
                    .and_then(|stated| parse_rfc3339(stated).ok_or(FREEBUSY_TIME))
            };
            events.push(Event {
                start: bound("start")?,
                end: bound("end")?,
                busy: true,
            });
        }
    }
    Ok(events)
}
