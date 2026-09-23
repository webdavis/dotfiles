//! The failure page's HTML: the shell every response shares, and the escape
//! every producer string passes through before it lands in one.
//!
//! THE TWO CARD LAYOUTS ARE ONE LEVEL DOWN, in [`listing`] and [`record`].
//! Both read their field VALUES off the same functions the terminal prints
//! from (`command_failures::rows`, `pns_domain::failure::fields`,
//! `pns_domain::failure::headline`); this module only owns the wrapper every
//! page shares and the escape every producer string passes through.

mod listing;
mod record;

pub(crate) use listing::listing_page;
pub(crate) use record::record_page;

/// Every response is wrapped in this: the doctype, title, viewport and
/// color-scheme meta, and the painted ground the card floats on. `color-scheme`
/// is what lets the supplied CSS's `light-dark()` calls resolve, and the body
/// background is what keeps black-on-black impossible for a browser that
/// ignores it.
fn shell(body: &str) -> String {
    format!(
        "<!doctype html>\n<title>pns failures</title>\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"color-scheme\" content=\"light dark\">\n\
         <style>:root{{color-scheme:light dark}}\
         body{{background:light-dark(#f2f3f5,#0f1215);margin:0;padding:16px}}</style>\n\
         {body}\n"
    )
}

/// A bare sentence, inside the shell: what an unreadable ledger, an unknown
/// id and a 404 all fall back to. No card, because there is nothing to lay
/// out for one line.
pub(crate) fn sentence_page(text: &str) -> String {
    shell(&format!("<pre>{}</pre>", escaped(text)))
}

/// The footer every card carries: "All times UTC" on the left, the render
/// time on the right. Never "Sample data": that was the operator's mockup
/// placeholder, and this is the real render time.
fn footer(now: u64) -> String {
    format!(
        "<footer><span>All times UTC</span><span>Updated {}</span></footer>",
        pns_adapters::utc_long(now).unwrap_or_default()
    )
}

/// Every producer string escaped before it lands in the page: a route name,
/// a sender, a destination, a command, a meaning line or a fix line is
/// producer text, and this is the one surface where it could otherwise
/// become markup.
pub(crate) fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// One `<dt>`/`<dd>` pair, shared by both cards' detail lists.
fn field(label: &str, value: &str) -> String {
    format!("<dt>{label}</dt><dd>{value}</dd>")
}

/// "September 30, 2026 at 03:20 UTC", the one long form every full
/// date-and-time field on the page uses.
fn long_utc(epoch: u64) -> String {
    format!("{} UTC", pns_adapters::utc_long(epoch).unwrap_or_default())
}

/// "03:32", sliced from the same ISO instant [`long_utc`] builds from.
fn hhmm(epoch: u64) -> String {
    pns_adapters::utc_timestamp(epoch)
        .map(|iso| iso[11..16].to_string())
        .unwrap_or_default()
}

/// How many whole minutes stand between `now` and a future `due`, rounded up
/// so a deadline forty seconds out still reads as one minute rather than
/// zero. `None` once `due` has passed, which every caller reads as "now".
fn minutes_until(due: u64, now: u64) -> Option<u64> {
    if due <= now {
        return None;
    }
    Some((due - now).div_ceil(60).max(1))
}

/// "1 minute" or "7 minutes".
fn minute_word(minutes: u64) -> String {
    if minutes == 1 {
        "1 minute".to_string()
    } else {
        format!("{minutes} minutes")
    }
}

#[cfg(test)]
#[path = "html/tests.rs"]
mod tests;
