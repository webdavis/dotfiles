use pns_domain::RawPresence;

pub enum PollClaim<G> {
    Held(G),
    Busy,
    Unavailable,
}

pub trait PresencePoll {
    type Guard;
    fn claim(&self) -> PollClaim<Self::Guard>;
    fn read(&self, watched: &[String], now: u64) -> Option<RawPresence>;
    fn publish(&self, reading: &RawPresence) -> bool;
}

/// What one poll did, and what the operator who typed it is told.
#[derive(Debug, PartialEq, Eq)]
pub enum Polled {
    /// A reading reached the state file.
    Published,
    /// Nothing was published and nothing this can name is wrong: a bridge that
    /// did not answer, a reading the format cannot carry, a state directory it
    /// cannot write. Every one of them ages the last reading out to Unknown,
    /// which is what they all mean.
    Nothing,
    /// Another poller is inside the bridge read.
    Busy,
}

/// The poll's whole effect: claim the poll, read the bridge, publish the reading.
///
/// A BRIDGE THAT DID NOT ANSWER PUBLISHES NOTHING. That single choice is what
/// makes a dead bridge, a wrong key and a wedged LAN all read as Unknown a few
/// seconds later, instead of pinning the operator wherever the last poll left
/// them. AND NEITHER DOES A READING THE FORMAT CANNOT CARRY: `render` hands
/// its own line back to its own parser and answers nothing at all when what
/// comes back is not what went in, so the write is never a line the reader
/// would read as a different reading.
///
/// The port exposes the poll claim, bridge reading and publication as separate
/// operations. The adapter owns network and filesystem access; this use case
/// keeps the claim held across both operations.
///
/// ONE POLLER AT A TIME ACROSS THE WHOLE MACHINE, held from before the first
/// read to after the rename. The running-child check in the daemon is
/// PROCESS-LOCAL, so a second daemon, a replacement daemon that orphaned the
/// first one's child, or a hand-typed `pns presence poll` can each be inside
/// this at once. Without the lock the LAST rename wins rather than the newest
/// reading: a poller stalled between its two reads publishes an older room
/// over a newer one and `classify` accepts it as current. Standing down costs
/// one interval of a reading somebody else is already taking.
///
/// THE LOCK IS THE KERNEL'S, not a name on disk, so the poll a killed poller
/// was inside is claimable the moment it dies: see `presence_lock`.
pub fn poll_presence<P: PresencePoll>(
    poll: &P,
    rooms: &[String],
    exclude: &[String],
    now: u64,
) -> Polled {
    let _lock = match poll.claim() {
        PollClaim::Held(guard) => guard,
        PollClaim::Busy => return Polled::Busy,
        PollClaim::Unavailable => return Polled::Nothing,
    };
    // THE EXCLUSION IS APPLIED BEFORE THE NEWEST EDGE IS CHOSEN, not left to
    // the reader. `classify` refuses an excluded room outright, so publishing
    // one would throw away the newest edge in a room that DOES count and
    // answer Unknown. The key is documented for "a room you pass through",
    // which is the room that reports MOST often, so the reading it swallowed
    // would be the common case rather than the corner.
    let watched: Vec<String> = rooms
        .iter()
        .filter(|room| !exclude.contains(room))
        .cloned()
        .collect();
    let Some(reading) = poll.read(&watched, now) else {
        return Polled::Nothing;
    };
    if poll.publish(&reading) {
        Polled::Published
    } else {
        Polled::Nothing
    }
}
