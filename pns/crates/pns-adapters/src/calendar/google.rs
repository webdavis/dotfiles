//! `type = "google"`: busy intervals read out of Google Calendar, two calls
//! per poll at most.
//!
//! THE CREDENTIALS TRAVEL IN A REQUEST BODY AND A HEADER AND NOWHERE ELSE.
//! Every refusal below is a fixed sentence naming the STEP or the SHAPE that
//! failed: no response body, no client secret, no refresh token and no access
//! token can reach a log line through one.
//!
//! freeBusy CARRIES NO EVENT TEXT AT ALL, which is why it is the endpoint
//! this reads: the answer is a list of intervals, so a meeting's subject and
//! its attendees never reach this machine in the first place.

use crate::config::GoogleCalendar;
use pns_domain::mute::calendar::Event;
use std::path::Path;
use std::time::Duration;

mod freebusy;
mod rfc3339;
mod token;

/// The OAuth token endpoint, which a refresh token is exchanged at.
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
/// The freeBusy endpoint, which answers busy intervals and nothing else.
const FREEBUSY_ENDPOINT: &str = "https://www.googleapis.com/calendar/v3/freeBusy";

/// The most of either answer read into memory. A freeBusy answer over a
/// handful of calendars measures kilobytes, so this is generous and still a
/// bound: a proxy streaming garbage costs at most this much before the poll
/// refuses.
const GOOGLE_BODY_CAP: u64 = 256 * 1024;

/// ONE HOUR AHEAD, the window the busy intervals are asked for. A meeting
/// further out than that cannot start before the next poll, and the mute this
/// feature sets never runs longer than the event it was set for.
const GOOGLE_WINDOW_SECS: u64 = 3_600;

pub(super) struct GoogleCalendarSource {
    /// The agent both calls ride, INJECTED so a test can hand in one wearing
    /// a scripted transport (`Agent::with_parts`): the production pipeline
    /// runs for real and only the wire is fake.
    agent: ureq::Agent,
    token_endpoint: String,
    freebusy_endpoint: String,
    settings: GoogleCalendar,
}

impl GoogleCalendarSource {
    /// The production wiring: TLS verified (these are public hosts with real
    /// certificates), no redirects, and the table's own deadline on every
    /// call.
    pub(super) fn new(settings: GoogleCalendar, deadline: Duration) -> Self {
        Self::with_agent(
            Self::production_config(deadline).new_agent(),
            TOKEN_ENDPOINT.to_string(),
            FREEBUSY_ENDPOINT.to_string(),
            settings,
        )
    }

    /// The agent semantics every call runs under, NAMED so the scripted
    /// transport can run the same ones: a test that built its own config
    /// would pass while production followed a redirect, which is how a
    /// refresh token reaches a host nobody meant to send it to.
    pub(crate) fn production_config(deadline: Duration) -> ureq::config::Config {
        ureq::Agent::config_builder()
            .timeout_global(Some(deadline))
            .max_redirects(0)
            .build()
    }

    /// The same source over any agent and any pair of endpoints: the seam the
    /// scripted transport injects through.
    pub(crate) fn with_agent(
        agent: ureq::Agent,
        token_endpoint: String,
        freebusy_endpoint: String,
        settings: GoogleCalendar,
    ) -> Self {
        Self {
            agent,
            token_endpoint,
            freebusy_endpoint,
            settings,
        }
    }

    /// One poll: the access token (cached, or exchanged), then the busy
    /// intervals over every configured calendar.
    ///
    /// A REFUSED `busy()` CALL EVICTS THE CACHED TOKEN. A revoked or expired
    /// token the cache still calls good would otherwise be re-sent, refused,
    /// on every poll up to an hour, so the next poll exchanges again instead
    /// of repeating the same dead token.
    pub(super) fn read(&self, state: &Path, now: u64) -> Result<Vec<Event>, String> {
        let access_token = self.access_token(state, now)?;
        self.busy(&access_token, now).inspect_err(|_| {
            let _ = std::fs::remove_file(state.join(token::TOKEN_STATE));
        })
    }
}

#[cfg(test)]
mod tests;
