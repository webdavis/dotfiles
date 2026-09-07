//! The moshi backend of the `mobile` channel, native: the phone push, a single
//! HTTPS POST. `mobile` is the plugin the config selects and `type = "moshi"`
//! is what picks this; the module keeps the backend's name because that is what
//! it implements.
//!
//! THE SECRET'S PATH IS THE POINT. The token is read from the config's
//! `[plugins.mobile]` table, placed in the request BODY, and never touches
//! argv, the environment of a child, or an error string: the bash put it on
//! stdin for the same reason (the process table is world-readable), and
//! in-process is the stronger form of the same rule. A missing or empty token
//! is the not-set-up case, and the deliver seam FAILS it by naming the config
//! key to write: nothing is posted, and no event hears the sentence, because
//! this channel is never handed a reporting leg. What has not changed is where
//! the token may appear, which is the request body and nowhere else.

use super::{Delivery, Event};
use crate::routing::ReportMode;

/// Where the push goes when `PNS_MOSHI_URL` says nothing.
pub const DEFAULT_MOSHI_URL: &str = "https://api.getmoshi.app/api/webhook";

/// The POST seam: one call, a URL and a JSON body in, success or not out.
/// The production impl carries the 10 second deadline; a fake records.
pub trait HttpPost {
    fn post_json(&self, url: &str, body: &str) -> bool;
}

pub use pns_adapters::{MOSHI_TYPE, mobile_backend, moshi_secret};

/// The deep link a card's tap follows, built from the ORIGIN PANE and nothing
/// else, or None when there is no pane worth linking to.
///
/// PANE-PRECISE AND PLUMBING-FREE. moshi's scheme is
/// `moshi://herdr?workspace=&tab=&pane=&session=` with every parameter
/// optional (tab and pane since moshi 3.13.0), and the sanitized origin pane
/// is already here at the dispatch site. So the link is built from that and
/// nothing is plumbed: no second herdr call, no workspace on the session
/// view, no id threaded through the gate inputs.
///
/// WELL-FORMED BY CONSTRUCTION, because a malformed action does not degrade
/// the card, it DELETES it: moshi answers a bad body non-2xx, and this
/// channel reads any non-2xx as a delivery that failed. So the guard is
/// `pane_is_safe`, asked HERE rather than assumed of the caller, and its
/// charset (ascii alphanumeric plus `.`, `_`, `:` and `-`) is legal unencoded
/// in a query value, which is what leaves nothing to escape.
///
/// WHAT THE TAP ACTUALLY DOES, stated rather than fought: moshi looks for an
/// active card matching server session AND workspace, else resumes the most
/// recently minimized card for that session, and with no card matching at all
/// it SHOWS AN ERROR rather than opening a connection, because these links
/// only ever resume a card moshi already holds. This link names neither, so it
/// rides whichever card the phone already has and asks the host daemon to
/// refine it to the pane; a pane-only link is best-effort exact focus with no
/// parent to degrade to. It is a DECORATION, so no pane means no action and
/// the card ships exactly as it does without this.
pub fn herdr_link(pane: &str) -> Option<String> {
    crate::safety::pane_is_safe(pane).then(|| format!("moshi://herdr?pane={pane}"))
}

/// The webhook body: token, title, and the PREVIEW as the message, because
/// the phone card has a length ceiling the full message ignores, plus the
/// optional deep link the card's tap follows.
pub fn webhook_body(token: &str, title: &str, preview: &str, link: Option<&str>) -> String {
    let mut body = serde_json::json!({ "token": token, "title": title, "message": preview });
    if let Some(link) = link {
        // ONE `data` object carrying ONE `type`, which is what makes a url
        // action and an image action mutually exclusive: a structural limit of
        // the field, not a rule moshi states.
        body["data"] = serde_json::json!({ "type": "url", "url": link });
    }
    body.to_string()
}

/// The native moshi plugin.
pub struct MoshiChannel<H: HttpPost> {
    pub http: H,
    /// The token, read from the config at the composition root. None is the
    /// not-set-up case, which delivers nothing.
    pub token: Option<String>,
    /// `PNS_MOSHI_URL` override, else the default.
    pub url: String,
}

