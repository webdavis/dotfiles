use crate::*;

/// What one tick found: the states the house is holding, and whether anything
/// is still in flight that could become one before the next tick.
///
/// TWO ANSWERS OFF ONE READING, because the tick's own lease is a function of
/// both. A lamp that is ON has to be re-armed; a run of work that has NOT yet
/// reached its threshold has to still be watched when it does, and taking that
/// as a second reading would be a second sweep of the same directories.
pub(crate) struct Standing {
    pub(crate) house: pns::lights::House,
    /// A run of work or a lease that is live and has not lit a lamp YET.
    pub(crate) in_flight: bool,
}

/// The states the house is in, taken off the machine.
///
/// THE STREAK IS ADVANCED HERE, which is the one reading that WRITES: a run of
/// work is a duration, and a duration needs somewhere to have started.
pub(crate) fn lights_house(state: &Path, lights: &pns::config::Lights, now: u64) -> Standing {
    // THE SAME CALL THE VISIBILITY MODEL MAKES, bounded the same way, and read
    // for a different field. A herdr that is missing, wedged or answering
    // something this cannot parse yields no working workspace, which is the
    // fail-toward-dark direction.
    let statuses =
        pns::system::CommandRunner::run(&SystemCommandRunner, "herdr", &["workspace", "list"])
            .map(|answer| pns::lights::workspace_agent_statuses(&answer))
            .unwrap_or_default();
    // THE SHELL'S OWN MARKERS, which each interactive shell writes while a
    // plain command runs in it. Nothing in this crate writes them.
    let shell_since = sweep_shell_markers(state);
    // BOTH SOURCES ARE WORK IN FLIGHT (operator ruling), which is the question
    // the UNREAD lamp asks: news that arrives while anything is still running is
    // not news anybody has missed yet.
    let working = pns::lights::any_working(&statuses, shell_since);
    // AND THE STREAK IS THE AGENTS' ALONE, because it exists to supply a start
    // that herdr does not give: a status word carries no clock. The shell
    // publishes the second its command began, so pooling the two had a fresh
    // command inherit an agent's finished run and a long build restart its own.
    let agents_working = pns::lights::any_working(&statuses, None);
    let streak = advance_streak(state, agents_working, now);
    let leases = sweep_leases(state, now, lights.looping.lease_timeout_secs);
    Standing {
        // WORK THAT HAS NOT REACHED ITS THRESHOLD IS STILL IN FLIGHT, and this
        // is the reading that keeps the tick alive long enough to see it get
        // there: the automatic trigger's default is five minutes and the
        // operator's is six, both of them PAST the ordinary lease an event
        // leaves behind.
        in_flight: streak.is_some() || shell_since.is_some() || !leases.is_empty(),
        house: pns::lights::House {
            blocked: blocked_lamp(state, lights, now),
            looping: pns::lights::loop_running(&pns::lights::Loop {
                streak: streak.as_ref(),
                agents_working,
                shell_since,
                leases: &leases,
                now,
                threshold_secs: lights.looping.threshold_secs,
                lease_timeout_secs: lights.looping.lease_timeout_secs,
            }),
            unread: pns::lights::unread_arming(
                &read_news(state),
                last_interaction(),
                working,
                now,
                lights.unread.after_secs,
            ),
        },
    }
}

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
/// HOISTING `let now = now_secs()?;` ABOVE THE SAMPLES WOULD BREAK THIS
/// SILENTLY: no test can catch a clock read moving a few hundred milliseconds
/// earlier, so the order below is load-bearing and not provable by a diff
/// alone. Do not reorder it.
///
/// THE OVERRIDES ARE NOT CONSULTED HERE. `PNS_IDLE_SECS` and
/// `PNS_PHONE_INPUT_AGE` steer the delivery decision in `engine::decide`, not
/// this reading: the unread lamp always sees the machine's own probes.
fn last_interaction() -> Option<u64> {
    let probes = system_probes();
    pns::lights::last_interaction(
        pns::probes::IdleProbe::idle_secs(&probes),
        pns::probes::PhoneInputProbe::phone_input_atime_secs(&probes),
        pns::probes::PhoneMarkerProbe::marker_mtime_secs(&probes),
        now_secs()?,
    )
}
/// The working streak after this tick's reading, published or removed.
fn advance_streak(state: &Path, working: bool, now: u64) -> Option<pns::lights::Streak> {
    let marker = state.join(LIGHTS_STREAK);
    let held = std::fs::read_to_string(&marker)
        .ok()
        .and_then(|line| pns::lights::parse_streak(&line));
    let next = pns::lights::next_streak(held, working, now, WORKING_GRACE_SECS);
    // FAIL-QUIET, in `record_missed`'s style: a streak that did not land costs
    // one lamp its breathing, and this process has no reader for a complaint.
    match &next {
        Some(streak) => {
            let _ = publish_state_line(&marker, &pns::lights::render_streak(streak));
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
