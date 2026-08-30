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

/// One place the operator muted by hand, and the second that mute ends.
///
/// ONE FILE, ONE LINE PER PLACE, rather than a file per place: a room name is
/// the operator's own text, spaces and all, and a file per place would make it
/// a filename. Nothing in this crate turns typed text into a path unless a
/// predicate already vouches for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Muted {
    pub expiry: u64,
    pub place: String,
}

/// The entries the state file holds, or ONE complaint naming what is wrong
/// with it.
///
/// FAIL OPEN, which is `quiet.rs`'s direction and the OPPOSITE of the quiet
/// window's: a file this cannot vouch for mutes NOTHING, because a lights mute
/// nobody can see is the dangerous state. The caller prints the complaint and
/// carries on with every lamp loud.
///
/// A LINE IS `<epoch> <place>` AND NOTHING ELSE, with the only leniency the ONE
/// trailing newline the publish itself writes. Padding is not something this
/// ever wrote, so a file carrying it was edited by something else: a `trim()`
/// here is exactly the leniency that read `" 9223372036854775807\n"` as a live
/// mute one module over.
///
/// THE PLACE IS THE REST OF THE LINE VERBATIM, spaces and all, because a room
/// is called `3F - Master Bedroom` and splitting on whitespace would make that
/// four fields. What it may not be is empty, or padded at either end, since
/// neither would ever match the name a family claims in.
pub fn muted_entries(contents: &str) -> Result<Vec<Muted>, String> {
    let held = contents.strip_suffix('\n').unwrap_or(contents);
    let lines: Vec<&str> = held.split('\n').collect();
    if lines.len() > MAX_MUTED_PLACES {
        return Err(quiet_state_error(format!(
            "{} lines, more than the {MAX_MUTED_PLACES} places it keeps",
            lines.len()
        )));
    }
    lines.iter().map(|line| muted_entry(line)).collect()
}

/// One line of it, or the complaint that quotes the line back.
fn muted_entry(line: &str) -> Result<Muted, String> {
    let refused = || quiet_state_error(format!("{line:?}, which is not an expiry and a place"));
    let (stated, place) = line.split_once(' ').ok_or_else(refused)?;
    if place.is_empty() || place.trim() != place {
        return Err(refused());
    }
    Ok(Muted {
        expiry: crate::parse_count(stated).ok_or_else(refused)?,
        place: place.to_string(),
    })
}

/// One wording for every way the file can be wrong, since the operator's move
/// is the same for all of them and a second sentence would only make two
/// problems look like one.
fn quiet_state_error(what: String) -> String {
    format!(
        "pns: state error (lights-quiet holds {what}); nothing is quiet, and \
         the next pns lights quiet write replaces the file"
    )
}

/// How many places the ad-hoc quiet keeps at once.
///
/// MORE PLACES THAN A HOUSE HAS, and it is a guard on a file rather than a
/// policy: the command republishes the whole file every time and drops what has
/// expired, so reaching this at all means something else has been writing to
/// it. Refusing the file whole is what keeps an unbounded read off the event
/// path.
pub const MAX_MUTED_PLACES: usize = 32;

/// What the operator typed at `pns lights quiet`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuietCommand {
    /// No argument at all: say what is quiet and mute nothing. There is no
    /// untimed form, for `pns quiet`'s reason: a mute the operator forgets is
    /// a lamp that has silently stopped working.
    Report,
    Mute {
        place: String,
        seconds: u64,
    },
    Unmute {
        place: String,
    },
}

/// The typed command, or the refusal that quotes back what was typed.
///
/// A PLACE NO CLAIM NAMES IS REFUSED RATHER THAN STORED. A mute is a line in a
/// file that nothing will ever match, so the lamp the operator meant to quiet
/// goes on flashing while the command reports success; the only evidence they
/// get is the lamp itself, at the hour they were trying not to be disturbed.
/// The vocabulary is `claimed_places`', which is the names a mute can ENFORCE
/// rather than every name the config wrote down.
///
/// `off` IS ALLOWED OVER ANY NAME, because it can only remove. A place muted
/// yesterday and dropped from the config today would otherwise be a mute
/// nothing could clear, which is the state the refusal exists to prevent rather
/// than to create.
///
/// THE DURATION IS `quiet::parse_duration`'S, refusal and all, so a second
/// spelling of "how long" cannot exist and neither can a second set of bounds.
pub fn quiet_command(arguments: &[String], known: &[String]) -> Result<QuietCommand, String> {
    match arguments {
        [] => Ok(QuietCommand::Report),
        [place, word] if word == "off" => Ok(QuietCommand::Unmute {
            place: place.clone(),
        }),
        [place, word] => {
            if !known.iter().any(|name| name == place) {
                return Err(unmutable(place, known));
            }
            Ok(QuietCommand::Mute {
                place: place.clone(),
                seconds: crate::quiet::parse_duration(word)?,
            })
        }
        // ANY OTHER ARITY IS A REFUSAL, never a silent fallthrough to the
        // report: a typo the operator does not see is a mute they believe is
        // on.
        _ => Err(
            "pns: lights quiet takes a place and a duration, a place and \
                  off, or nothing at all"
                .to_string(),
        ),
    }
}

