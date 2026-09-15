//! The one HTTP request this destination ever composes, and the seam that
//! sends it.
//!
//! COMPOSED PURELY, SENT BEHIND A TRAIT, which is what `SignedPost` already
//! does for hermes: every header and every byte of the body is decided by a
//! function with no socket in it, so the tests that pin the authorization, the
//! mandatory User-Agent and the empty `allowed_mentions` never talk to
//! Discord.

use pns_domain::retry::DeliveryOutcome;
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
        headers: vec![
            ("Authorization".to_string(), format!("Bot {token}")),
            ("Content-Type".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), user_agent()),
        ],
        body: serde_json::json!({
            "content": content,
            "allowed_mentions": {"parse": []},
        })
        .to_string(),
    }
}

/// The POST seam: a composed request in, a domain outcome out. The production
/// impl honors the deadline; a fake records the request it was handed.
pub trait DiscordPost {
    fn post(&self, request: &DiscordRequest) -> DeliveryOutcome;
}

/// The production POST: one agent, no redirects (following one would send the
/// bot token to whatever host the redirect names), the deadline per call.
pub struct UreqDiscordPost;

impl DiscordPost for UreqDiscordPost {
    fn post(&self, request: &DiscordRequest) -> DeliveryOutcome {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(POST_DEADLINE))
            .max_redirects(0)
            .build()
            .new_agent();
        let mut call = agent.post(&request.url);
        for (name, value) in &request.headers {
            call = call.header(name.as_str(), value.as_str());
        }
        match call.send(&request.body) {
            Ok(response) => DeliveryOutcome::Status(response.status().as_u16()),
            Err(ureq::Error::StatusCode(code)) => DeliveryOutcome::Status(code),
            // Never put on the wire: a URI ureq refuses, or a header the http
            // crate refuses to build. No amount of waiting produces a status.
            Err(ureq::Error::BadUri(_) | ureq::Error::Http(_)) => DeliveryOutcome::NoStatus,
            Err(_) => DeliveryOutcome::NoResponse,
        }
    }
}
