/// One narrowing decision, as the fields the line carries.
///
/// THE STRUCT IS THE SCHEMA, and it is the READ side too: `entry` writes these
/// fields and `last` reads them back, so the pair cannot drift.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PresenceDecision {
    /// The clock the decision's readings were taken against, absent when
    /// there was no clock.
    pub at: Option<u64>,
    /// What the room sensor said, in one phrase.
    pub presence: String,
    /// Seconds since the desk keyboard was touched, absent when unreadable.
    pub desk_idle_secs: Option<u64>,
    /// What the router said about the phone, as its variant name.
    pub home: String,
    /// The room the lamps were narrowed to. `None` is the whole routing left
    /// standing, and `reason` says why.
    pub room: Option<String>,
    /// Why the routing was left whole, empty when a room was named.
    pub reason: String,
}
