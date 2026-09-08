/// When the operator last touched this machine, by ANY road: the desk, the
/// phone's input, or the deliberate phone marker. The rule is
/// `lights::last_interaction`'s; this reads the three probes and hands them in.
///
/// THE CLOCK IS READ LAST, BY DESIGN, after the three samples rather than
/// before them. The two phone edges are file times and need no clock; the
/// desk edge is the one `lights::last_interaction` computes, as
/// `t_now - idle(t_sample)`. Reading `t_now` first would put it BEFORE the
/// sample, so the edge would land earlier than the true touch and news the
/// operator had already seen could arm the lamp. Reading it last puts the
/// residual the other way: `t_now` is later than the sample by at most the
/// four bounded spawns above this line (one `ioreg` for idle, then the phone
/// probe's `pgrep`, `pgrep -P` and `ps`), each capped at `PROBE_DEADLINE`
/// (5 seconds in `system.rs`), so the bound is four five-second receive
/// budgets, plus spawn and cleanup overhead on top, sub-second in the common
/// case. The desk touch reads that much YOUNGER
/// than it was, never older. The direction is DARK: news that landed inside
/// that residual reads as seen and the lamp stays off, and no edge can arm
/// it early.
///
/// THE OVERRIDES ARE NOT CONSULTED HERE. `PNS_IDLE_SECS` and
/// `PNS_PHONE_INPUT_AGE` steer the delivery decision in `engine::decide`, not
/// this reading: the unread lamp always sees the machine's own probes.
pub fn last_lamp_interaction<
    P: crate::IdleProbe + crate::PhoneInputProbe + crate::PhoneMarkerProbe + crate::Clock,
>(
    probes: &P,
) -> Option<u64> {
    pns_domain::lights::unread::last_interaction(
        probes.idle_secs(),
        probes.phone_input_atime_secs(),
        probes.marker_mtime_secs(),
        probes.now_secs()?,
    )
}

#[cfg(test)]
mod tests;
