//! The lamps' three STATES, and the readings each one is derived from.
//!
//! PURE AND TOTAL, like every other decision module: no network, no files, no
//! clock and no environment. The tick reads the machine at its edge and hands
//! the values in, which is what lets a state be swept a second at a time in a
//! unit test.
//!
//! THE TICK RE-DERIVES EVERY STATE FROM SCRATCH and holds nothing in memory
//! between runs, for the reason the daemon states about itself: a divergence
//! between what a process believes and what the disk says is the class this
//! crate keeps paying for.

/// The one word herdr's agent-status enum uses for a loop that is running.
///
/// The enum is `idle`, `working`, `blocked`, `unknown`, read off the binary's
/// own serde variant table on 0.8.2. Only `working` lights a lamp: `blocked`
/// is the operator's turn, which is the BLUE lamp's business, and the other
/// two are nothing happening.
pub const WORKING: &str = "working";

/// One `agent_status` per workspace, in the order herdr listed them, with a
/// workspace that carries no such field answering the EMPTY string.
///
/// A MISSING FIELD IS NOT A WORKING LOOP, which is the fail-toward-dark
/// direction this whole design takes, and it is not hypothetical: the suite's
/// own shipped herdr stub answers a `workspace list` with no `agent_status` in
/// it, and a herdr that stops carrying the field must leave a lamp dark rather
/// than breathing forever.
///
/// A SECOND READER OF ONE ANSWER, not a change to `parse_focused_tab`: that
/// function reads `focused` and `active_tab_id` for the visibility model and
/// has no business knowing what a lamp does.
pub fn workspace_agent_statuses(workspace_list_json: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(workspace_list_json)
        .ok()
        .as_ref()
        .and_then(|body| body.pointer("/result/workspaces"))
        .and_then(serde_json::Value::as_array)
        .map(|workspaces| {
            workspaces
                .iter()
                .map(|workspace| {
                    workspace
                        .get("agent_status")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Whether ANYTHING is working: any workspace herdr calls `working`, or a
/// plain long command the shell is holding a marker open for.
///
/// AN OR, WHICH IS THE OPERATOR'S OWN AGGREGATION RULE applied literally:
/// the loop lamp breathes if at least one thing is working, and goes dark only
/// when none of them is. An AND would leave the lamp dark for the whole of a
/// single agent's run, which is precisely the run it exists to show.
///
/// THE SHELL MARKER IS THE SECOND PRODUCER and it is a plain epoch: the shell
/// records a long command's start (`dot_bashrc.tmpl` already writes epochs
/// this way) and removes it when the command ends. It is not read for its
/// value here, only for its presence; the streak below is what turns presence
/// into a duration, and it must be the SAME streak the workspaces feed, or a
/// build and an agent loop would each start a clock of their own.
pub fn any_working(agent_statuses: &[String], shell_command_since: Option<u64>) -> bool {
    shell_command_since.is_some() || agent_statuses.iter().any(|status| status == WORKING)
}

/// How long something has been working: when the run started, and when it was
/// last CONFIRMED still going.
///
/// TWO NUMBERS AND NOT ONE, because a streak has to answer two questions that
/// move at different times. `since` is what the breathe threshold measures
/// against and must never move while a loop is alive; `last_seen` is what the
/// grace below measures against and moves on every tick that reads working.
/// One number could carry either meaning and not both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Streak {
    /// The second the current run of work began.
    pub since: u64,
    /// The second something was last read as working.
    pub last_seen: u64,
}

/// One streak as one line: the two seconds, space separated, in
/// `render_heartbeat`'s shape.
pub fn render_streak(streak: &Streak) -> String {
    format!("{} {}", streak.since, streak.last_seen)
}

/// That line read back, or None for anything this will not vouch for.
///
/// REFUSED, NEVER GUESSED AT, in `parse_heartbeat`'s style. A file some other
/// hand rewrote is not a streak, and reading a garbled half as zero would
/// report a loop as having worked since 1970, which passes every threshold
/// there is and leaves a lamp breathing over nothing.
pub fn parse_streak(line: &str) -> Option<Streak> {
    let (since, last_seen) = line.trim_end_matches('\n').split_once(' ')?;
    Some(Streak {
        since: crate::parse_count(since)?,
        last_seen: crate::parse_count(last_seen)?,
    })
}

/// The streak after this tick's reading: started, carried, or cleared.
///
/// THE GRACE IS THE WHOLE POINT OF THE FUNCTION. An agent loop reads
/// not-working in the seconds between its turns, and a streak that reset on
/// the first such reading could never reach a threshold measured in minutes,
/// so the lamp would never breathe at all. The grace is closed at its far
/// edge: exactly `grace_secs` since the last confirmed working second still
/// carries the streak, and one second past it clears.
///
/// A CLEARED STREAK IS GONE RATHER THAN REMEMBERED. The next working reading
/// starts a fresh one at that second, which is what makes "how long has this
/// run been going" answerable at all.
pub fn next_streak(
    held: Option<Streak>,
    working: bool,
    now: u64,
    grace_secs: u64,
) -> Option<Streak> {
    if working {
        return Some(Streak {
            since: held.map_or(now, |streak| streak.since),
            last_seen: now,
        });
    }
    held.filter(|streak| now.saturating_sub(streak.last_seen) <= grace_secs)
}

/// Everything the breathing condition is a function of.
///
/// A NAMED STRUCT rather than seven positional arguments, four of which are
/// `u64`-shaped: a transposition between the two thresholds, or between `now`
/// and either of them, is a lamp judged against the wrong clock and nothing
/// would catch it.
pub struct Breath<'reading> {
    /// The sources the operator left switched on. A source not in here
    /// contributes NOTHING, whatever the readings below say.
    pub enabled: &'reading [crate::config::BreatheSource],
    /// herdr says at least one workspace is working, right now.
    pub agent_working: bool,
    /// The AGENT run in progress, which only `agent-loops` reads. It tracks
    /// herdr alone: a streak fed by the shell marker as well would let a
    /// long build satisfy `agent-loops` on a machine where the agent sources
    /// were the only ones switched on.
    pub streak: Option<&'reading Streak>,
    /// When the shell's tracked command started, if one is running.
    pub shell_since: Option<u64>,
    pub now: u64,
    /// How long `agent-loops` waits before calling a run a loop.
    pub breathe_after_secs: u64,
    /// How long a command runs before `long-commands` counts it. It is the
    /// NOTIFIER'S OWN tier, read through the same function the notifier reads,
    /// so the lamp and the card share the CODE that decides what "long" means.
    /// They do not share the ENVIRONMENT it reads: that function takes an
    /// override out of the environment, the notifier's comes from the
    /// interactive shell and the tick's from the daemon's plist, so an override
    /// set in only one of the two is a disagreement of exactly that size.
    pub long_command_secs: u64,
}

/// The second a breathing run began, or None when nothing is breathing.
///
/// A UNION OVER THE ENABLED SOURCES, which is the operator's own rule: a
/// source that is switched off contributes nothing and the ones still on carry
/// on regardless. Naming both agent sources, or both command sources, is
/// harmless: the eager one simply wins.
///
/// THE START EPOCH RATHER THAN A BOOLEAN, because the catch-up rule needs to
/// know WHEN a state began in order to ask whether it began inside a quiet
/// window. Every state here answers the same shape for the same reason, and
/// the FRESHEST source wins for `glow_since`'s reason: a run that began after
/// a quiet window ended is not a leftover of that window.
///
/// A `now` BEHIND A START HAS NO ELAPSED TIME IN IT. A clock that stepped
/// backwards would otherwise wrap a subtraction into a huge number that passes
/// every threshold there is.
pub fn breathing_since(breath: &Breath<'_>) -> Option<u64> {
    let on = |source| breath.enabled.contains(&source);
    let ran_for = |since: u64, threshold: u64| {
        breath
            .now
            .checked_sub(since)
            .is_some_and(|elapsed| elapsed >= threshold)
    };
    // THE STREAK IS THE ONLY START AN AGENT HAS. The tick advances it before
    // asking this, so a working agent always has one; `now` is the honest
    // answer if it somehow does not, rather than declining to breathe over a
    // missing file.
    let agent_since = breath.streak.map(|streak| streak.since);
    [
        (
            on(crate::config::BreatheSource::AgentWork) && breath.agent_working,
            agent_since.or(Some(breath.now)),
        ),
        (
            // BOTH HALVES, which is the rule as written: something is working
            // AND the run is at least `breathe_after_secs` old. The streak
            // deliberately outlives the work by the grace that covers the gap
            // between a loop's turns, so the threshold alone would keep the
            // lamp claiming work in progress after the agent went idle.
            on(crate::config::BreatheSource::AgentLoops)
                && breath.agent_working
                && agent_since.is_some_and(|since| ran_for(since, breath.breathe_after_secs)),
            agent_since,
        ),
        (
            on(crate::config::BreatheSource::Commands),
            breath.shell_since,
        ),
        (
            on(crate::config::BreatheSource::LongCommands)
                && breath
                    .shell_since
                    .is_some_and(|since| ran_for(since, breath.long_command_secs)),
            breath.shell_since,
        ),
    ]
    .into_iter()
    .filter_map(|(fires, since)| fires.then_some(since).flatten())
    .max()
}

/// The second the newest UNSEEN journal entry landed, when nothing is working,
/// or None for a lamp that has nothing to glow about.
///
/// THE CONDITION IS "NOTHING WORKING AND A JOURNAL ENTRY NEWER THAN THE RETURN
/// EDGE", and it is one function rather than two so the two halves cannot come
/// out disagreeing about one tick. Something working is the BREATHING lamp's
/// business, and a lamp cannot be both.
///
/// THE EDGE IS `LAST_PRESENT`, which the return moment already advances on
/// every present event. That is what makes this state clear itself with no
/// timeout and no new clear path: a journal whose newest entry predates the
/// edge stops satisfying the condition, and the next tick stops arming the
/// lamp.
///
/// NO EDGE AT ALL IS NO GLOW, never an edge at epoch zero. `read_epoch`'s own
/// rule one level up is that an unparseable marker is no edge, and a machine
/// that cannot prove the operator ever came back cannot prove this news is
/// unseen either. Dark is the direction every unreadable reading on this path
/// takes.
///
/// AN ENTRY AT THE EDGE IS NOT NEWER THAN IT, which is the same direction on a
/// tie.
///
/// THE NEWEST ENTRY AND NOT THE OLDEST, because the only reader of this epoch
/// is the catch-up rule, and the question it asks is whether the state is a
/// leftover of a quiet window that has since ended. News that arrived after
/// the window ended is not, whatever is queued behind it.
///
/// AN ENTRY WITH NO `at` CANNOT GLOW. Its writer had no readable clock, so it
/// sits in no window at all and there is nothing to compare against an edge.
pub fn glow_since(
    entries: &[crate::missed_notifications::Entry],
    return_edge: Option<u64>,
    working: bool,
) -> Option<u64> {
    if working {
        return None;
    }
    let edge = return_edge?;
    entries
        .iter()
        .filter_map(|entry| entry.at)
        .filter(|at| *at > edge)
        .max()
}

/// The second the freshest LIVE wait began, or None when nothing is waiting on
/// the operator.
///
/// A MARKER PAST THE BOUND IS IGNORED, which is the only thing standing
/// between an abandoned session and a lamp held blue forever. The marker
/// clears at the next event from its own session, and a session that never
/// sends one again would otherwise hold the lamp until the operator went
/// looking for a file. BOTH EDGES CLOSED: exactly at the bound is still live.
///
/// THE FRESHEST AND NOT THE OLDEST, for `glow_since`'s reason: the only reader
/// of this epoch is the catch-up rule, and a wait that began after a quiet
/// window ended is not a leftover of that window.
pub fn needs_you_at(marker_epochs: &[u64], now: u64, max_age_secs: u64) -> Option<u64> {
    marker_epochs
        .iter()
        .copied()
        .filter(|at| needs_is_live(*at, now, max_age_secs))
        .max()
}

/// Whether one marker still counts as a wait.
///
/// ITS OWN FUNCTION because two callers ask it and they must agree: the
/// aggregate above, and the sweep that DELETES the ones past the bound. Two
/// spellings of "expired" would be a marker the aggregate ignored and the
/// sweep kept, accumulating forever, or one the sweep removed while the
/// aggregate was still lighting a lamp for it.
///
/// A MARKER FROM THE FUTURE IS LIVE. A clock that stepped backwards is not a
/// wait that ended, and the saturating subtraction reads it as zero seconds
/// old rather than as an enormous age that would delete it.
pub fn needs_is_live(at: u64, now: u64, max_age_secs: u64) -> bool {
    now.saturating_sub(at) <= max_age_secs
}

/// What this tick read, one field per state, each carrying the second the
/// freshest thing behind it happened.
///
/// A NAMED STRUCT RATHER THAN THREE POSITIONAL OPTIONS, because three values
/// of one type in one signature is a swap nothing would catch and all three
/// are epochs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readings {
    pub needs_you_at: Option<u64>,
    pub breathing_since: Option<u64>,
    pub glow_since: Option<u64>,
}

/// One state, and when the thing behind it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub behaviour: crate::config::Behaviour,
    /// The second the freshest signal behind this state landed. The CATCH-UP
    /// rule is the only reader: it asks whether the state began inside a quiet
    /// window that has since ended.
    pub since: u64,
}

/// The one state the house is in, or None for a dark one.
///
/// NEEDS-YOU ON TOP, which is the operator's own ruling and is delivered here
/// rather than by a per-fixture priority file: one state is derived from
/// scratch every tick, so there is no stored priority for two processes to
/// disagree about and no read-before-write on the event path.
///
/// ONE STATE FOR THE WHOLE HOUSE, and each family shows it only if that family
/// produces it (`hue::family_produces`). So a waiting agent turns the local
/// lamps blue and leaves the loop lamp dark, which is honest: a loop with a
/// blocked agent in it is not working, and the news it would glow about is
/// exactly the wait the blue lamp is already reporting.
pub fn house_state(readings: &Readings) -> Option<State> {
    let (behaviour, since) = match readings {
        Readings {
            needs_you_at: Some(at),
            ..
        } => (crate::config::Behaviour::NeedsYou, *at),
        Readings {
            breathing_since: Some(since),
            ..
        } => (crate::config::Behaviour::Breathing, *since),
        Readings {
            glow_since: Some(since),
            ..
        } => (crate::config::Behaviour::Glow, *since),
        _ => return None,
    };
    Some(State { behaviour, since })
}

/// What one harness event does to its session's needs marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// An agent is waiting on the operator from here until something says
    /// otherwise.
    Start,
    /// A later event from that session, which is what says otherwise.
    End,
}

