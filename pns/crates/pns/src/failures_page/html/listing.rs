//! `/`: the vertical timeline. Day headings, a time gutter, a dot on a rail,
//! one entry per burst of consecutive same-day, same-status, same-gave-up
//! legs, a `<details>` per entry.
//!
//! EVERY VALUE COMES FROM [`command_failures::rows`], the same function the
//! terminal's own listing builds its columns from. This module groups those
//! rows into bursts and lays them out; it invents no status word, no clock
//! and no destination name of its own.

use super::{escaped, field, footer, long_utc, minute_word, minutes_until, shell};
use crate::command_failures::{self, ListingRow};
use pns_application::{RetryFacts, StoredFailure};
use pns_domain::failure;
use pns_domain::retry::RetryLimits;

const CSS: &str = "<style>:root { color-scheme:dark; background:#101417; }
#failure-history { --paper:#15191d; --ink:#edf0f3; --muted:#a0a9b3; --line:#343d46; --red:#ff8993; --amber:#efbd6a; --tint:#1c2228; font:14px/1.5 system-ui,sans-serif; color:var(--ink); background:var(--paper); border:1px solid var(--line); border-radius:12px; padding:28px; max-width:690px; margin:0 auto; box-sizing:border-box; }
#failure-history * { box-sizing:border-box; }
#failure-history h2 { font-size:22px; line-height:1.3; margin:0; font-weight:650; letter-spacing:-.5px; }
#failure-history .fh-header { display:flex; flex-wrap:wrap; justify-content:space-between; align-items:baseline; gap:8px; margin-bottom:26px; }
#failure-history .fh-muted { color:var(--muted); }
#failure-history .fh-small { font-size:12px; }
#failure-history .fh-day { display:grid; grid-template-columns:76px 1fr; gap:20px; align-items:baseline; margin:0 0 16px; font-size:13px; }
#failure-history .fh-day strong { font-weight:650; }
#failure-history .fh-row { display:grid; grid-template-columns:76px minmax(0,1fr); gap:20px; position:relative; }
#failure-history .fh-clock { font-variant-numeric:tabular-nums; text-align:right; font-size:13px; padding-top:1px; }
#failure-history .fh-content { position:relative; padding:0 0 22px 24px; border-left:2px solid var(--line); min-width:0; }
#failure-history .fh-dot { position:absolute; left:-7px; top:6px; width:12px; height:12px; background:var(--red); border:3px solid var(--paper); border-radius:50%; box-shadow:0 0 0 1px var(--red); }
#failure-history .fh-active .fh-dot { background:var(--paper); box-shadow:0 0 0 2px var(--amber); }
#failure-history .fh-title { display:flex; align-items:baseline; justify-content:space-between; flex-wrap:wrap; gap:4px 12px; margin-bottom:4px; }
#failure-history .fh-title strong { font-size:16px; font-weight:600; }
#failure-history .fh-state { color:var(--red); font-size:12px; font-weight:600; white-space:nowrap; }
#failure-history .fh-active .fh-state { color:var(--amber); }
#failure-history .fh-sub { color:var(--muted); }
#failure-history .fh-next { margin-top:10px; }
#failure-history details { margin-top:10px; }
#failure-history summary { width:fit-content; color:var(--muted); font-size:12px; padding:5px 0; }
#failure-history summary:hover { color:var(--ink); }
#failure-history .fh-detail { background:var(--tint); border-radius:6px; padding:12px 14px; margin-top:8px; }
#failure-history dl { display:grid; grid-template-columns:auto 1fr; gap:7px 16px; margin:0; font-size:12px; }
#failure-history dt { color:var(--muted); }
#failure-history dd { margin:0; overflow-wrap:anywhere; }
#failure-history .fh-row + .fh-day { margin-top:24px; }
#failure-history .fh-last .fh-content { border-color:transparent; padding-bottom:0; }
#failure-history .fh-footer { border-top:1px solid var(--line); padding-top:14px; margin-top:26px; display:flex; flex-wrap:wrap; gap:5px 16px; justify-content:space-between; color:var(--muted); font-size:11px; }
@media(max-width:460px) { #failure-history { padding:20px 14px; } #failure-history .fh-row,#failure-history .fh-day { grid-template-columns:48px minmax(0,1fr); gap:14px; } #failure-history .fh-content { padding-left:18px; } #failure-history dl { grid-template-columns:1fr; gap:3px; } #failure-history dd { margin-bottom:6px; } }
@media(pointer:coarse) { #failure-history summary { min-height:44px; display:flex; align-items:center; } }
</style>";

/// One burst: consecutive legs, in the ledger's newest-first order, sharing
/// a UTC day, a status word and a gave-up state. A burst of one is a single
/// leg rendered with the single-leg field set.
struct Burst<'a> {
    day: String,
    members: Vec<(&'a StoredFailure, &'a ListingRow)>,
}

/// `/`: the header, the timeline (or the empty-ledger line), the footer.
///
/// `retry_facts` IS A CLOSURE rather than a store reference, so a test can
/// hand this fixture data with no sandbox and no database at all.
pub(crate) fn listing_page(
    failures: &[StoredFailure],
    retry_facts: &impl Fn(u64) -> Option<RetryFacts>,
    now: u64,
) -> String {
    let mut body = String::from(CSS);
    body.push_str(
        "<section id=\"failure-history\" aria-label=\"Failures\">\
         <header class=\"fh-header\"><h2>Failures</h2></header>",
    );
    if failures.is_empty() {
        body.push_str("<p>Nothing is failing to deliver.</p>");
    } else {
        let rows = command_failures::rows(failures);
        body.push_str(&entries(&bursts(failures, &rows), retry_facts, now));
    }
    body.push_str(&footer(now));
    body.push_str("</section>");
    shell(&body)
}

fn bursts<'a>(failures: &'a [StoredFailure], rows: &'a [ListingRow]) -> Vec<Burst<'a>> {
    let mut out: Vec<Burst<'a>> = Vec::new();
    for member in failures.iter().zip(rows.iter()) {
        let (failure, row) = member;
        let day = pns_adapters::utc_timestamp(failure.failed_at)
            .map(|iso| iso[..10].to_string())
            .unwrap_or_default();
        let joins_last = out.last().is_some_and(|burst: &Burst| {
            burst.day == day
                && burst.members.last().is_some_and(|(_, last_row)| {
                    last_row.status == row.status && last_row.gave_up == row.gave_up
                })
        });
        if joins_last {
            out.last_mut().expect("just matched").members.push(member);
        } else {
            out.push(Burst {
                day,
                members: vec![member],
            });
        }
    }
    out
}

fn entries(bursts: &[Burst], retry_facts: &impl Fn(u64) -> Option<RetryFacts>, now: u64) -> String {
    let mut out = String::new();
    let mut last_day: Option<&str> = None;
    for (index, burst) in bursts.iter().enumerate() {
        if last_day != Some(burst.day.as_str()) {
            let heading = pns_adapters::utc_day_heading(burst.members[0].0.failed_at, now)
                .unwrap_or_default();
            out.push_str(&format!(
                "<div class=\"fh-day\"><strong>{}</strong></div>",
                escaped(&heading)
            ));
            last_day = Some(burst.day.as_str());
        }
        out.push_str(&entry(burst, retry_facts, now, index + 1 == bursts.len()));
    }
    out
}

fn entry(
    burst: &Burst,
    retry_facts: &impl Fn(u64) -> Option<RetryFacts>,
    now: u64,
    is_last: bool,
) -> String {
    let newest = burst.members[0];
    let oldest = *burst.members.last().expect("a burst has at least one leg");
    let count = burst.members.len();
    let gave_up = newest.1.gave_up;
    let mut classes = vec!["fh-row"];
    if !gave_up {
        classes.push("fh-active");
    }
    if is_last {
        classes.push("fh-last");
    }
    let title = if count == 1 {
        failure::headline(newest.0.outcome, &newest.1.destination)
    } else {
        failure::capitalized(&newest.1.status)
    };
    let state_word = if gave_up { "Not delivered" } else { "Retrying" };
    let newest_hhmm = &newest.1.when[11..16];
    let gutter = if count == 1 {
        format!("<time class=\"fh-clock\">{newest_hhmm}</time>")
    } else {
        format!(
            "<div class=\"fh-clock\"><time>{newest_hhmm}</time><br>\
             <span class=\"fh-muted fh-small\">to {}</span></div>",
            &oldest.1.when[11..16]
        )
    };
    format!(
        "<article class=\"{}\" aria-label=\"{}\">{gutter}\
         <div class=\"fh-content\"><span class=\"fh-dot\" aria-hidden=\"true\"></span>\
         <div class=\"fh-title\"><strong>{}</strong><span class=\"fh-state\">{state_word}</span></div>\
         {}{}\
         <details><summary>Details</summary><div class=\"fh-detail\">{}</div></details>\
         </div></article>",
        classes.join(" "),
        escaped(&format!("{newest_hhmm}, {title}, {state_word}")),
        escaped(&title),
        subtitle(burst),
        next_line(burst, count, gave_up, retry_facts, now),
        details(burst, count, gave_up, retry_facts),
    )
}

/// The destinations, distinct and capitalized, joined with "and".
fn subtitle(burst: &Burst) -> String {
    let mut seen: Vec<String> = Vec::new();
    for (failure, _) in &burst.members {
        let name = failure::capitalized(&failure.destination);
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    format!(
        "<div class=\"fh-sub\">{}</div>",
        escaped(&joined_with_and(&seen))
    )
}

fn joined_with_and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let (last, head) = items.split_last().expect("non-empty above");
            format!("{}, and {last}", head.join(", "))
        }
    }
}

