use pns_domain::stale::Blocked;

/// The session row's own record of one wait: the state the return card's open
/// waits and the escalation are both derived from.
///
/// SEPARATE FROM THE BLOCKED MARKER, and the difference is the whole reason
/// this port exists. That marker is written only where a lamp map and a
/// transport are both live (`BlockedMarker`), so an escalation built on it
/// would be silently dead on a machine with no `[lights]` table. This row is
/// written whatever the lamps are doing.
///
/// WHICH EVENTS START AND END A WAIT IS NOT THIS PORT'S QUESTION. It is
/// `lights::phase::blocked_marker_action` over `pulse::LAMP_BLOCKED`, the one
/// list the lamps already carry, applied by `track_wait`.
pub trait SessionWaits {
    /// Start this session's wait, clearing any previous escalation: a new
    /// wait is a new thing nobody has answered. One that does not `escalates`
    /// is recorded as already claimed, so nothing ever pages about it.
    fn begin(&self, session_id: &str, now: u64, escalates: bool) -> Result<(), String>;
    /// End it, so nothing escalates about a wait that is over.
    fn end(&self, session_id: &str) -> Result<(), String>;
}

/// The rows a fire acts on, and the claim that keeps one page per block.
///
/// THE CLAIM IS A WRITE AND NOT A LOCK. Stamping the row under
/// `escalated_at IS NULL` is a compare-and-swap the database arbitrates, so
/// two fires woken in one tick produce one page between them; the port
/// answers whether THIS caller was the one that stamped it.
pub trait StaleWaits {
    /// Every session waiting since `threshold` or earlier with no escalation
    /// stamped, oldest first.
    fn waiting_since(&self, threshold: u64) -> Vec<Blocked>;
    /// Stamp this session's escalation, answering whether this caller did.
    fn claim(&self, session_id: &str, now: u64) -> bool;
}