/// Which of the two an event's STATE is.
///
/// A CLOSED SET OF STARTERS AND EVERYTHING ELSE ENDS, rather than a closed set
/// on both sides. A state this does not recognise is still a later event from
/// that session, and the fail direction that matters is the one that lets a
/// lamp go dark: an unknown word treated as a start would hold blue on a
/// session nobody is waiting for.
///
/// IT READS `pulse::LAMP_NEEDS_YOU`, the four-word list the lamps already
/// carry, and NOT `missed_notifications::NEEDS_YOU`, which correctly includes
/// `failed`. A dead turn is red, not blue, and it is not a wait anybody can
/// end.
pub fn needs_marker_action(event_state: &str) -> Action {
    if crate::pulse::LAMP_NEEDS_YOU.contains(&event_state) {
        Action::Start
    } else {
        Action::End
    }
}

/// Where the needs markers live: one file per waiting session.
pub fn needs_dir(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("lights-needs")
}

/// One session's marker path, or None for a session id that cannot become a
/// filename.
///
/// THE SESSION ID AND NOT THE PANE, and the difference is a path escape.
/// `pane_is_safe` permits `..` because a pane id becomes a shell WORD, never a
/// filename; `session_id_is_safe` forbids it and already backs a filename in
/// this same directory (`session-<id>.start`). Reusing it writes no new
/// predicate and opens no new door.
pub fn needs_marker(state_dir: &std::path::Path, session_id: &str) -> Option<std::path::PathBuf> {
    crate::safety::session_id_is_safe(session_id).then(|| needs_dir(state_dir).join(session_id))
}

