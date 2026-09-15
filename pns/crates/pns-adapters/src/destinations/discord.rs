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
use pns_domain::channel_map::{ChannelMap, channel_for};
use pns_domain::registry::Routing;
use pns_domain::retry::DeliveryOutcome;

mod request;
mod threads;
pub use request::{DiscordPost, DiscordReply, DiscordRequest, UreqDiscordPost, message};
use request::{create_thread, id_of};
pub use threads::SessionThreads;
use threads::{thread_is_gone, thread_name};

/// The state a recap is raised with. A RECAP OPENS NO THREAD: it is a window
/// of time rather than a session, and threading it would bury the one message
/// a day the operator most wants at channel level.
const RECAP_STATE: &str = "recap";

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
/// THE RECAP'S OWN SHED RULE: lines go from the END, and the count of what
/// went is appended so a truncated post says it was truncated. The header and
/// the subheader are the first two lines and are therefore the last to go,
/// which is what keeps the one line a push preview shows. The first line that
/// does not fit whole is CLIPPED to what remains rather than dropped outright,
/// so a single oversized paste still shows its start instead of vanishing
/// under the note alone.
fn fitted(lines: Vec<String>) -> String {
    let whole = lines.join("\n");
    if whole.chars().count() <= MAX_CONTENT_CHARS {
        return whole;
    }
    // The note is the last line and needs room of its own; 40 characters
    // is more than "(N more lines dropped)" can ever spend.
    const NOTE_ROOM: usize = 40;
    let mut kept: Vec<String> = Vec::new();
    let mut spent = 0;
    let mut shown = lines.len();
    for (index, line) in lines.iter().enumerate() {
        let ceiling = MAX_CONTENT_CHARS.saturating_sub(NOTE_ROOM);
        let next = spent + line.chars().count() + 1;
        if next <= ceiling {
            spent = next;
            kept.push(line.clone());
            continue;
        }
        shown = index;
        let room = ceiling.saturating_sub(spent);
        if room > 1 {
            kept.push(pns_domain::render::clipped(line, room - 1));
        }
        break;
    }
    let dropped = lines.len() - shown;
    if dropped > 0 {
        let noun = if dropped == 1 { "line" } else { "lines" };
        kept.push(format!("({dropped} more {noun} dropped)"));
    }
    kept.join("\n")
}

/// The native discord plugin.
pub struct DiscordChannel<P: DiscordPost> {
    pub post: P,
    /// The bot token, read from `[plugins.discord]` at the composition root.
    /// None is the not-set-up case, which posts nothing and says so.
    pub token: Option<String>,
    /// `[plugins.discord.channels]` whole, because the channel is decided per
    /// EVENT rather than per process: one map, one lookup, and no branch here.
    pub channels: ChannelMap,
    /// The route this leg was submitted on, taken at construction the way
    /// `HermesChannel`'s is and for the same reason: the submission and the
    /// retry both build their destinations from the leg's own route, while a
    /// `DeliveryRequest` carries an empty one on the paths that never reached
    /// the ledger.
    pub route: String,
    /// What this machine calls its default route (`[routes] default`), which
    /// is the map key an event with NO PROJECT lands on and the one route the
    /// lookup does not try ahead of the project. Taken at construction
    /// because the composition root is where the config is read.
    pub default_route: String,
    /// Where the thread this session already owns in a channel is kept. A
    /// `dyn` seam for the tests' reason and no other: the production value is
    /// always the sqlite store.
    pub threads: Box<dyn SessionThreads>,
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
        let Some(token) = self.token.as_deref() else {
            return Delivery::Failed(skipped_line(true));
        };
        // THE SUBJECT PICKS THE CHANNEL, and the route the severity already
        // chose picks it first: the order is the domain's, so this destination
        // holds no policy of its own.
        let Some(channel_id) = channel_for(
            &self.channels,
            &self.route,
            &request.event.project,
            &self.default_route,
        ) else {
            return Delivery::Failed(skipped_line(false));
        };
        let reply = self.posted(token, channel_id, request.event);
        let outcome = reply.outcome;
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

impl<P: DiscordPost> DiscordChannel<P> {
    /// Where this event goes: the thread its session already owns in this
    /// channel, or the channel itself.
    ///
    /// THE WHOLE RECOVERY HAPPENS HERE, before `deliver` answers and therefore
    /// before the ledger records anything: an event whose thread was deleted
    /// or locked is reposted to the channel in the same call, so the record
    /// says delivered because it was.
    fn posted(&self, token: &str, channel_id: &str, event: &Event) -> DiscordReply {
        let content = content(event);
        let session = event.session.as_str();
        if session.is_empty() || event.state == RECAP_STATE {
            return self.post.post(&message(token, channel_id, &content));
        }
        if let Some(thread) = self.threads.thread(session, channel_id) {
            let reply = self.post.post(&message(token, &thread, &content));
            if !thread_is_gone(&reply) {
                return reply;
            }
            self.threads.forget(session, channel_id);
        }
        self.opening(token, channel_id, event, &content)
    }

    /// The first event of a pair: post to the channel, then open a thread on
    /// the message that post returned.
    ///
    /// THE MESSAGE IS THE DELIVERY and its reply is what comes back, whatever
    /// the thread call did: the event has landed in the channel either way,
    /// and a pair with no row simply opens its thread on the next event.
    fn opening(&self, token: &str, channel_id: &str, event: &Event, content: &str) -> DiscordReply {
        let reply = self.post.post(&message(token, channel_id, content));
        if !reply.outcome.delivered() {
            return reply;
        }
        let Some(message_id) = id_of(&reply.body) else {
            return reply;
        };
        let name = thread_name(&event.project, &event.branch, &event.state);
        let opened = self
            .post
            .post(&create_thread(token, channel_id, &message_id, &name));
        if opened.outcome.delivered()
            && let Some(thread) = id_of(&opened.body)
        {
            self.threads.remember(&event.session, channel_id, &thread);
        }
        reply
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
///
/// THE MAP'S FAILURE IS THE CATCH-ALL'S, always: every lookup ends at
/// `default`, so a map that answered nothing is a map missing that one key
/// rather than a project nobody mapped.
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
#[path = "discord/double.rs"]
mod double;
#[cfg(test)]
#[path = "discord/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "discord/thread_tests.rs"]
mod thread_tests;