/// Why one name cannot be muted, and what can be instead.
///
/// THE ALTERNATIVES ARE LISTED, because the name refused is often one the
/// operator is reading out of their own config file: a `[lights.places]` entry
/// is a real name that a mute cannot enforce, and nothing on the page says
/// which of the two vocabularies this command speaks. A refusal that only
/// repeats what was typed sends them back to the file that misled them.
fn unmutable(place: &str, known: &[String]) -> String {
    let reaches = if known.is_empty() {
        "this config claims no lamp at all, so there is nothing a mute could \
         reach"
            .to_string()
    } else {
        format!(
            "a mute reaches {}",
            known
                .iter()
                .map(|name| format!("{name:?}"))
                .collect::<Vec<String>>()
                .join(", ")
        )
    };
    format!(
        "pns: lights quiet: {place:?} is no room or lamp a [lights.families] \
         claim names; {reaches}"
    )
}

/// What the file holds after one typed command: this place at a new expiry, or
/// gone, and every other place kept as it was.
///
/// ONE FUNCTION FOR BOTH VERBS, because they differ by one value: `off` is an
/// expiry that is not there. Written as two, the drop and the replace would be
/// two spellings of the same rebuild and only one of them would learn about the
/// pruning below.
///
/// EXPIRED ENTRIES ARE DROPPED AS IT GOES PAST, and that is not tidiness: this
/// file has a line cap and a machine that mutes a different room every night
/// would otherwise reach it and have the whole file refused, which is a corrupt
/// state the command inflicted on itself.
///
/// A CLOCK NOBODY CAN READ KEEPS EVERY OTHER ENTRY. Dropping what cannot be
/// judged would let one broken clock reading erase mutes the operator set and
/// can still see, and the only command that reaches here without a clock is
/// `off`, which has one place to remove and no opinion about the rest.
///
/// AND IT REFUSES A MUTE PAST THE CAP RATHER THAN WRITING ONE. `muted_entries`
/// rejects a file past `MAX_MUTED_PLACES` WHOLE and mutes nothing, so a command
/// that published one more line would cancel every mute on the machine at the
/// next event with nothing said anywhere. A refusal beats a truncate, which
/// would silently drop a mute the operator typed. `off` never refuses: it can
/// only shrink the file, and so can re-muting a place already in it.
pub fn muted_after(
    entries: &[Muted],
    place: &str,
    expiry: Option<u64>,
    now: Option<u64>,
) -> Result<Vec<Muted>, String> {
    let mut kept: Vec<Muted> = entries
        .iter()
        .filter(|entry| {
            entry.place != place
                && now.is_none_or(|now| crate::quiet::is_muted(Some(entry.expiry), Some(now)))
        })
        .cloned()
        .collect();
    if let Some(expiry) = expiry {
        if kept.len() >= MAX_MUTED_PLACES {
            return Err(format!(
                "pns: lights quiet: {MAX_MUTED_PLACES} places are already quiet, \
                 which is every line lights-quiet keeps; the mute was not set, \
                 and `pns lights quiet <place> off` ends one"
            ));
        }
        kept.push(Muted {
            expiry,
            place: place.to_string(),
        });
    }
    Ok(kept)
}

/// The file's body: one line per entry, in the order they are kept.
///
/// NO TRAILING NEWLINE, because `publish_state_line` writes one, and the parse
/// strips exactly that one. Two would read back as an empty last line, which
/// the parse refuses, so the round trip is what keeps this honest.
pub fn render_muted(entries: &[Muted]) -> String {
    entries
        .iter()
        .map(|entry| format!("{} {}", entry.expiry, entry.place))
        .collect::<Vec<String>>()
        .join("\n")
}

