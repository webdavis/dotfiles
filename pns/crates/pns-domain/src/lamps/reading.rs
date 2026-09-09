use super::Muting;

/// What one lamp is judged against: the minute it is being asked about, and the
/// names the operator's own mute is covering.
pub struct Reading<'reading> {
    pub minutes_now: Option<u16>,
    /// AN EMPTY `Places` IS THE ORDINARY CASE, and a machine that has never run
    /// `pns lights quiet` reads an absent file as exactly that.
    pub muted: &'reading Muting,
}
