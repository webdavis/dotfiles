//! One conditional request to the notifications API.
//!
//! THE CONDITIONAL REQUEST IS THE WHOLE DESIGN. The documentation states that
//! "notifications are optimized for polling with the `Last-Modified` header.
//! If there are no new notifications, you will see a `304 Not Modified`
//! response, leaving your current rate limit untouched", and that
//! `X-Poll-Interval` "specifies how often (in seconds) you are allowed to
//! poll ... Please obey the header." A poll that sends no
//! `If-Modified-Since`, or that ignores the interval, is a poll that spends
//! the rate limit this one does not.

use pns_domain::github::notifications::NotificationThread;
use pns_domain::github::poll::Answer;

/// What one tick learned.
#[derive(Debug, PartialEq, Eq)]
pub enum Polled {
    /// `304`: nothing happened, and the interval to use next time. The cursor
    /// is unchanged and the rate limit was not touched.
    NotModified { interval_secs: Option<u64> },
    /// `200`: the listing, newest first, with the cursor and interval to carry
    /// forward.
    Listed {
        threads: Vec<NotificationThread>,
        answer: Answer,
    },
    /// `401`, or a `403` that is not the rate limit: a CONFIGURATION problem,
    /// and deliberately not an empty listing. A token that expired, was
    /// revoked, or never carried the `notifications` scope leaves a source
    /// that looks alive and reports nothing, which is the one failure this
    /// whole feature cannot afford to be quiet about.
    Unauthorized { status: u16 },
    /// `403` with the rate limit exhausted. NOT the same fact as the one
    /// above: the token is fine and nothing about the config needs editing,
    /// so a message sending the operator to the vault would be wrong.
    RateLimited,
    /// Anything else: a `5xx`, an unreachable host, a body that would not
    /// read. Transient by assumption, so the next tick tries again.
    Unavailable { detail: String },
}

/// The notifications listing, read by one conditional request.
pub struct GithubNotifications {
    /// The agent every call rides, INJECTED so a test can hand in one wearing
    /// a scripted transport: the production pipeline runs for real and only
    /// the wire is fake.
    agent: ureq::Agent,
    /// The API root, `https://api.github.com` in production.
    base: String,
    /// The classic personal access token, from `[plugins.github] personal_access_token`.
    token: String,
}

/// One bounded fetch. The daemon spawns this poll under its own child bound,
/// so the transport's deadline is what keeps a wedged request inside it.
pub const GITHUB_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// The most of one listing that is read into memory.
///
/// Fifty threads (the documented `per_page` ceiling) of a few hundred bytes
/// each measures tens of kilobytes, so this is generous and still a bound: a
/// proxy streaming garbage costs at most this much before reading unavailable.
pub const GITHUB_BODY_CAP: u64 = 2_000_000;

/// The most pages one poll follows, even when the server keeps offering a
/// `next` link.
///
/// TEN, because notifications are never marked read, so an account can carry
/// more than one page of unread threads for a long time: ten pages of fifty
/// is five hundred threads, generous for a backlog and still a bound against
/// a poll that never returns.
pub const GITHUB_MAX_PAGES: usize = 10;

/// The API version this build speaks, sent on every request as the
/// documentation asks.
const API_VERSION: &str = "2022-11-28";

impl GithubNotifications {
    /// The production wiring: TLS verified (unlike the bridge and the router,
    /// this is a public host with a real certificate), no redirects, and the
    /// deadline on every call.
    pub fn new(token: String) -> Self {
        Self::with_agent(
            Self::production_config().new_agent(),
            "https://api.github.com".to_string(),
            token,
        )
    }

    /// The agent semantics every poll runs under, NAMED so the scripted
    /// transport can run the same ones: a test that built its own config
    /// would pass while production followed a redirect, which is how a
    /// bearer token reaches a host nobody meant to send it to.
    pub fn production_config() -> ureq::config::Config {
        ureq::Agent::config_builder()
            .timeout_global(Some(GITHUB_DEADLINE))
            .max_redirects(0)
            .build()
    }

    /// The same reader over any agent and any base: the seam the scripted
    /// transport injects through.
    pub fn with_agent(agent: ureq::Agent, base: String, token: String) -> Self {
        Self { agent, base, token }
    }