/// The places an ad-hoc quiet covers at this second.
///
/// THE VERDICT IS `quiet::is_muted`'S, never re-derived here, which is that
/// module's own rule: one property read by two readers that each decide it is
/// how a report and a behaviour come to disagree about whether a mute is on.
/// Half open comes with it, so a mute ends on the second it names.
///
/// AND FAIL OPEN comes with it too: a clock this run cannot read mutes
/// nothing. A lights mute nobody can see is the dangerous state, which is the
/// opposite direction to the quiet WINDOW one module over and deliberately so.
pub fn muted_places(entries: &[Muted], now: Option<u64>) -> Vec<String> {
    live(entries, now)
        .map(|entry| entry.place.clone())
        .collect()
}

/// What `pns lights quiet` prints, which is the whole file in the operator's
/// own vocabulary.
///
/// THE REPORT IS THE SAME READING THE LAMPS TAKE, entry for entry, because a
/// report that decided liveness for itself is how a command and a lamp come to
/// disagree about whether a room is quiet.
pub fn muted_report(entries: &[Muted], now: Option<u64>) -> Vec<String> {
    let lines: Vec<String> = live(entries, now)
        .map(|entry| {
            let minutes = crate::quiet::minutes_left(entry.expiry, now);
            let unit = if minutes == 1 { "minute" } else { "minutes" };
            format!(
                "pns lights: `{}` is quiet for another {minutes} {unit}",
                entry.place
            )
        })
        .collect();
    if lines.is_empty() {
        return vec!["pns lights: nothing is quiet".to_string()];
    }
    lines
}

/// The entries still muted at this second.
fn live(entries: &[Muted], now: Option<u64>) -> impl Iterator<Item = &Muted> {
    entries
        .iter()
        .filter(move |entry| crate::quiet::is_muted(Some(entry.expiry), now))
}

