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

use super::Delivery;
use pns_application::{DeliveryRequest, DestinationId, NotificationDestination};
use pns_domain::registry::Routing;

/// Where the push goes when `PNS_MOSHI_URL` says nothing.
pub const DEFAULT_MOSHI_URL: &str = "https://api.getmoshi.app/api/webhook";

/// The POST seam: one call, a URL and a JSON body in, success or not out.
/// The production impl carries the 10 second deadline; a fake records.
pub trait HttpPost {
    fn post_json(&self, url: &str, body: &str) -> bool;
}

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
    pns_domain::safety::pane_is_safe(pane).then(|| format!("moshi://herdr?pane={pane}"))
}

/// The webhook body: token, title, and the PREVIEW as the message, because
/// the phone card has a length ceiling the full message ignores, plus the
/// optional deep link and the original request id in the data object.
pub fn webhook_body(
    token: &str,
    title: &str,
    preview: &str,
    link: Option<&str>,
    request_id: &str,
) -> String {
    body_with_id(token, title, preview, link, Some(request_id))
}

fn body_with_id(
    token: &str,
    title: &str,
    preview: &str,
    link: Option<&str>,
    request_id: Option<&str>,
) -> String {
    let mut body = serde_json::json!({ "token": token, "title": title, "message": preview });
    if let Some(id) = request_id {
        body["data"] = serde_json::json!({"request_id": id});
    }
    if let Some(link) = link {
        // ONE `data` object carrying ONE `type`, which is what makes a url
        // action and an image action mutually exclusive: a structural limit of
        // the field, not a rule moshi states.
        body["data"]["type"] = serde_json::json!("url");
        body["data"]["url"] = serde_json::json!(link);
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

impl<H: HttpPost + Send + Sync> NotificationDestination for MoshiChannel<H> {
    fn id(&self) -> &DestinationId {
        const ID: DestinationId = DestinationId::new("mobile");
        &ID
    }

    fn capabilities(&self) -> Routing {
        Routing {
            local: false,
            presence_gated: true,
            durable: false,
            event_dispatched: true,
        }
    }

    /// WHETHER THE PUSH LANDED, NEVER WHAT IT CARRIED. The channel used to be
    /// silent on the reasoning that the only thing worth reporting would be
    /// the request holding the token; the verdict says nothing about the
    /// request, so the secret stays where it was and a hand-run check can
    /// finally learn that the phone leg is broken.
    ///
    /// NO EVENT HEARS ANY OF IT. `ReportOutcome` is produced only under
    /// `--remote-only`, which selects durable plugins, and this one is not
    /// durable, so these sentences are unreachable from an event's stdout.
    fn deliver(&self, request: &DeliveryRequest<'_>) -> Delivery {
        let event = request.event;
        let Some(token) = &self.token else {
            return Delivery::Failed(NO_TOKEN_LINE.to_string());
        };
        if self.http.post_json(
            &self.url,
            &body_with_id(
                token,
                &event.title,
                &event.preview,
                herdr_link(&event.pane).as_deref(),
                request.request_id,
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
    "push SKIPPED, no moshi token in the config ([plugins.mobile] token); nothing was sent";

/// The line for a mobile leg refused before either delivery seam: the table
/// names a backend nothing compiled in answers.
///
/// THE SAME SHAPE AS `NO_TOKEN_LINE` ABOVE IT, and deliberately beside it,
/// because the two are the same news in the operator's terms: the leg was
/// selected, nothing was sent, and here is the config to fix. What differs is
/// only which key is wrong, and a report that named `token` for a `type` fault
/// sends them to the one edit that is already correct.
pub fn refused_backend_line(reason: &str) -> String {
    format!("push SKIPPED, {reason}; nothing was sent")
}

mod http;
pub use http::{POST_DEADLINE, UreqPost};

#[cfg(test)]
mod tests;