    /// One tick, against the cursor the last 200 answered with.
    ///
    /// `participating=false` IS THE POINT OF THE ENDPOINT: the default is
    /// already everything the account watches, and narrowing to participating
    /// threads would drop the `ci_activity` notifications this exists for.
    /// It is stated rather than left implicit so a default change upstream
    /// cannot silently narrow it. `per_page=50` is the documented ceiling,
    /// stated for the same reason.
    ///
    /// A LISTING IS FOLLOWED ACROSS PAGES, bounded by `GITHUB_MAX_PAGES`,
    /// because notifications are never marked read: an account with more
    /// unread threads than one page holds would otherwise never see the
    /// oldest of them, page one being newest-first. Only the FIRST page's
    /// request carries `If-Modified-Since` and only its own cursor and
    /// interval headers are kept; a `next` link is an unconditional
    /// continuation of the one listing, not a second resource.
    pub fn poll(&self, last_modified: &str) -> Polled {
        let mut url = format!(
            "{}/notifications?participating=false&per_page=50",
            self.base
        );
        let mut threads = Vec::new();
        let mut answer = Answer::default();
        for page in 1..=GITHUB_MAX_PAGES {
            let mut request = self
                .agent
                .get(&url)
                .header("Authorization", &format!("Bearer {}", self.token))
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", API_VERSION);
            if page == 1 && !last_modified.is_empty() {
                request = request.header("If-Modified-Since", last_modified);
            }
            let response = match request.call() {
                Ok(response) => response,
                // A STATUS IS NOT A TRANSPORT FAILURE. ureq answers `Err` for
                // a 4xx or 5xx, and the four cases below are the whole reason
                // this poll reports something other than "nothing happened".
                Err(ureq::Error::StatusCode(status)) => return refused(status, None),
                Err(error) => {
                    return Polled::Unavailable {
                        detail: error.to_string(),
                    };
                }
            };
            let status = response.status().as_u16();
            if page == 1 && status == 304 {
                let interval_secs =
                    header(&response, "x-poll-interval").and_then(|it| it.parse().ok());
                return Polled::NotModified { interval_secs };
            }
            if status != 200 {
                // A 2xx or 3xx nobody expects, and the rate-limit header with
                // it in case a future build answers 403 through this arm.
                return refused(status, header(&response, "x-ratelimit-remaining"));
            }
            let next = header(&response, "link").and_then(|link| next_page(&link));
            if page == 1 {
                answer.last_modified = header(&response, "last-modified").unwrap_or_default();
                answer.interval_secs =
                    header(&response, "x-poll-interval").and_then(|it| it.parse().ok());
            }
            match read_page(response) {
                Ok(mut page_threads) => threads.append(&mut page_threads),
                Err(polled) => return polled,
            }
            match next {
                Some(next_url) => url = next_url,
                None => break,
            }
        }
        Polled::Listed { threads, answer }
    }
}

/// One page's body, read into the threads it carries.
fn read_page(
    mut response: ureq::http::Response<ureq::Body>,
) -> Result<Vec<NotificationThread>, Polled> {
    match response
        .body_mut()
        .with_config()
        .limit(GITHUB_BODY_CAP)
        .read_to_string()
    {
        Ok(body) => Ok(super::notifications::notification_threads(&body)),
        Err(error) => Err(Polled::Unavailable {
            detail: error.to_string(),
        }),
    }
}

/// The `rel="next"` target off a `Link` header, or nothing when this is the
/// last page.
///
/// THE SERVER'S OWN URL, VERBATIM, never rebuilt from a page number this
/// crate would have to keep in step with `per_page`.
fn next_page(link_header: &str) -> Option<String> {
    link_header.split(',').find_map(|entry| {
        let mut segments = entry.split(';');
        let url = segments
            .next()?
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>');
        segments
            .any(|attribute| attribute.trim() == r#"rel="next""#)
            .then(|| url.to_string())
    })
}

/// Which refusal a status is.
///
/// A 403 SPLITS ON THE RATE-LIMIT HEADER, because the two facts behind one
/// status need two different sentences: `x-ratelimit-remaining: 0` is a
/// budget that refills by itself, and anything else is a token whose scope or
/// validity the operator has to go and fix. ureq's `StatusCode` error carries
/// no headers, so a 403 arriving that way is read as the configuration
/// problem: it is the louder of the two, and a rate limit misreported as one
/// costs a log line the 304s make near-unreachable anyway.
fn refused(status: u16, rate_limit_remaining: Option<String>) -> Polled {
    match (status, rate_limit_remaining.as_deref()) {
        (403, Some("0")) => Polled::RateLimited,
        (401 | 403, _) => Polled::Unauthorized { status },
        _ => Polled::Unavailable {
            detail: format!("the notifications API answered {status}"),
        },
    }
}

fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

#[cfg(test)]
mod tests;