/// What a tick does with the complaints it has this second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Say {
    /// Print nothing and change nothing: either there is nothing wrong, or the
    /// same thing is still wrong and it has already been said.
    Nothing,
    /// Print the complaints and remember this line as what was said.
    Aloud(String),
    /// The complaint cleared. Print nothing, and forget, so that the same
    /// complaint coming back is news again.
    Forget,
}

/// Whether this tick's complaints are worth saying, given what the last one
/// said.
///
/// ONCE, NOT EVERY TICK, and the memory is on disk because there is no
/// process to hold it in: the daemon re-executes this binary for every tick,
/// so "once per daemon lifetime" cannot be a variable. This is
/// `remember_staleness`'s idiom one directory over, and its reason is the
/// same: the thing worth saying is a CHANGE.
///
/// ONE LINE, JOINED, because the memory is one state file and every state file
/// in this crate is published as a single line. A complaint carrying a newline
/// is flattened into it, so the memory can never be read back as two.
pub fn say(lines: &[String], remembered: &str) -> Say {
    let said = lines.join(" | ").replace('\n', " ");
    if said == remembered {
        return Say::Nothing;
    }
    if said.is_empty() {
        return Say::Forget;
    }
    Say::Aloud(said)
}

#[cfg(test)]
mod tests {
    use super::{
        Action, Breath, Readings, Say, State, Streak, WORKING, any_working, breathing_since,
        glow_since, house_state, needs_marker, needs_marker_action, needs_you_at, next_streak,
        parse_streak, render_streak, say, workspace_agent_statuses,
    };
    use crate::config::{Behaviour, BreatheSource};

