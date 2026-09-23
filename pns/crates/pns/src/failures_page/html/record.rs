//! `/<id>`: one record. Headline and state chip, the meaning line, a timing
//! box for the last and next attempt, the suggested fix, a folded technical
//! detail list.
//!
//! EVERY VALUE COMES FROM [`pns_domain::failure::fields`] and
//! [`pns_domain::failure::headline`], the same functions the terminal's own
//! full form renders from. This module lays them out; it computes none of
//! them itself beyond the relative "in N minutes" the render time needs.

use super::{
    datetime_attr, escaped, field, footer, hhmm, hhmm_of, long_utc, minute_word, minutes_until,
    shell,
};
use crate::command_failures::ListingRow;
use pns_application::RetryFacts;
use pns_domain::failure::{self, Failure};
use pns_domain::retry::RetryLimits;

const CSS: &str = "<style>:root { color-scheme:dark; background:#101417; }
#failure-record { --fr-paper:#15191d; --fr-ink:#edf0f3; --fr-muted:#a0a9b3; --fr-line:#343d46; --fr-amber:#efbd6a; --fr-red:#ff8993; --fr-tint:#1c2228; background:var(--fr-paper); color:var(--fr-ink); font:14px/1.5 system-ui,sans-serif; max-width:690px; margin:0 auto; padding:28px; border:1px solid var(--fr-line); border-radius:12px; box-sizing:border-box; }
#failure-record * { box-sizing:border-box; }
#failure-record .fr-back { color:var(--fr-muted); font-size:12px; display:block; margin-bottom:14px; }
#failure-record .fr-heading { display:flex; justify-content:space-between; align-items:baseline; flex-wrap:wrap; gap:8px 20px; }
#failure-record h2 { font-size:22px; line-height:1.3; font-weight:650; letter-spacing:-.5px; margin:0; }
#failure-record .fr-status { font-size:12px; font-weight:600; color:var(--fr-amber); }
#failure-record .fr-status::before { content:''; display:inline-block; width:8px; height:8px; border:2px solid currentColor; border-radius:50%; margin-right:8px; }
#failure-record .fr-status.fr-dead { color:var(--fr-red); }
#failure-record .fr-meaning { margin:8px 0 24px; color:var(--fr-muted); }
#failure-record .fr-timing { display:grid; grid-template-columns:1fr 1fr; gap:24px; background:var(--fr-tint); border-radius:6px; padding:18px 20px; margin:0; }
#failure-record .fr-timing dt { color:var(--fr-muted); font-size:12px; margin:0 0 5px; }
#failure-record .fr-timing dd { margin:0; }
#failure-record .fr-value { font-size:18px; font-weight:600; font-variant-numeric:tabular-nums; }
#failure-record .fr-meta { color:var(--fr-muted); font-size:12px; display:block; margin-top:3px; }
#failure-record .fr-fix { margin:22px 0; display:grid; grid-template-columns:92px minmax(0,1fr); gap:16px; }
#failure-record .fr-fix dt { font-size:12px; color:var(--fr-muted); padding-top:2px; }
#failure-record .fr-fix dd { margin:0; }
#failure-record details { border-top:1px solid var(--fr-line); }
#failure-record summary { padding:16px 0 8px; color:var(--fr-muted); font-size:12px; }
#failure-record summary:hover { color:var(--fr-ink); }
#failure-record .fr-fields { display:grid; grid-template-columns:110px minmax(0,1fr); gap:12px 18px; margin:16px 0; font-size:12px; }
#failure-record .fr-fields dt { color:var(--fr-muted); }
#failure-record .fr-fields dd { margin:0; overflow-wrap:anywhere; }
#failure-record code { font:12px/1.6 ui-monospace,SFMono-Regular,Menlo,monospace; color:var(--fr-ink); background:transparent; padding:0; white-space:normal; overflow-wrap:anywhere; }
#failure-record footer { display:flex; flex-wrap:wrap; justify-content:space-between; gap:5px 16px; color:var(--fr-muted); font-size:11px; margin-top:18px; }
@media(max-width:460px) { #failure-record { padding:20px 14px; } #failure-record .fr-timing { padding:14px; gap:16px; } #failure-record .fr-fix { grid-template-columns:1fr; gap:5px; } #failure-record .fr-fields { grid-template-columns:1fr; gap:3px; } #failure-record .fr-fields dd { margin-bottom:8px; } }
@media(max-width:350px) { #failure-record .fr-timing { grid-template-columns:1fr; } }
@media(pointer:coarse) { #failure-record summary { min-height:44px; } }
</style>";

/// `/<id>`: the back link, the headline and state chip, the meaning line,
/// the timing box, the suggested fix, the folded technical details.
///
/// `retry` IS THE LEDGER'S OWN FACTS, resolved by the caller. Passing them in
/// rather than a store reference is what lets a test build this page from
/// fixture data with no sandbox and no database at all. `Err(())` is a read
/// failure, kept apart from `Ok(None)` ("no facts recorded") so this can say
/// "unknown" rather than claim a retry is due right now when it simply could
/// not read the ledger.
pub(crate) fn record_page(
    failure: &Failure,
    row: &ListingRow,
    retry: Result<Option<RetryFacts>, ()>,
    now: u64,
) -> String {
    let gave_up = row.gave_up;
    let headline = failure::headline(failure.outcome, &failure.destination);
    let fields = failure::fields(failure);
    let value = |label: &str| -> String {
        fields
            .iter()
            .find(|(field_label, _)| *field_label == label)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    let mut body = String::from(CSS);
    body.push_str(&format!(
        "<article id=\"failure-record\" aria-label=\"{}\">\
         <a class=\"fr-back\" href=\"/failures\">Failures</a>\
         <header><div class=\"fr-heading\"><h2>{}</h2>\
         <span class=\"fr-status{}\">{}</span></div>\
         <p class=\"fr-meaning\">{}</p></header>",
        escaped(&headline),
        escaped(&headline),
        if gave_up { " fr-dead" } else { "" },
        if gave_up { "Not delivered" } else { "Retrying" },
        escaped(&value("meaning")),
    ));
    body.push_str(&timing(row, gave_up, retry, now));
    body.push_str(&format!(
        "<dl class=\"fr-fix\">{}</dl>",
        field("Suggested fix", &escaped(&value("fix")))
    ));
    body.push_str(&format!(
        "<details><summary>Technical details</summary>{}</details>",
        technical(failure, gave_up, retry, &value("failed command"))
    ));
    body.push_str(&footer(now, ""));
    body.push_str("</article>");
    shell(&body)
}

/// The `fr-timing` box: what the last attempt was, and what the next one is
/// (or that there will not be one).
fn timing(
    row: &ListingRow,
    gave_up: bool,
    retry: Result<Option<RetryFacts>, ()>,
    now: u64,
) -> String {
    let generation = row.retries + 1;
    let last_meta = format!(
        "Attempt {generation} · {}",
        failure::capitalized(&row.status)
    );
    let (value, meta) = if gave_up {
        (
            "<span class=\"fr-value\">None</span>".to_string(),
            "pns gave up".to_string(),
        )
    } else {
        match retry {
            Ok(Some(facts)) => match minutes_until(facts.due, now) {
                Some(minutes) => (
                    format!(
                        "<span class=\"fr-value\">In {}</span>",
                        minute_word(minutes)
                    ),
                    format!("{} · Attempt {}", hhmm(facts.due), generation + 1),
                ),
                None => (
                    "<span class=\"fr-value\">Now</span>".to_string(),
                    format!("Attempt {}", generation + 1),
                ),
            },
            // Neither "no facts recorded" nor a read failure is "due now":
            // both mean this page cannot say when the next attempt is.
            Ok(None) | Err(()) => (
                "<span class=\"fr-value\">Unknown</span>".to_string(),
                "pns could not read the retry schedule".to_string(),
            ),
        }
    };
    format!(
        "<dl class=\"fr-timing\" aria-label=\"Delivery attempts\">\
         <div><dt>Last attempt</dt><dd><time class=\"fr-value\"{}>{}</time>\
         <span class=\"fr-meta\">{}</span></dd></div>\
         <div><dt>Next attempt</dt><dd>{value}<span class=\"fr-meta\">{}</span></dd></div>\
         </dl>",
        datetime_attr(&row.when),
        hhmm_of(&row.when),
        escaped(&last_meta),
        escaped(&meta),
    )
}

/// The folded `fr-fields` list: identifiers and the routing facts, not the
/// story the headline and meaning already told.
fn technical(
    failure: &Failure,
    gave_up: bool,
    retry: Result<Option<RetryFacts>, ()>,
    command: &str,
) -> String {
    let route = if failure.route.is_empty() {
        "<span class=\"fr-meta\">none</span>".to_string()
    } else {
        escaped(&failure.route)
    };
    // A dead-lettered leg has no deadline any more (blank, matching the
    // timing box's own "pns gave up"); a live leg with no readable facts
    // reads "unknown" rather than silently blank, which would be
    // indistinguishable from "retries have stopped".
    let deadline = if gave_up {
        String::new()
    } else {
        match retry {
            Ok(Some(facts)) => long_utc(facts.started + RetryLimits::default().event_max_age_secs),
            Ok(None) | Err(()) => "unknown".to_string(),
        }
    };
    format!(
        "<dl class=\"fr-fields\">{}{}{}{}{}{}</dl>",
        field("Delivery ID", &failure.id.to_string()),
        field("Sent by", &escaped(&failure.agent)),
        field("Webhook route", &route),
        field(
            "Failed command",
            &format!("<code>{}</code>", escaped(command))
        ),
        field(
            "Retry budget",
            &format!(
                "{} of {} retries used",
                failure.retries,
                RetryLimits::default().max_retries
            )
        ),
        field("Retry deadline", &deadline),
    )
}

#[cfg(test)]
#[path = "record/tests.rs"]
mod tests;
