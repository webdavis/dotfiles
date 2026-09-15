//! The discord channel, native: the durable Discord log, one HTTPS POST
//! straight to a channel with no gateway in between.
//!
//! THE ALTERNATIVE TO hermes, NEVER A COMPANION. Both declare the same
//! routing, and enabling both is refused at config load, naming both tables,
//! because two durable channels post every event twice while the recap follows
//! whichever registered first.
//!
//! THE TOKEN'S PATH IS THE POINT. It is read from `[plugins.discord]`, placed
//! in the `Authorization` header of a type deriving no `Debug`, and never
//! reaches argv, a child's environment, or any line this module prints: a
//! failure names the STATUS and the config key, never the credential.
//!
//! CLASSIFICATION IS NOT THIS MODULE'S. A status is handed up as
//! `Delivery::Rejected` and `DeliveryOutcome::class` decides whether it can be
//! retried, which is what puts 429 and every 5xx on the ledger's backoff and
//! dead-letters 401, 403 and 404 on the FIRST attempt. There is deliberately
//! no `Retry-After` scheduler: a second schedule can disagree with the
//! ledger's about when a leg is due.

use super::{Delivery, Event};
use pns_application::{DeliveryRequest, DestinationId, NotificationDestination};
use pns_domain::registry::Routing;
use pns_domain::retry::DeliveryOutcome;

mod request;
pub use request::{DiscordPost, DiscordRequest, UreqDiscordPost, message};

/// The most a `content` field may carry, kept at the recap's own budget
/// rather than Discord's 2000 character ceiling: the 200 character margin is
/// what the shed rule below spends its dropped-line note out of.
const MAX_CONTENT_CHARS: usize = pns_domain::recap::budget::MAX_CHARS;

/// The three lines a post carries: the header, the dim subheader, and the
/// body they head.
///
/// COMPOSED HERE, off the event's own parts, because this is the one
/// destination that renders them for this transport. `joined`'s rule that an
/// empty part takes its separator with it already keeps a missing branch out
/// of the header, and an empty line is dropped rather than posted blank.
pub fn content(event: &Event) -> String {
    let header = pns_domain::render::header(&event.project, &event.branch, &event.state);
    let subheader = pns_domain::render::subheader(
        &event.agent,
        &pns_domain::render::short_session(&event.session),
        &event.session_title,
    );
    let mut lines = Vec::with_capacity(3);
    if !header.is_empty() {
        lines.push(format!("**{header}**"));
    }
    if !subheader.is_empty() {
        lines.push(format!("-# {subheader}"));
    }
    lines.extend(event.detail.lines().map(str::to_string));
    fitted(lines)
}

/// The lines that fit, with a note counting whatever was shed.
///
/// THE RECAP'S OWN SHED RULE: whole lines go from the END, never half a line,
/// and the count of what went is appended so a truncated post says it was
/// truncated. The header and the subheader are the first two lines and are
/// therefore the last to go, which is what keeps the one line a push preview
/// shows.
fn fitted(lines: Vec<String>) -> String {
    let whole = lines.join("\n");
    if whole.chars().count() <= MAX_CONTENT_CHARS {
        return whole;
    }
    let mut kept: Vec<String> = Vec::new();
    let mut spent = 0;
    for line in &lines {
        // The note is the last line and needs room of its own; 40 characters
        // is more than "(N more lines dropped)" can ever spend.
        let next = spent + line.chars().count() + 1;
        if next + 40 > MAX_CONTENT_CHARS {
            break;
        }
        spent = next;
        kept.push(line.clone());
    }
    let dropped = lines.len() - kept.len();
    kept.push(format!("({dropped} more lines dropped)"));
    kept.join("\n")
}

/// The native discord plugin.
pub struct DiscordChannel<P: DiscordPost> {
    pub post: P,
    /// The bot token, read from `[plugins.discord]` at the composition root.
    /// None is the not-set-up case, which posts nothing and says so.
    pub token: Option<String>,
    /// The channel id every post goes to, read from
    /// `[plugins.discord.channels] default`. None is the same not-set-up case
    /// by a different missing key, and the sentence says which.
    pub channel_id: Option<String>,
}

impl<P: DiscordPost + Send + Sync> NotificationDestination for DiscordChannel<P> {
    fn id(&self) -> &DestinationId {
        const ID: DestinationId = DestinationId::new("discord");
        &ID
    }

    fn capabilities(&self) -> Routing {
        Routing {
            local: false,
            presence_gated: false,
            durable: true,
            event_dispatched: true,
        }
    }

    fn deliver(&self, request: &DeliveryRequest<'_>) -> Delivery {
        // NOT SET UP IS A FAILED VERDICT, for `HermesChannel`'s reason: from
        // the record's point of view it reads the same as a refusal, and an
        // empty Discord channel otherwise looks like the jobs stopped. The
        // sentence names the KEY to write, never the value that is missing.
        let (Some(token), Some(channel_id)) = (self.token.as_deref(), self.channel_id.as_deref())
        else {
            return Delivery::Failed(skipped_line(self.token.is_none()));
        };
        let outcome = self
            .post
            .post(&message(token, channel_id, &content(request.event)));
        let line = outcome_line(outcome);
        if outcome.delivered() {
            return Delivery::Delivered(line);
        }
        match outcome {
            // The channel REPORTS the status; `DeliveryOutcome::class` decides
            // whether trying again could ever help, once, where the outcome is
            // recorded, so a second destination cannot disagree with this one
            // about a 404.
            DeliveryOutcome::Status(status) => Delivery::Rejected {
                status,
                detail: line,
            },
            DeliveryOutcome::NoStatus | DeliveryOutcome::NoResponse => Delivery::Failed(line),
        }
    }
}

/// What one attempt had to say, the STATUS and nothing else: the request
/// carried the token, so nothing about the request is ever printed.
fn outcome_line(outcome: DeliveryOutcome) -> String {
    match outcome {
        DeliveryOutcome::Status(status) if outcome.delivered() => {
            format!("posted to discord HTTP {status}")
        }
        DeliveryOutcome::Status(status) => format!("discord post FAILED HTTP {status}"),
        DeliveryOutcome::NoResponse => {
            "discord post FAILED (no response from discord.com)".to_string()
        }
        DeliveryOutcome::NoStatus => {
            "discord post FAILED (the request was never sendable)".to_string()
        }
    }
}

/// The line for a channel that was selected and never set up, naming the one
/// key to write.
fn skipped_line(no_token: bool) -> String {
    let key = if no_token {
        "[plugins.discord] token"
    } else {
        "[plugins.discord.channels] default"
    };
    format!("discord post SKIPPED, no {key} in the config; nothing was sent")
}

/// The line for a discord leg refused before the seam: the table names a
/// transport nothing compiled in answers.
///
/// THE SAME SHAPE AS `skipped_line` ABOVE IT, because the two are the same news
/// in the operator's terms: the leg was selected, nothing was sent, and here is
/// the config to fix. What differs is only which key is wrong.
pub fn refused_discord_line(reason: &str) -> String {
    format!("discord post SKIPPED, {reason}; nothing was sent")
}

#[cfg(test)]
#[path = "discord/tests.rs"]
mod tests;
