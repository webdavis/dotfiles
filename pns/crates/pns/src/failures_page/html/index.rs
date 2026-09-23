//! `/`: the site index. One row per page this server has; today that is
//! `/failures` alone, with a live count of what is failing so the row is
//! worth reading before it is clicked.

use super::{footer, shell};

const CSS: &str = "<style>:root { color-scheme:dark; background:#101417; }
#pns-index { --paper:#15191d; --ink:#edf0f3; --muted:#a0a9b3; --line:#343d46; font:14px/1.5 system-ui,sans-serif; color:var(--ink); background:var(--paper); border:1px solid var(--line); border-radius:12px; padding:28px; max-width:690px; margin:0 auto; box-sizing:border-box; }
#pns-index * { box-sizing:border-box; }
#pns-index h2 { font-size:22px; line-height:1.3; margin:0 0 20px; font-weight:650; letter-spacing:-.5px; }
#pns-index a { color:var(--ink); font-weight:600; text-decoration:none; }
#pns-index a:hover { text-decoration:underline; }
#pns-index .idx-row { display:flex; align-items:baseline; gap:16px; padding:10px 0; border-top:1px solid var(--line); }
#pns-index .idx-row:first-of-type { border-top:none; }
#pns-index .idx-desc { color:var(--muted); font-size:12px; flex:1; }
#pns-index .idx-count { color:var(--muted); font-variant-numeric:tabular-nums; font-size:12px; }
#pns-index footer { border-top:1px solid var(--line); padding-top:14px; margin-top:20px; display:flex; flex-wrap:wrap; gap:5px 16px; justify-content:space-between; color:var(--muted); font-size:11px; }
</style>";

/// `/`: the index. `count` is the live number of failing legs the listing
/// would show, read by the caller off the same store rather than derived
/// here, so this page never reaches into the ledger of its own accord.
pub(crate) fn index_page(count: usize, now: u64) -> String {
    let mut body = String::from(CSS);
    body.push_str("<section id=\"pns-index\" aria-label=\"pns\"><header><h2>pns</h2></header>");
    body.push_str(&format!(
        "<div class=\"idx-row\"><a href=\"/failures\">Failures</a>\
         <span class=\"idx-desc\">Deliveries that have not gone through yet.</span>\
         <span class=\"idx-count\">{}</span></div>",
        if count == 0 {
            "none".to_string()
        } else {
            count.to_string()
        }
    ));
    body.push_str(&footer(now));
    body.push_str("</section>");
    shell(&body)
}

#[cfg(test)]
#[path = "index/tests.rs"]
mod tests;
