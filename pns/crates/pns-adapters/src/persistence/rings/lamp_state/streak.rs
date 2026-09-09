use super::*;
/// The working streak after this tick's reading, published or removed.
pub fn advance_streak(
    state: &Path,
    working: bool,
    now: u64,
) -> Option<pns_domain::lights::streak::Streak> {
    let marker = state.join(LIGHTS_STREAK);
    let held = std::fs::read_to_string(&marker)
        .ok()
        .and_then(|line| crate::lights_codec::parse_streak(&line));
    let next = pns_domain::lights::streak::next_streak(held, working, now, WORKING_GRACE_SECS);
    // FAIL-QUIET, in `record_missed`'s style: a streak that did not land costs
    // one lamp its breathing, and this process has no reader for a complaint.
    match &next {
        Some(streak) => {
            let _ = publish_state_line(&marker, &crate::lights_codec::render_streak(streak));
        }
        None => {
            let _ = std::fs::remove_file(&marker);
        }
    }
    next
}
/// How long a run of work survives readings that say nothing is working.
///
/// THE GAP BETWEEN A LOOP'S TURNS IS WHAT THIS COVERS, and it is why the
/// streak is not simply "is something working right now": an agent reads idle
/// for the seconds between one turn and the next, and a streak that reset
/// there could never reach a threshold measured in minutes.
const WORKING_GRACE_SECS: u64 = 120;

/// Where the streak lives.
const LIGHTS_STREAK: &str = "lights-streak";