    /// herdr 0.8.2's own answer, captured live on 2026-09-01: three workspaces
    /// carrying three of the four status words.
    const HERDR_WORKSPACES: &str = r#"{"result":{"workspaces":[
      {"active_tab_id":"t1","agent_status":"working","focused":true,"workspace_id":"w1"},
      {"active_tab_id":"t4","agent_status":"idle","focused":false,"workspace_id":"w2"},
      {"active_tab_id":"t7","agent_status":"unknown","focused":false,"workspace_id":"w3"}
    ]}}"#;

    /// The answer the suite's SHIPPED stub gives, which carries no
    /// `agent_status` at all.
    const NO_STATUS_FIELD: &str =
        r#"{"result":{"workspaces":[{"active_tab_id":"t1","focused":true,"workspace_id":"w1"}]}}"#;

    #[test]
    fn every_workspaces_agent_status_is_read_and_a_missing_one_is_not_working() {
        assert_eq!(
            workspace_agent_statuses(HERDR_WORKSPACES),
            vec![WORKING, "idle", "unknown"],
            "herdr's real answer, in its own order"
        );
        assert_eq!(
            workspace_agent_statuses(NO_STATUS_FIELD),
            vec![String::new()],
            "a workspace with no agent_status is a workspace this will not call working"
        );
        assert!(
            workspace_agent_statuses("not json").is_empty(),
            "an unreadable answer names no working workspace"
        );
    }

    #[test]
    fn one_working_workspace_is_enough_and_none_of_them_working_is_not() {
        let statuses =
            |words: &[&str]| -> Vec<String> { words.iter().map(|word| word.to_string()).collect() };
        assert!(
            any_working(&statuses(&["idle", WORKING, "unknown"]), None),
            "the operator's rule, applied literally: breathing if AT LEAST ONE thing is working"
        );
        assert!(
            !any_working(&statuses(&["idle", "unknown", "blocked"]), None),
            "blocked is the operator's turn, not a loop running, so nothing here is working"
        );
        assert!(
            !any_working(&[], None),
            "no workspace at all is nothing working"
        );
        assert!(
            any_working(&statuses(&["idle"]), Some(1_000)),
            "and a plain long shell command is a working loop with no workspace behind it"
        );
    }

    #[test]
    fn the_streak_starts_survives_a_gap_between_turns_and_clears_behind_the_grace() {
        const GRACE: u64 = 120;
        let held = Streak {
            since: 1_000,
            last_seen: 1_050,
        };
        assert_eq!(
            next_streak(None, true, 1_000, GRACE),
            Some(Streak {
                since: 1_000,
                last_seen: 1_000
            }),
            "working with no streak starts one at now"
        );
        assert_eq!(
            next_streak(Some(held.clone()), true, 1_200, GRACE),
            Some(Streak {
                since: 1_000,
                last_seen: 1_200
            }),
            "working with a streak keeps its START and only moves what it last saw"
        );
        // THE CASE THAT MATTERS. The seconds between a loop's turns read as
        // not-working, and a streak that reset there could never reach a
        // threshold measured in minutes.
        assert_eq!(
            next_streak(Some(held.clone()), false, 1_050 + GRACE, GRACE),
            Some(held.clone()),
            "not working INSIDE the grace leaves the streak exactly as it was"
        );
        assert_eq!(
            next_streak(Some(held.clone()), false, 1_050 + GRACE + 1, GRACE),
            None,
            "and one second past the grace clears it"
        );
        assert_eq!(
            next_streak(None, false, 1_000, GRACE),
            None,
            "nothing working and no streak stays nothing"
        );
    }

    #[test]
    fn a_streak_survives_as_one_line_and_anything_else_is_no_streak() {
        let held = Streak {
            since: 1_000,
            last_seen: 1_200,
        };
        assert_eq!(render_streak(&held), "1000 1200");
        assert_eq!(parse_streak("1000 1200"), Some(held));
        // REFUSED, NEVER GUESSED AT, in `parse_heartbeat`'s style: a file some
        // other hand rewrote is not a streak, and reading half of one as zero
        // would report a loop as having worked since 1970.
        for garbled in [
            "",
            "1000",
            "1000 1200 1400",
            "x 1200",
            "1000 x",
            " 1000 1200",
        ] {
            assert_eq!(parse_streak(garbled), None, "{garbled:?} is not a streak");
        }
    }

    const NOW: u64 = 10_000;
    const AFTER: u64 = 900;
    const LONG: u64 = 300;

    /// One reading, with everything not under test set to nothing happening.
    fn breath<'reading>(
        enabled: &'reading [BreatheSource],
        agent_working: bool,
        streak: Option<&'reading Streak>,
        shell_since: Option<u64>,
    ) -> Breath<'reading> {
        Breath {
            enabled,
            agent_working,
            streak,
            shell_since,
            now: NOW,
            breathe_after_secs: AFTER,
            long_command_secs: LONG,
        }
    }

    /// A run of agent work that started `ago` seconds before now.
    fn streak_from(ago: u64) -> Streak {
        Streak {
            since: NOW - ago,
            last_seen: NOW,
        }
    }

    #[test]
    fn each_breathe_on_source_gates_its_own_detector_and_watches_nothing_else() {
        let long_run = streak_from(AFTER);
        let short_run = streak_from(0);
        // Per source: the reading it watches, and a reading it must ignore.
        let cases: [(BreatheSource, Breath<'_>, Breath<'_>); 4] = [
            (
                // Any working agent at all, however briefly.
                BreatheSource::AgentWork,
                breath(&[BreatheSource::AgentWork], true, Some(&short_run), None),
                breath(&[BreatheSource::AgentWork], false, None, Some(NOW)),
            ),
            (
                // The same agent, but only once it has kept at it.
                BreatheSource::AgentLoops,
                breath(&[BreatheSource::AgentLoops], true, Some(&long_run), None),
                breath(&[BreatheSource::AgentLoops], true, Some(&short_run), None),
            ),
            (
                // Any tracked shell command.
                BreatheSource::Commands,
                breath(&[BreatheSource::Commands], false, None, Some(NOW)),
                breath(&[BreatheSource::Commands], true, Some(&long_run), None),
            ),
            (
                // Only one that has reached the notifier's long tier.
                BreatheSource::LongCommands,
                breath(
                    &[BreatheSource::LongCommands],
                    false,
                    None,
                    Some(NOW - LONG),
                ),
                breath(
                    &[BreatheSource::LongCommands],
                    false,
                    None,
                    Some(NOW - LONG + 1),
                ),
            ),
        ];
        for (source, watched, ignored) in &cases {
            assert!(
                breathing_since(watched).is_some(),
                "{source:?} must breathe for the activity it names"
            );
            assert_eq!(
                breathing_since(ignored),
                None,
                "{source:?} must contribute NOTHING for an activity it does not name"
            );
        }
    }

    #[test]
    fn a_source_left_out_of_breathe_on_contributes_nothing_and_the_others_still_do() {
        let run = streak_from(0);
        // An agent working, a command running, and only the commands named.
        assert_eq!(
            breathing_since(&breath(
                &[BreatheSource::Commands],
                true,
                Some(&run),
                Some(NOW - 40)
            )),
            Some(NOW - 40),
            "the named source still breathes, and it answers ITS OWN start"
        );
        assert_eq!(
            breathing_since(&breath(&[], true, Some(&run), Some(NOW))),
            None,
            "and an empty breathe_on is breathing off, however much is working"
        );
    }

    #[test]
    fn agent_loops_keeps_the_streak_threshold_and_both_of_its_edges_are_closed() {
        let only_loops = [BreatheSource::AgentLoops];
        let under = streak_from(AFTER - 1);
        let at = streak_from(AFTER);
        let over = streak_from(AFTER + 1);
        assert_eq!(
            breathing_since(&breath(&only_loops, true, Some(&under), None)),
            None,
            "one second under the threshold is not a loop yet"
        );
        assert_eq!(
            breathing_since(&breath(&only_loops, true, Some(&at), None)),
            Some(NOW - AFTER),
            "exactly at it, it breathes, and it answers the second the run STARTED"
        );
        assert_eq!(
            breathing_since(&breath(&only_loops, true, Some(&over), None)),
            Some(NOW - AFTER - 1),
        );
        assert_eq!(
            breathing_since(&breath(&only_loops, false, None, None)),
            None,
            "no streak is nothing working, however late it is"
        );
        // AND THE CONJUNCTION, which is the brief's own wording: something is
        // working AND the run is at least `breathe_after_secs` old. The streak
        // deliberately OUTLIVES the work by the grace that covers the gap
        // between a loop's turns, so a reading of the streak alone keeps
        // claiming work in progress for minutes after the agent went idle, and
        // breathing outranks glow, so the lamp says the wrong thing rather than
        // saying nothing.
        assert_eq!(
            breathing_since(&breath(&only_loops, false, Some(&over), None)),
            None,
            "a streak still inside its grace is not an agent that is still working"
        );
        // A CLOCK BEHIND THE MARKER IS NOT A LONG RUN. A machine whose clock
        // stepped back would otherwise read a huge elapsed time through a
        // wrapping subtraction and breathe over nothing.
        let future = Streak {
            since: NOW + 500,
            last_seen: NOW + 500,
        };
        assert_eq!(
            breathing_since(&breath(&only_loops, true, Some(&future), None)),
            None,
            "a now before the streak began has no elapsed time in it"
        );
    }

    /// One journal line, in the shape `missed_notifications::entries` answers.
    fn missed(at: Option<u64>) -> crate::missed_notifications::Entry {
        crate::missed_notifications::Entry {
            at,
            agent: "claude".to_string(),
            state: "blocked".to_string(),
            project: "dotfiles".to_string(),
            branch: "main".to_string(),
            detail: "x".to_string(),
        }
    }

    #[test]
    fn glow_is_a_journal_entry_newer_than_the_return_edge_and_nothing_else() {
        const NOT_WORKING: bool = false;
        const WORKING_NOW: bool = true;
        assert_eq!(
            glow_since(&[missed(Some(1_100))], Some(1_000), NOT_WORKING),
            Some(1_100),
            "news the operator has not been back for, with nothing running: glow"
        );
        assert_eq!(
            glow_since(&[missed(Some(1_100))], Some(1_000), WORKING_NOW),
            None,
            "the same entry with something working is the BREATHING lamp's business"
        );
        assert_eq!(
            glow_since(&[missed(Some(900))], Some(1_000), NOT_WORKING),
            None,
            "an entry older than the edge was seen when the operator came back"
        );
        assert_eq!(
            glow_since(&[missed(Some(1_000))], Some(1_000), NOT_WORKING),
            None,
            "an entry AT the edge is not newer than it; dark is the direction on a tie"
        );
        assert_eq!(
            glow_since(&[missed(None)], Some(1_000), NOT_WORKING),
            None,
            "an entry whose writer had no clock sits in no window and cannot glow"
        );
        assert_eq!(
            glow_since(&[missed(Some(1_100))], None, NOT_WORKING),
            None,
            "no return edge at all is no proof the news is unseen, so the lamp stays dark"
        );
        assert_eq!(
            glow_since(&[], Some(1_000), NOT_WORKING),
            None,
            "an empty journal is nothing unseen"
        );
        // THE NEWEST UNSEEN ENTRY, which is the epoch the catch-up rule reads:
        // a glow carrying news from after the quiet window ended is not a
        // leftover of that window, whatever else is queued behind it.
        assert_eq!(
            glow_since(
                &[
                    missed(Some(1_100)),
                    missed(Some(1_400)),
                    missed(Some(1_200))
                ],
                Some(1_000),
                NOT_WORKING
            ),
            Some(1_400),
            "the newest unseen entry is what the state started at"
        );
    }

    #[test]
    fn needs_you_outranks_both_and_an_expired_marker_does_not_count() {
        const BOUND: u64 = 1_800;
        assert_eq!(
            needs_you_at(&[1_000, 1_400], 2_000, BOUND),
            Some(1_400),
            "a live marker is a wait, and the freshest one is when it last began"
        );
        assert_eq!(
            needs_you_at(&[1_000], 1_000 + BOUND, BOUND),
            Some(1_000),
            "exactly at the bound is still live: both edges closed"
        );
        assert_eq!(
            needs_you_at(&[1_000], 1_000 + BOUND + 1, BOUND),
            None,
            "one second past it, an abandoned session can no longer hold a lamp blue"
        );
        assert_eq!(
            needs_you_at(&[], 2_000, BOUND),
            None,
            "no marker is no wait"
        );

        // THE PRIORITY, which is the operator's own "needs-you on top".
        assert_eq!(
            house_state(&Readings {
                needs_you_at: Some(1_400),
                breathing_since: Some(1_000),
                glow_since: Some(1_200),
            }),
            Some(State {
                behaviour: Behaviour::NeedsYou,
                since: 1_400
            }),
            "a live wait beats working and beats unseen news"
        );
        assert_eq!(
            house_state(&Readings {
                needs_you_at: None,
                breathing_since: Some(1_000),
                glow_since: Some(1_200),
            }),
            Some(State {
                behaviour: Behaviour::Breathing,
                since: 1_000
            }),
            "with no wait, something working beats unseen news"
        );
        assert_eq!(
            house_state(&Readings {
                needs_you_at: None,
                breathing_since: None,
                glow_since: Some(1_200),
            }),
            Some(State {
                behaviour: Behaviour::Glow,
                since: 1_200
            }),
        );
        assert_eq!(
            house_state(&Readings {
                needs_you_at: None,
                breathing_since: None,
                glow_since: None,
            }),
            None,
            "and none of the three is a dark house"
        );
    }

    #[test]
    fn a_needs_you_event_starts_a_wait_and_every_other_event_ends_one() {
        for waiting in crate::pulse::LAMP_NEEDS_YOU {
            assert_eq!(
                needs_marker_action(waiting),
                Action::Start,
                "{waiting} is an agent waiting on the operator"
            );
        }
        for ended in ["done", "failed", "stale", "", "anything-else"] {
            assert_eq!(
                needs_marker_action(ended),
                Action::End,
                "{ended} is a later event from that session, so the wait is over"
            );
        }
    }

    #[test]
    fn a_session_id_that_cannot_be_a_filename_names_no_marker_at_all() {
        let state = std::path::Path::new("/state");
        assert_eq!(
            needs_marker(state, "sess-123"),
            Some(state.join("lights-needs").join("sess-123")),
            "an ordinary id names a file inside the needs directory"
        );
        // THE PATH-ESCAPE GUARD, through the predicate that already backs
        // `session-<id>.start` in this same directory rather than a second one.
        for refused in ["..", "../etc/passwd", "a/b", "", "a:b", "a b"] {
            assert_eq!(
                needs_marker(state, refused),
                None,
                "{refused:?} must name no marker"
            );
        }
    }

    #[test]
    fn a_tick_says_a_complaint_once_and_says_it_again_only_when_it_changes() {
        let lines =
            |texts: &[&str]| -> Vec<String> { texts.iter().map(|text| text.to_string()).collect() };
        assert_eq!(
            say(&[], ""),
            Say::Nothing,
            "a happy tick says nothing at all"
        );
        assert_eq!(
            say(&lines(&["HCL9 is not on the bridge"]), ""),
            Say::Aloud("HCL9 is not on the bridge".to_string()),
            "the first tick to see a typo says so"
        );
        assert_eq!(
            say(
                &lines(&["HCL9 is not on the bridge"]),
                "HCL9 is not on the bridge"
            ),
            Say::Nothing,
            "and every tick after it is silent, which is what makes the first one readable"
        );
        assert_eq!(
            say(
                &lines(&["HCL8 is not on the bridge"]),
                "HCL9 is not on the bridge"
            ),
            Say::Aloud("HCL8 is not on the bridge".to_string()),
            "a DIFFERENT complaint is news again"
        );
        assert_eq!(
            say(&[], "HCL9 is not on the bridge"),
            Say::Forget,
            "and a complaint that cleared is forgotten, so its return is news"
        );
        assert_eq!(
            say(&lines(&["one", "two"]), ""),
            Say::Aloud("one | two".to_string()),
            "several complaints are remembered as one line, since the memory is one line"
        );
        assert_eq!(
            say(&lines(&["a\nb"]), ""),
            Say::Aloud("a b".to_string()),
            "and a complaint carrying a newline cannot become two remembered lines"
        );
    }
}