fn next_line(
    burst: &Burst,
    count: usize,
    gave_up: bool,
    retry_facts: &impl Fn(u64) -> Option<RetryFacts>,
    now: u64,
) -> String {
    if count > 1 {
        let newest = burst.members[0].0.failed_at;
        let oldest = burst.members.last().expect("non-empty").0.failed_at;
        let minutes = newest.saturating_sub(oldest) / 60;
        let span = if minutes == 0 {
            "under a minute".to_string()
        } else {
            minute_word(minutes)
        };
        return format!(
            "<div class=\"fh-next\">{count} failed deliveries <span class=\"fh-muted\">over {span}</span></div>"
        );
    }
    if gave_up {
        return String::new();
    }
    let id = burst.members[0].0.id;
    match retry_facts(id).and_then(|facts| minutes_until(facts.due, now)) {
        Some(minutes) => format!(
            "<div class=\"fh-next\">Next try in {}</div>",
            minute_word(minutes)
        ),
        None => "<div class=\"fh-next\">Next try now</div>".to_string(),
    }
}

fn details(
    burst: &Burst,
    count: usize,
    gave_up: bool,
    retry_facts: &impl Fn(u64) -> Option<RetryFacts>,
) -> String {
    if count == 1 {
        single_details(burst.members[0], gave_up, retry_facts)
    } else {
        burst_details(burst, gave_up)
    }
}