impl<H: HttpPost> MoshiChannel<H> {
    /// WHETHER THE PUSH LANDED, NEVER WHAT IT CARRIED. The channel used to be
    /// silent on the reasoning that the only thing worth reporting would be
    /// the request holding the token; the verdict says nothing about the
    /// request, so the secret stays where it was and a hand-run check can
    /// finally learn that the phone leg is broken.
    ///
    /// NO EVENT HEARS ANY OF IT. `ReportOutcome` is produced only under
    /// `--remote-only`, which selects durable plugins, and this one is not
    /// durable, so these sentences are unreachable from an event's stdout.
    pub fn deliver(&self, event: &Event, _mode: ReportMode) -> Delivery {
        let Some(token) = &self.token else {
            return Delivery::Failed(NO_TOKEN_LINE.to_string());
        };
        if self.http.post_json(
            &self.url,
            &webhook_body(
                token,
                &event.title,
                &event.preview,
                herdr_link(&event.pane).as_deref(),
            ),
        ) {
            Delivery::Delivered("pushed the card".to_string())
        } else {
            // WHY IS NOT KNOWN, and the sentence says so rather than picking
            // one: the seam answers a bool, so a refusal and an unreachable
            // endpoint arrive here identically.
            Delivery::Failed(
                "push FAILED (the moshi endpoint refused it or could not be reached)".to_string(),
            )
        }
    }
}

/// The line for a channel that was selected and never set up. It names the
/// config key to write, the way hermes's does, because "not set up" without an
/// address sends the operator hunting.
const NO_TOKEN_LINE: &str =
    "push SKIPPED -- no moshi token in the config ([plugins.mobile] token); nothing was sent";

/// The line for a mobile leg refused before either delivery seam: the table
/// names a backend nothing compiled in answers.
///
/// THE SAME SHAPE AS `NO_TOKEN_LINE` ABOVE IT, and deliberately beside it,
/// because the two are the same news in the operator's terms: the leg was
/// selected, nothing was sent, and here is the config to fix. What differs is
/// only which key is wrong, and a report that named `token` for a `type` fault
/// sends them to the one edit that is already correct.
pub fn refused_backend_line(reason: &str) -> String {
    format!("push SKIPPED -- {reason}; nothing was sent")
}

/// The deadline one moshi post runs under. Nobody waits on the answer and
/// nothing is retried, so this only bounds how long the process lingers.
pub const POST_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// The production POST: one agent, one deadline, no retry. Every failure is
/// `false` and nothing is logged, because the only thing worth reporting
/// would be the request that carries the token.
pub struct UreqPost {
    /// The whole-request deadline. Production uses the default; tests hand
    /// in a short one to prove the deadline actually fires.
    pub timeout: std::time::Duration,
}

impl Default for UreqPost {
    fn default() -> Self {
        Self {
            timeout: POST_DEADLINE,
        }
    }
}

impl HttpPost for UreqPost {
    fn post_json(&self, url: &str, body: &str) -> bool {
        ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            // The bash curl carried no -L, and following one would send the
            // token to whatever host the endpoint names. Zero returns the 3xx
            // as the response rather than an error, so the post simply ends.
            .max_redirects(0)
            .build()
            .new_agent()
            .post(url)
            .content_type("application/json")
            .send(body)
            // NOT `is_ok`. With no redirects followed, a 3xx comes back as a
            // RESPONSE rather than an error, so `is_ok` answered true for a
            // card the endpoint bounced somewhere else and never delivered.
            .is_ok_and(|response| DELIVERED_STATUS.contains(&response.status().as_u16()))
    }
}

/// The status codes that mean the card reached the phone. Spelled here rather
/// than shared with hermes: the two channels answer to different endpoints and
/// a range moved for one of them must not follow the other.
const DELIVERED_STATUS: std::ops::Range<u16> = 200..300;

#[cfg(test)]
mod tests;
