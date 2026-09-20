//! The one HTTP request this destination ever composes, and the seam that
//! sends it.
//!
//! COMPOSED PURELY, SENT BEHIND A TRAIT, which is what `SignedPost` already
//! does for hermes: every header and every byte of the body is decided by a
//! function with no socket in it, so the tests that pin the authorization, the
//! mandatory User-Agent and the empty `allowed_mentions` never talk to
//! Discord.

use pns_domain::retry::TransportOutcome;
use std::time::Duration;

/// The API version this posts against: v10 is Discord's recommended version
/// (docs.discord.com/developers/reference, read 2026-09-15).
pub const API_BASE: &str = "https://discord.com/api/v10";

/// The deadline one post runs under. Not configurable: the durable leg can be
/// posted synchronously, and this only bounds how long anybody waits on a
/// gateway that stopped answering.
pub const POST_DEADLINE: Duration = Duration::from_secs(10);

/// The User-Agent every call carries. MANDATORY, not decoration: "requests
/// without a valid User-Agent may be blocked and return Cloudflare errors"
/// (same reference page), and Discord specifies the
/// `DiscordBot (<url>, <version>)` form.
pub fn user_agent() -> String {
    format!(
        "DiscordBot (https://github.com/webdavis/dotfiles, {})",
        env!("CARGO_PKG_VERSION")
    )
}

/// The three headers every call carries: the bot authorization, the JSON
/// content type, and the mandatory User-Agent.
fn headers(token: &str) -> Vec<(String, String)> {
    vec![
        ("Authorization".to_string(), format!("Bot {token}")),
        ("Content-Type".to_string(), "application/json".to_string()),
        ("User-Agent".to_string(), user_agent()),
    ]
}

/// One composed call: where it goes, what it carries, and the headers that
/// authorize it.
///
/// NO `Debug`, for `HermesKeys`'s reason: the authorization header holds the
/// bot token, so this type cannot ride a formatted dump into a log line.
pub struct DiscordRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// A message posted to one channel: `POST /channels/{id}/messages` with a
/// `content` body (docs.discord.com/developers/resources/message).
///
/// `allowed_mentions` IS ALWAYS `{"parse": []}`. The default for a regular
/// message parses all mention types, users, roles and `@everyone` included, and
/// an agent quoting a branch name or a diff must not page the guild.
pub fn message(token: &str, channel_id: &str, content: &str) -> DiscordRequest {
    DiscordRequest {
        url: format!("{API_BASE}/channels/{channel_id}/messages"),
        headers: headers(token),
        body: serde_json::json!({
            "content": content,
            "allowed_mentions": {"parse": []},
        })
        .to_string(),
    }
}

/// A thread opened on one message: `POST /channels/{id}/messages/{id}/threads`
/// (docs.discord.com/developers/resources/channel, read 2026-09-15), which
/// answers the thread as a CHANNEL object, so every later post to it is an
/// ordinary `message` call against the id it returns.
///
/// `auto_archive_duration` IS 1440 MINUTES, a day: a week would keep every
/// session of the last seven days in the project channel's active list, and
/// archival costs nothing, since Discord unarchives a thread on send.
pub fn create_thread(
    token: &str,
    channel_id: &str,
    message_id: &str,
    name: &str,
) -> DiscordRequest {
    DiscordRequest {
        url: format!("{API_BASE}/channels/{channel_id}/messages/{message_id}/threads"),
        headers: headers(token),
        body: serde_json::json!({
            "name": name,
            "auto_archive_duration": 1440,
        })
        .to_string(),
    }
}

/// What one call answered: the outcome the ledger classifies, and the body
/// the caller reads an id or an error code out of.
///
/// THE BODY IS CARRIED BECAUSE THREADING NEEDS IT: the id of the message a
/// thread is opened on, and the error code that says a thread is gone, are
/// both in it. It is Discord's own JSON and never a rendered line, so nothing
/// here reaches an operator's screen.
pub struct DiscordReply {
    pub outcome: TransportOutcome,
    pub body: String,
}

/// The `id` field of a message or channel object Discord just answered with.
pub fn id_of(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get("id")?
        .as_str()
        .map(str::to_string)
}

/// Discord's own error code, the number beside the status that says WHICH
/// refusal this was (docs.discord.com/developers/topics/opcodes-and-status-codes).
pub fn error_code(body: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get("code")?
        .as_u64()
}

/// The POST seam: a composed request in, the outcome and the body out. The
/// production impl honors the deadline; a fake records the request it was
/// handed.
pub trait DiscordPost {
    fn post(&self, request: &DiscordRequest) -> DiscordReply;
}

/// The production POST: one agent, no redirects (following one would send the
/// bot token to whatever host the redirect names), the deadline per call.
pub struct UreqDiscordPost;

/// The most of an answer this reads. Discord's message and channel objects
/// are well under it, and the cap is what keeps a wrong host on the other end
/// of the URL from being read into memory whole.
const MAX_REPLY_BYTES: u64 = 64 * 1024;

impl DiscordPost for UreqDiscordPost {
    fn post(&self, request: &DiscordRequest) -> DiscordReply {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(POST_DEADLINE))
            .max_redirects(0)
            // A REFUSAL IS A RESPONSE HERE, not an error: the body beside a
            // 404 carries the code that says a thread was deleted rather than
            // a channel mistyped, and ureq's default would throw it away.
            .http_status_as_error(false)
            .build()
            .new_agent();
        let mut call = agent.post(&request.url);
        for (name, value) in &request.headers {
            call = call.header(name.as_str(), value.as_str());
        }
        match call.send(&request.body) {
            Ok(mut response) => DiscordReply {
                outcome: TransportOutcome::Status(response.status().as_u16()),
                body: response
                    .body_mut()
                    .with_config()
                    .limit(MAX_REPLY_BYTES)
                    .read_to_string()
                    .unwrap_or_default(),
            },
            // Never put on the wire: a URI ureq refuses, or a header the http
            // crate refuses to build. No amount of waiting produces a status.
            Err(error) => DiscordReply {
                outcome: match error {
                    ureq::Error::BadUri(_) | ureq::Error::Http(_) => TransportOutcome::NoStatus,
                    _ => TransportOutcome::NoResponse,
                },
                body: String::new(),
            },
        }
    }
}
