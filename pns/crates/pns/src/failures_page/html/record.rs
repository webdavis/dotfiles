//! `/<id>`: one record. Headline and state chip, the meaning line, a timing
//! box for the last and next attempt, the suggested fix, a folded technical
//! detail list.
//!
//! EVERY VALUE COMES FROM [`pns_domain::failure::fields`] and
//! [`pns_domain::failure::headline`], the same functions the terminal's own
//! full form renders from. This module lays them out; it computes none of
//! them itself beyond the relative "in N minutes" the render time needs.

use super::{escaped, field, footer, hhmm, long_utc, minute_word, minutes_until, shell};
use crate::command_failures::ListingRow;
use pns_application::RetryFacts;
use pns_domain::failure::{self, Failure};
use pns_domain::retry::RetryLimits;

const CSS: &str = "<style>#failure-record { --fr-paper:light-dark(#fff,#15191d); --fr-ink:light-dark(#20252b,#edf0f3); --fr-muted:light-dark(#626a73,#a0a9b3); --fr-line:light-dark(#dfe3e7,#343d46); --fr-amber:light-dark(#865200,#efbd6a); --fr-red:light-dark(#b12737,#ff8993); --fr-tint:light-dark(#f5f6f8,#1c2228); background:var(--fr-paper); color:var(--fr-ink); font:14px/1.5 system-ui,sans-serif; max-width:690px; margin:0 auto; padding:28px; border:1px solid var(--fr-line); border-radius:12px; box-sizing:border-box; }
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
/// fixture data with no sandbox and no database at all.
pub(crate) fn record_page(
    failure: &Failure,
    row: &ListingRow,
    retry: Option<RetryFacts>,
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
         <a class=\"fr-back\" href=\"/\">Failures</a>\
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
        technical(failure, retry, &value("failed command"))
    ));
    body.push_str(&footer(now));
    body.push_str("</article>");
    shell(&body)
}

/// The `fr-timing` box: what the last attempt was, and what the next one is
/// (or that there will not be one).
fn timing(row: &ListingRow, gave_up: bool, retry: Option<RetryFacts>, now: u64) -> String {
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
        match retry.and_then(|facts| minutes_until(facts.due, now).map(|m| (m, facts.due))) {
            Some((minutes, due)) => (
                format!(
                    "<span class=\"fr-value\">In {}</span>",
                    minute_word(minutes)
                ),
                format!("{} · Attempt {}", hhmm(due), generation + 1),
            ),
            None => (
                "<span class=\"fr-value\">Now</span>".to_string(),
                format!("Attempt {}", generation + 1),
            ),
        }
    };
    format!(
        "<dl class=\"fr-timing\" aria-label=\"Delivery attempts\">\
         <div><dt>Last attempt</dt><dd><time class=\"fr-value\">{}</time>\
         <span class=\"fr-meta\">{}</span></dd></div>\
         <div><dt>Next attempt</dt><dd>{value}<span class=\"fr-meta\">{}</span></dd></div>\
         </dl>",
        &row.when[11..16],
        escaped(&last_meta),
        escaped(&meta),
    )
}

/// The folded `fr-fields` list: identifiers and the routing facts, not the
/// story the headline and meaning already told.
fn technical(failure: &Failure, retry: Option<RetryFacts>, command: &str) -> String {
    let route = if failure.route.is_empty() {
        "<span class=\"fr-meta\">none</span>".to_string()
    } else {
        escaped(&failure.route)
    };
    let deadline = retry
        .map(|facts| long_utc(facts.started + RetryLimits::default().event_max_age_secs))
        .unwrap_or_default();
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