fn single_details(
    member: (&StoredFailure, &ListingRow),
    gave_up: bool,
    retry_facts: &impl Fn(u64) -> Option<RetryFacts>,
) -> String {
    let (failure, row) = member;
    let mut dl = String::from("<dl>");
    dl.push_str(&field("Source", &escaped(&row.agent)));
    dl.push_str(&field(
        "Delivery",
        &format!("<a href=\"/failures/{}\">{}</a>", failure.id, failure.id),
    ));
    if gave_up {
        dl.push_str(&field("Outcome", "Permanent failure; no retries"));
        dl.push_str(&field("Recorded", &long_utc(failure.failed_at)));
    } else if let Some(facts) = retry_facts(failure.id) {
        dl.push_str(&field("Next try", &long_utc(facts.due)));
        dl.push_str(&field(
            "Attempts used",
            &format!("{} of {}", row.retries, RetryLimits::default().max_retries),
        ));
        dl.push_str(&field(
            "Retry deadline",
            &long_utc(facts.started + RetryLimits::default().event_max_age_secs),
        ));
    }
    dl.push_str("</dl>");
    dl
}

/// No live retry facts to read for a burst: each leg has its own, and there
/// is no one "next try" to show for eighteen of them at once. A burst that
/// has not been dead-lettered says so in general terms rather than picking
/// one leg's schedule to stand for all of them.
fn burst_details(burst: &Burst, gave_up: bool) -> String {
    let outcome = if gave_up {
        "Permanent failure; no retries"
    } else {
        "Retrying; still within budget"
    };
    let mut sources: Vec<String> = Vec::new();
    for (failure, _) in &burst.members {
        if !sources.contains(&failure.agent) {
            sources.push(failure.agent.clone());
        }
    }
    let oldest = burst.members.last().expect("non-empty");
    let newest = &burst.members[0];
    let window = format!(
        "{}, {} to {} UTC",
        month_day(oldest.0.failed_at),
        &oldest.1.when[11..16],
        &newest.1.when[11..16],
    );
    let mut records = String::new();
    for (index, (failure, row)) in burst.members.iter().enumerate() {
        if index > 0 {
            records.push_str("<br>");
        }
        let route = if row.route.is_empty() {
            String::new()
        } else {
            format!(" · {}", escaped(&row.route))
        };
        records.push_str(&format!(
            "<a href=\"/failures/{}\">{}</a> · {}{route} · {}",
            failure.id,
            failure.id,
            escaped(&row.agent),
            &row.when[11..16],
        ));
    }
    format!(
        "<dl>{}{}{}{}</dl>",
        field("Outcome", outcome),
        field("Sources", &escaped(&sources.join(", "))),
        field("Window", &escaped(&window)),
        field("Records", &records),
    )
}

/// "September 20": [`super::long_utc`]'s date half, with no year and no time.
fn month_day(epoch: u64) -> String {
    pns_adapters::utc_long(epoch)
        .and_then(|long| long.split(',').next().map(str::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "listing/tests.rs"]
mod tests;
