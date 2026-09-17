//! What the poll remembers between ticks, and what it does with one answer.
//!
//! THREE THINGS IN ONE VALUE, because one atomic write is what makes "a poll
//! that fails loses nothing" true: the conditional-request cursor, the
//! interval the server last asked for, and the identities already reported.
//! A state published in two files can be published half way.

/// One identity already reported, and when it was first seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub identity: String,
    pub first_seen: u64,
}

/// Everything one poll carries forward.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PollState {
    /// The `Last-Modified` the last 200 answered with, sent back as
    /// `If-Modified-Since`. Empty until the first answer.
    ///
    /// THE SERVER'S OWN TEXT, VERBATIM. It is an HTTP date this crate never
    /// parses: a cursor we reformatted would stop matching the resource and
    /// turn every free 304 into a full 200.
    pub last_modified: String,
    /// The `X-Poll-Interval` the server last asked for, in seconds.
    pub interval_secs: u64,
    /// The identities already reported, newest last.
    pub seen: Vec<Seen>,
}

/// How long an identity is remembered.
///
/// TWENTY-FOUR HOURS, sized to "a laptop asleep overnight" rather than
/// measured: it is longer than any plausible skew between a webhook (seconds)
/// and a poll (a minute), and short enough that the file stays small. An
/// EXPIRED entry costs one duplicate notification, which is the cheap
/// failure; no expiry at all costs a file that grows without bound and is
/// read on every tick, which is the expensive one.
pub const SEEN_FOR_SECS: u64 = 24 * 60 * 60;

/// The most identities one state holds, however recent they are.
///
/// A SECOND BOUND BESIDE THE WINDOW, because the window alone is a promise
/// about time and not about size: a repository storm inside one day would
/// otherwise grow the file without limit. The oldest go first, so the
/// duplicate this costs is of the event least likely to still be arriving.
pub const SEEN_MAX: usize = 2000;

/// The identities of `answered` this state has not reported yet, OLDEST
/// FIRST, and the state to publish once they are.
///
/// OLDEST FIRST, which is the order the notifications are worth reading in:
/// the API answers newest first, and a reader scrolling a channel wants the
/// run that finished first at the top.
///
/// THE STATE IS ANSWERED RATHER THAN MUTATED so the caller decides when it
/// becomes durable. Nothing here is published, which is what makes "a poll
/// that fails does not lose the cursor" a property of the caller's ordering
/// rather than a rollback.
pub fn advance(state: &PollState, answered: &Answer, now: u64) -> (Vec<String>, PollState) {
    let mut kept: Vec<Seen> = state
        .seen
        .iter()
        .filter(|seen| now.saturating_sub(seen.first_seen) < SEEN_FOR_SECS)
        .cloned()
        .collect();
    // THE FILTER READS WHAT THIS VERY ANSWER HAS ALREADY TAKEN, not only the
    // durable state: one listing can name the same thread twice across a page
    // boundary, and a duplicate inside one answer is the same duplicate.
    let mut fresh: Vec<String> = Vec::new();
    for identity in answered.identities.iter().rev() {
        let known = kept.iter().any(|seen| seen.identity == *identity)
            || fresh.iter().any(|taken| taken == identity);
        if !known {
            fresh.push(identity.clone());
        }
    }
    kept.extend(fresh.iter().map(|identity| Seen {
        identity: identity.clone(),
        first_seen: now,
    }));
    let over = kept.len().saturating_sub(SEEN_MAX);
    kept.drain(..over);
    (
        fresh,
        PollState {
            // AN ANSWER THAT STATED NO CURSOR KEEPS THE OLD ONE rather than
            // clearing it: a 200 without `Last-Modified` would otherwise turn
            // every later tick into a full listing.
            last_modified: if answered.last_modified.is_empty() {
                state.last_modified.clone()
            } else {
                answered.last_modified.clone()
            },
            interval_secs: answered.interval_secs.unwrap_or(state.interval_secs),
            seen: kept,
        },
    )
}

/// What one 200 said: the identities it listed NEWEST FIRST, as the API
/// orders them, the cursor to send next time, and the interval it asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answer {
    pub identities: Vec<String>,
    pub last_modified: String,
    pub interval_secs: Option<u64>,
}

#[cfg(test)]
mod tests;
