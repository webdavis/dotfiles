//! One thread per (session, channel) pair: where the id is kept, when a new
//! thread is opened, and what counts as a thread that is gone.
//!
//! THE PAIR IS THE KEY. A session that posts from two checkouts resolves two
//! channels and owns one thread in each, holding the events that channel was
//! about.
//!
//! ARCHIVAL NEEDS NO HANDLING AT ALL: "sending a message will automatically
//! unarchive the thread, unless the thread has been locked by a moderator"
//! (docs.discord.com/developers/topics/threads, read 2026-09-15). A thread
//! that is DELETED answers code 10003, one that is ARCHIVED and cannot
//! auto-unarchive on send answers 50083, and one that is LOCKED answers
//! 160005, and the bot holds no Manage Threads permission, so it can unlock
//! neither: all three take the one path, the stored row dropped and a fresh
//! thread opened from a new message.

use super::request::{DiscordReply, error_code};

/// Where a pair's thread id is kept between events.
///
/// SWALLOWED FAILURE IS THE CONTRACT, not an oversight of the implementer: a
/// store that cannot answer costs an event its thread and nothing else.
pub trait SessionThreads: Send + Sync {
    fn thread(&self, session: &str, channel: &str) -> Option<String>;
    fn remember(&self, session: &str, channel: &str, thread: &str);
    fn forget(&self, session: &str, channel: &str);
}

/// The most a thread name may carry: Discord's own channel name ceiling
/// (docs.discord.com/developers/resources/channel).
pub const MAX_THREAD_NAME_CHARS: usize = 100;

/// What a thread is called: the header's own first line, so the thread list
/// and the message that opened it read the same.
pub fn thread_name(project: &str, branch: &str, state: &str) -> String {
    pns_domain::render::clipped(
        &pns_domain::render::header(project, branch, state),
        MAX_THREAD_NAME_CHARS,
    )
}

/// Whether this refusal means the stored thread can never be posted to again.
///
/// THE CODE DECIDES, NOT THE STATUS: 10003 (unknown channel) arrives as a
/// 404 a mistyped channel id would also earn, and 50083 (thread archived,
/// cannot auto-unarchive) and 160005 (thread locked) arrive beside a 403. On
/// any of the three the row is dropped and the event is reposted to the
/// channel, which is the trade `channel_url` already makes for an unusable
/// route name: a message in a second-choice place beats a message nowhere.
pub fn thread_is_gone(reply: &DiscordReply) -> bool {
    matches!(error_code(&reply.body), Some(10003 | 50083 | 160005))
}