#[cfg(test)]
mod tests {
    use super::{
        Action, Breath, MAX_MUTED_PLACES, Muted, QuietCommand, Readings, Say, State, Streak,
        WORKING, any_working, breathing_since, glow_since, house_state, muted_after, muted_entries,
        muted_places, muted_report, needs_marker, needs_marker_action, needs_you_at, next_streak,
        parse_streak, quiet_command, render_muted, render_streak, say, workspace_agent_statuses,
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

    // --- the ad-hoc quiet ---------------------------------------------------

    fn muted(entries: &[(u64, &str)]) -> Vec<Muted> {
        entries
            .iter()
            .map(|(expiry, place)| Muted {
                expiry: *expiry,
                place: (*place).to_string(),
            })
            .collect()
    }

    #[test]
    fn a_state_file_that_is_not_epoch_and_place_lines_complains_and_mutes_nothing() {
        // FAIL OPEN AND SAY SO. Every row here is a file this did not write,
        // and the outcome for all of them is the same: no lamp is muted and the
        // operator is told what the file holds, because a mute nobody can see
        // is the state that costs them a notification they were waiting on.
        //
        // THE PADDED ROWS ARE THE POINT. A `trim()` here is the exact leniency
        // that read a padded epoch as a live mute one module over, so a line
        // with a space anywhere it does not belong is refused rather than read.
        for (contents, named) in [
            ("later 3F - Studio\n", "\"later 3F - Studio\""),
            ("-5 3F - Studio\n", "\"-5 3F - Studio\""),
            (" 1000 3F - Studio\n", "\" 1000 3F - Studio\""),
            ("1000  3F - Studio\n", "\"1000  3F - Studio\""),
            ("1000 3F - Studio \n", "\"1000 3F - Studio \""),
            ("1000\n", "\"1000\""),
            ("1000 \n", "\"1000 \""),
            ("1000 3F - Studio\n\n", "\"\""),
            ("\n", "\"\""),
            ("", "\"\""),
        ] {
            assert_eq!(
                muted_entries(contents),
                Err(format!(
                    "pns: state error (lights-quiet holds {named}, which is not \
                     an expiry and a place); nothing is quiet, and the next \
                     pns lights quiet write replaces the file"
                )),
                "contents: {contents:?}"
            );
        }
        // AND A FILE PAST THE CAP IS REFUSED WHOLE rather than truncated to it:
        // this command republishes the file every time and drops what expired,
        // so a file this long was written by something else and none of it can
        // be vouched for.
        let past_cap: String = (0..=MAX_MUTED_PLACES)
            .map(|index| format!("1000 room-{index}\n"))
            .collect();
        assert_eq!(
            muted_entries(&past_cap),
            Err(format!(
                "pns: state error (lights-quiet holds {} lines, more than the \
                 {MAX_MUTED_PLACES} places it keeps); nothing is quiet, and the \
                 next pns lights quiet write replaces the file",
                MAX_MUTED_PLACES + 1
            )),
            "a file past the cap"
        );
        // THE ROUND TRIP, which is what makes every refusal above a refusal of
        // something this never wrote: the place is the rest of the line
        // verbatim, spaces and all, because that is how a room is named.
        assert_eq!(
            muted_entries("1000 3F - Studio\n1800 3F - Master Bedroom\n"),
            Ok(muted(&[
                (1_000, "3F - Studio"),
                (1_800, "3F - Master Bedroom")
            ])),
            "the file this command writes reads back as what it wrote"
        );
        assert_eq!(
            muted_entries("1000 3F - Studio"),
            Ok(muted(&[(1_000, "3F - Studio")])),
            "and the one trailing newline is the only leniency there is"
        );
    }

    #[test]
    fn the_report_names_every_live_place_and_says_so_when_there_are_none() {
        // ROUNDED UP, which is `quiet::status_line`'s own rule reached through
        // its own function: a mute with forty seconds left is still on, and "0
        // minutes" reads as off.
        //
        // AND AN EXPIRED ENTRY IS NOT REPORTED, because the report and the
        // lamps read the same list through the same predicate: a command that
        // said a room was quiet while its lamps were signalling would be worse
        // than saying nothing.
        let now = 1_000;
        assert_eq!(
            muted_report(
                &muted(&[
                    (now + 40, "3F - Studio"),
                    (now + 1_620, "3F - Master Bedroom")
                ]),
                Some(now)
            ),
            vec![
                "pns lights: `3F - Studio` is quiet for another 1 minute".to_string(),
                "pns lights: `3F - Master Bedroom` is quiet for another 27 minutes".to_string(),
            ]
        );
        assert_eq!(
            muted_report(&muted(&[(now, "3F - Studio")]), Some(now)),
            vec!["pns lights: nothing is quiet".to_string()],
            "an expired entry is not a place to report"
        );
        assert_eq!(
            muted_report(&[], Some(now)),
            vec!["pns lights: nothing is quiet".to_string()],
            "and neither is an empty file"
        );
    }

    #[test]
    fn a_duration_outside_the_bounds_is_refused_by_what_was_typed() {
        // ONE SPELLING OF "HOW LONG" IN THE WHOLE CRATE. The refusal is
        // `parse_duration`'s own, word for word, because a second wording here
        // would be a second set of bounds the day either one moved.
        let known = places(&["3F - Studio"]);
        for typed in ["0s", "25h", "1441m", "9223372036854775807h"] {
            assert_eq!(
                quiet_command(&typed_at("3F - Studio", typed), &known),
                Err(format!(
                    "pns: quiet duration {typed:?} is outside 1s to 24h"
                )),
                "typed: {typed:?}"
            );
        }
        for typed in ["30", "", "1d", " 5m"] {
            assert_eq!(
                quiet_command(&typed_at("3F - Studio", typed), &known),
                Err(format!(
                    "pns: quiet duration {typed:?} is not <count><s|m|h>"
                )),
                "typed: {typed:?}"
            );
        }
        assert_eq!(
            quiet_command(&typed_at("3F - Studio", "30m"), &known),
            Ok(QuietCommand::Mute {
                place: "3F - Studio".to_string(),
                seconds: 1_800,
            }),
            "and the two ends of the range are what the bounds let through"
        );
    }

    #[test]
    fn a_place_the_config_does_not_name_is_refused_rather_than_silently_stored() {
        // A MUTE IS A LINE NOTHING WILL EVER MATCH. Stored quietly, the lamp
        // the operator meant to quiet goes on flashing while the command
        // reports success, and the only evidence they get is the lamp itself at
        // the hour they were trying not to be disturbed.
        let known = places(&["3F - Studio", "3F - Studio - HCL3"]);
        assert_eq!(
            quiet_command(&typed_at("3F - Nowhere", "30m"), &known),
            Err("pns: lights quiet: \"3F - Nowhere\" is no room or lamp a \
                 [lights.families] claim names; a mute reaches \"3F - Studio\", \
                 \"3F - Studio - HCL3\""
                .to_string()),
            "a place nothing in the config names"
        );
        assert_eq!(
            quiet_command(&typed_at("3f - studio", "30m"), &known),
            Err("pns: lights quiet: \"3f - studio\" is no room or lamp a \
                 [lights.families] claim names; a mute reaches \"3F - Studio\", \
                 \"3F - Studio - HCL3\""
                .to_string()),
            "and a case-folded one is a typo rather than a name to forgive, \
             which is how the bridge listing reads it too"
        );
        assert_eq!(
            quiet_command(&typed_at("3F - Studio - HCL3", "30m"), &known),
            Ok(QuietCommand::Mute {
                place: "3F - Studio - HCL3".to_string(),
                seconds: 1_800,
            }),
            "the control: a lamp the config names is stored"
        );
        assert_eq!(
            quiet_command(&typed_at("3F - Nowhere", "off"), &known),
            Ok(QuietCommand::Unmute {
                place: "3F - Nowhere".to_string(),
            }),
            "and `off` is allowed over any name, because it can only remove: a \
             place muted yesterday and dropped from the config today would \
             otherwise be a mute nothing could clear"
        );
        assert_eq!(
            quiet_command(&[], &known),
            Ok(QuietCommand::Report),
            "no argument reports and mutes nothing"
        );
        assert_eq!(
            quiet_command(&typed_at("3F - Studio - HCL1", "30m"), &places(&[])),
            Err(
                "pns: lights quiet: \"3F - Studio - HCL1\" is no room or lamp a \
                 [lights.families] claim names; this config claims no lamp at \
                 all, so there is nothing a mute could reach"
                    .to_string()
            ),
            "and a config that claims nothing says so rather than trailing off \
             after `a mute reaches`"
        );
        for arguments in [
            vec!["3F - Studio".to_string()],
            vec![
                "3F - Studio".to_string(),
                "30m".to_string(),
                "x".to_string(),
            ],
        ] {
            assert_eq!(
                quiet_command(&arguments, &known),
                Err("pns: lights quiet takes a place and a duration, a place \
                     and off, or nothing at all"
                    .to_string()),
                "arguments: {arguments:?}"
            );
        }
    }

    fn places(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    fn typed_at(place: &str, word: &str) -> Vec<String> {
        vec![place.to_string(), word.to_string()]
    }

    #[test]
    fn a_mute_past_the_places_the_file_keeps_is_refused_rather_than_written() {
        // THE COMMAND MUST NOT PUBLISH A FILE ITS OWN READER REFUSES WHOLE.
        // `muted_entries` rejects a file past the cap and mutes NOTHING, so one
        // line over would cancel every mute on the machine at the next event,
        // silently, at the hour the operator was trying not to be disturbed.
        let full: Vec<Muted> = (0..MAX_MUTED_PLACES)
            .map(|which| Muted {
                expiry: 9_000,
                place: format!("3F - Room {which}"),
            })
            .collect();
        assert_eq!(
            muted_after(&full, "3F - One More", Some(9_000), Some(1_000)),
            Err(
                "pns: lights quiet: 32 places are already quiet, which is every \
                 line lights-quiet keeps; the mute was not set, and `pns lights \
                 quiet <place> off` ends one"
                    .to_string()
            ),
            "a full file plus one more place is a file the reader refuses whole"
        );
        assert_eq!(
            muted_after(&full, "3F - Room 0", Some(9_500), Some(1_000)).map(|kept| kept.len()),
            Ok(MAX_MUTED_PLACES),
            "the control: re-muting a place already in the file replaces its \
             line and never reaches the cap"
        );
        assert_eq!(
            muted_after(&full, "3F - Room 0", None, Some(1_000)).map(|kept| kept.len()),
            Ok(MAX_MUTED_PLACES - 1),
            "and `off` can only shrink it, so it is never refused"
        );
        assert_eq!(
            muted_after(&full, "3F - One More", Some(9_500), Some(9_500)).map(|kept| kept.len()),
            Ok(1),
            "and a file of entries that have all expired is pruned before the \
             cap is asked about, which is what keeps a machine muting a \
             different room every night off this refusal"
        );
    }

    #[test]
    fn off_clears_one_place_and_leaves_the_others_where_they_were() {
        // THE WHOLE FILE IS REPUBLISHED EVERY TIME, so "leaves the others" is
        // the property that has to be pinned: a rewrite that dropped a sibling
        // would be a mute the operator set and can no longer see, which is the
        // silent state this path refuses everywhere else.
        let entries = muted(&[(2_000, "3F - Studio"), (3_000, "3F - Master Bedroom")]);
        assert_eq!(
            muted_after(&entries, "3F - Studio", None, Some(1_000)),
            Ok(muted(&[(3_000, "3F - Master Bedroom")])),
            "off takes the place it names and nothing else"
        );
        assert_eq!(
            muted_after(&entries, "3F - Nowhere", None, Some(1_000)),
            Ok(entries.clone()),
            "and off over a place the file does not hold changes nothing"
        );
        assert_eq!(
            muted_after(&entries, "3F - Studio", Some(9_000), Some(1_000)),
            Ok(muted(&[
                (3_000, "3F - Master Bedroom"),
                (9_000, "3F - Studio")
            ])),
            "a second mute over one place REPLACES its expiry rather than \
             adding a second line for it"
        );
        // THE PRUNE, and it is a bug fix rather than tidiness: the file has a
        // line cap, so a machine that mutes a different room every night would
        // otherwise reach it and have the whole file refused.
        assert_eq!(
            muted_after(
                &muted(&[(500, "3F - Studio"), (3_000, "3F - Master Bedroom")]),
                "3F - Kitchen",
                Some(9_000),
                Some(1_000)
            ),
            Ok(muted(&[
                (3_000, "3F - Master Bedroom"),
                (9_000, "3F - Kitchen")
            ])),
            "an entry that expired is dropped as the file goes past it"
        );
        assert_eq!(
            muted_after(
                &muted(&[(500, "3F - Studio"), (3_000, "3F - Master Bedroom")]),
                "3F - Kitchen",
                None,
                None
            ),
            Ok(muted(&[
                (500, "3F - Studio"),
                (3_000, "3F - Master Bedroom")
            ])),
            "but a clock nobody can read judges nothing, so `off` over a place \
             the file does not hold erases none of it"
        );
        // AND THE ROUND TRIP: what this writes is what the reader reads.
        let kept =
            muted_after(&entries, "3F - Studio", Some(9_000), Some(1_000)).expect("under the cap");
        assert_eq!(
            muted_entries(&format!("{}\n", render_muted(&kept))),
            Ok(kept),
            "the file this writes parses back as the entries it wrote"
        );
    }

    #[test]
    fn an_ad_hoc_quiet_ends_on_the_second_it_names_and_an_expired_file_mutes_nothing() {
        // HALF OPEN, AND THE BOUNDARY SECOND ITSELF is the assertion: a `<=`
        // here is an off-by-one nobody sees, because both neighbours agree
        // under either spelling. It is `quiet::is_muted`'s own edge, asked
        // through this reader so the two cannot come out disagreeing.
        let entries = muted(&[(1_000, "3F - Studio")]);
        assert_eq!(
            muted_places(&entries, Some(999)),
            vec!["3F - Studio".to_string()],
            "the second before the expiry is still quiet"
        );
        assert_eq!(
            muted_places(&entries, Some(1_000)),
            Vec::<String>::new(),
            "and the expiry second itself is already over"
        );
        assert_eq!(
            muted_places(&entries, Some(1_001)),
            Vec::<String>::new(),
            "as is every second after it"
        );
        // A WHOLE FILE OF EXPIRED ENTRIES MUTES NOTHING, which is the state a
        // machine that ran the command yesterday wakes up in: the file is
        // still there and every lamp is loud again.
        assert_eq!(
            muted_places(
                &muted(&[(1_000, "3F - Studio"), (900, "3F - Master Bedroom")]),
                Some(1_000)
            ),
            Vec::<String>::new(),
            "an expired file mutes nothing at all"
        );
        // AND A CLOCK NOBODY CAN READ MUTES NOTHING, which is `is_muted`'s own
        // fail-open direction: a lights mute nobody can see is the dangerous
        // state, so an unreadable clock leaves every lamp loud.
        assert_eq!(
            muted_places(&entries, None),
            Vec::<String>::new(),
            "and a clock this run cannot read mutes nothing"
        );
    }
}
