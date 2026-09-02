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

/// The two epochs the unread lamp is armed from: when a turn last finished, and
/// when one last died.
///
/// TWO FIELDS AND NOT A QUEUE, because the question is not what happened but
/// whether anything has happened since the operator last touched the machine.
/// A queue would answer the same question with a file that grows.
///
/// `None` IS "NOTHING OF THAT KIND YET", never an epoch of zero. Zero is 1970,
/// which is older than every interaction there has ever been, so a zero read as
/// a real epoch simply never arms and a zero WRITTEN as one would arm forever
/// against an unreadable interaction clock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct News {
    pub done_at: Option<u64>,
    pub failed_at: Option<u64>,
}

/// The record as one line: the two epochs, space separated, with `0` for a kind
/// that has not happened. `render_heartbeat`'s shape, and `render_streak`'s.
pub fn render_news(news: &News) -> String {
    format!(
        "{} {}",
        news.done_at.unwrap_or_default(),
        news.failed_at.unwrap_or_default()
    )
}

/// That line read back, or None for anything this will not vouch for.
///
/// REFUSED, NEVER GUESSED AT, in `parse_streak`'s style, and the fail direction
/// is DARK: a file some other hand rewrote yields no news, so the lamp stays
/// out rather than breathing about something nobody can name.
pub fn parse_news(line: &str) -> Option<News> {
    let (done, failed) = line.trim_end_matches('\n').split_once(' ')?;
    let epoch = |count: u64| (count > 0).then_some(count);
    Some(News {
        done_at: epoch(crate::parse_count(done)?),
        failed_at: epoch(crate::parse_count(failed)?),
    })
}

/// The record after one event, or None for an event that is not news.
///
/// THE TWO PULSE BEHAVIOURS AND NOTHING ELSE. A wait is the blocked lamp's
/// business and is not news the operator has missed: it is a question still on
/// screen. Reusing `pulse::state_behaviour`'s answer rather than re-reading the
/// state word is what keeps the lamp that flashes and the record that arms the
/// unread lamp from disagreeing about one event.
///
/// IT IS WRITTEN WHATEVER THE DELIVERY DID. A card that was suppressed, muted
/// or dropped is exactly the news this lamp exists to carry, so the record is
/// not a function of whether anything was delivered.
///
/// AND AN EPOCH ONLY EVER MOVES FORWARD. Two events land together often enough
/// (an agent that finished beside one that died), each reads the record and
/// publishes the whole line, so a run that was slow to publish would otherwise
/// put an OLDER second back over a newer one. What that costs is the lamp's
/// colour: a failure recorded and then overwritten is red the operator never
/// sees, and a success pushed backwards arms its lamp before it should.
pub fn news_after(held: News, behaviour: crate::config::Behaviour, now: u64) -> Option<News> {
    let forward = |at: Option<u64>| at.max(Some(now));
    match behaviour {
        crate::config::Behaviour::Done => Some(News {
            done_at: forward(held.done_at),
            ..held
        }),
        crate::config::Behaviour::Failed => Some(News {
            failed_at: forward(held.failed_at),
            ..held
        }),
        crate::config::Behaviour::Blocked
        | crate::config::Behaviour::Unread
        | crate::config::Behaviour::Looping => None,
    }
}

/// Which of the unread lamp's two colours is showing.
///
/// TWO FLAVOURS OF ONE BEHAVIOUR, never two routable behaviours: a config
/// carries `unread` or it does not, and both colours ride the lamp that carries
/// it. That is the operator's own routing map read literally, where the two are
/// always listed together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    Failure,
    Success,
}

/// Whether the unread lamp is armed, and in which colour.
///
/// THE QUESTION IS "IS THERE NEWS THE OPERATOR HAS NOT BEEN BACK FOR", and the
/// edge is their LAST INTERACTION of any kind: a key at the desk, input from the
/// phone, or the deliberate phone marker. One rule over every input rather than
/// one rule per input, which is the operator's own wording.
///
/// NOTHING WORKING, which is the other half of the condition. Work in progress
/// is the loop lamp's business and a lamp cannot be both; news that arrives
/// while a run is still going is not news anybody has missed yet.
///
/// NO INTERACTION AT ALL IS NO LAMP, never an edge at epoch zero. A machine
/// that cannot prove the operator was ever here cannot prove this news is
/// unseen either, and dark is the direction every unreadable reading on this
/// path takes.
///
/// RED WINS WHEN BOTH ARE PENDING (operator ruling): a run that died outranks
/// one that finished, and showing the calmer of the two would hide the one that
/// needs answering.
///
/// FAILURE ARMS AT ONCE AND SUCCESS WAITS. A result the operator is still
/// looking at should not light a lamp about itself, so success news has to be
/// `after_secs` old; a failure has no such grace, because the sooner they know
/// the better.
///
/// THE AGE TEST IS CLOSED AND THE EDGE TEST IS NOT, which is two different
/// questions taking the crate's two standing conventions. News exactly
/// `after_secs` old HAS waited that long (`session_was_long`'s rule), and news
/// exactly AT the interaction edge is not newer than it (`marker_is_live`'s
/// sibling rule, and the direction that leaves a lamp dark on a tie).
pub fn unread_arming(
    news: &News,
    last_interaction: Option<u64>,
    working: bool,
    now: u64,
    after_secs: u64,
) -> Option<Unread> {
    if working {
        return None;
    }
    let edge = last_interaction?;
    // NEWS FROM THE FUTURE IS NEWS NOBODY CAN JUDGE, and it arms nothing of
    // either flavour. A clock that stepped backwards leaves an epoch ahead of
    // now, and the record only ever moves FORWARD, so nothing later will pull it
    // back: read as ordinary news it is newer than every interaction there will
    // ever be, and the lamp would hold red until wall time caught up with it.
    // The success flavour has always taken this direction through its age test;
    // this is the same rule said once for both.
    let unseen = |at: Option<u64>| at.filter(|at| *at > edge && *at <= now);
    if unseen(news.failed_at).is_some() {
        return Some(Unread::Failure);
    }
    unseen(news.done_at)
        .filter(|at| now.checked_sub(*at).is_some_and(|age| age >= after_secs))
        .map(|_| Unread::Success)
}

/// When the operator last touched the machine, from the three roads' own
/// readings: the desk clock's idle age, and the two phone epochs.
///
/// THE FRESHEST OF THE THREE, which is the operator's "any input, one clear
/// rule". Taking the stalest would arm the unread lamp about news they had
/// already seen through whichever road they were actually using.
///
/// THE DESK READING IS AN AGE AND THE OTHER TWO ARE EPOCHS, which is why it is
/// subtracted here rather than compared: an idle clock counts back from now,
/// and the saturation is for an idle age longer than the clock itself, which is
/// an interaction at the epoch rather than a wrapped one in the far future.
///
/// NONE WHEN NONE OF THEM CAN BE READ, never an epoch of zero. A machine that
/// cannot prove the operator was ever here cannot prove any news is unseen
/// either, and dark is the direction every unreadable reading on this path
/// takes.
pub fn last_interaction(
    desk_idle_secs: Option<u64>,
    phone_input_at: Option<u64>,
    phone_marker_at: Option<u64>,
    now: u64,
) -> Option<u64> {
    let desk = desk_idle_secs.map(|idle| now.saturating_sub(idle));
    [desk, phone_input_at, phone_marker_at]
        .into_iter()
        .flatten()
        .max()
}

/// Everything the loop condition is a function of.
///
/// A NAMED STRUCT rather than six positional arguments, four of which are
/// `u64`-shaped: a transposition between the two thresholds, or between `now`
/// and either of them, is a lamp judged against the wrong clock and nothing
/// would catch it.
pub struct Loop<'reading> {
    /// The AGENTS' run in progress, which is the only source whose start has to
    /// be inferred: herdr answers a status word and no clock, so the run is
    /// timed from the first tick that read one working.
    pub streak: Option<&'reading Streak>,
    /// Whether any agent is working right now.
    pub agents_working: bool,
    /// When the longest-running tracked shell command STARTED, which is an
    /// exact epoch the shell itself published. It needs no streak: the marker
    /// exists for exactly as long as the command runs.
    pub shell_since: Option<u64>,
    /// When each live lease was last renewed. EMPTY IS THE ORDINARY CASE.
    pub leases: &'reading [u64],
    pub now: u64,
    /// How long tracked work must run continuously before the lamp arms itself.
    pub threshold_secs: u64,
    /// How long a lease survives with nothing renewing it.
    pub lease_timeout_secs: u64,
}

/// Whether the loop lamp is on.
///
/// TWO TRIGGERS AND AN OR, which is the operator's own design: work that has
/// been going long enough arms it by itself, and `pns loop begin` arms it by
/// hand for work whose length nothing can measure in advance. Either is enough,
/// and neither can turn the other off.
///
/// EACH SOURCE IS TIMED AGAINST ITS OWN START, and pooling them was wrong in
/// both directions. The shell publishes the second its command began, so a
/// build is measured from when it really started; an agent gives a status word
/// and nothing else, so its run is timed from the first tick that read it
/// working, and that streak deliberately outlives the work by the grace
/// covering an agent's turn gap. Shared, a fresh five-second command starting
/// inside that grace inherited a finished agent's run and armed the lamp at
/// once, while a build already ten minutes in was clocked from now and had to
/// wait out the whole threshold again.
///
/// BOTH HALVES OF THE AGENT ONE. The streak outliving the work is exactly why
/// the threshold alone would keep the lamp claiming a run in progress after
/// everything went idle.
///
/// AND THE SHELL NEEDS NO SECOND HALF, because its marker exists for exactly as
/// long as its command runs: the reading IS the liveness.
///
/// A `now` BEHIND A START HAS NO ELAPSED TIME IN IT. A clock that stepped
/// backwards would otherwise wrap a subtraction into a huge number that passes
/// every threshold there is.
pub fn loop_running(state: &Loop<'_>) -> bool {
    let long_enough = |since: u64| {
        state
            .now
            .checked_sub(since)
            .is_some_and(|elapsed| elapsed >= state.threshold_secs)
    };
    let agent_run =
        state.agents_working && state.streak.is_some_and(|streak| long_enough(streak.since));
    agent_run
        || state.shell_since.is_some_and(long_enough)
        || state
            .leases
            .iter()
            .any(|at| marker_is_live(*at, state.now, state.lease_timeout_secs))
}

/// What the operator typed at `pns loop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopCommand {
    /// Take a lease for this pane, and hold the lamp until it is given back or
    /// times out.
    Begin(String),
    /// Give it back.
    End(String),
}

/// Where a pane's loop lease lives: one file per pane holding one epoch.
///
/// A DIRECTORY, LIKE THE WAITS, because several panes can each be running a
/// loop and each must be the only writer and the only ordinary remover of its
/// own file. One shared file would be a lease any other pane erases.
pub fn lease_dir(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("lights-loop")
}

/// One pane's lease path, or None for a pane id that cannot become a filename.
pub fn lease_marker(state_dir: &std::path::Path, pane: &str) -> Option<std::path::PathBuf> {
    crate::safety::pane_file_is_safe(pane).then(|| lease_dir(state_dir).join(pane))
}

/// The typed command, or the refusal that says what was missing.
///
/// THE PANE IS THE OPERATOR'S OWN, TAKEN FROM THE ENVIRONMENT they typed in,
/// because that is what a lease is keyed to: `HERDR_PANE_ID` is set for every
/// pane herdr owns, so the ordinary case needs no argument at all.
///
/// TYPED OUTSIDE A PANE IT IS A REFUSAL, NEVER A GUESS. There is no sensible
/// pane to pick, and picking one would key the lease to a pane whose ordinary
/// traffic will never renew it: the lamp would then breathe for the whole
/// timeout with nothing behind it, which is the exact opposite of a liveness
/// signal.
///
/// AN UNKNOWN ARGUMENT IS AN ERROR, never a silent fallthrough, because a
/// mistyped flag would otherwise be a lease the operator believes they took.
pub fn loop_command(
    verb: &str,
    arguments: &[String],
    env_pane: Option<&str>,
) -> Result<LoopCommand, String> {
    let pane = match arguments {
        [] => env_pane
            .filter(|pane| !pane.is_empty())
            .map(str::to_string)
            .ok_or_else(|| NO_PANE.to_string())?,
        [flag, pane] if flag == "--pane" => pane.clone(),
        _ => return Err(LOOP_USAGE.to_string()),
    };
    if !crate::safety::pane_file_is_safe(&pane) {
        return Err(format!(
            "pns: loop: {pane:?} is not a pane id this can key a lease to"
        ));
    }
    match verb {
        "begin" => Ok(LoopCommand::Begin(pane)),
        "end" => Ok(LoopCommand::End(pane)),
        _ => Err(LOOP_USAGE.to_string()),
    }
}

/// Why a lease cannot be taken with no pane to key it to.
const NO_PANE: &str = "pns: loop: no HERDR_PANE_ID in this environment, so there \
is no pane to key the lease to; run it inside the pane, or name one with --pane";

pub const LOOP_USAGE: &str = "pns: usage: pns loop begin [--pane <id>] | \
pns loop end [--pane <id>]";

/// The run that owns a WORKING FILE in a marker directory, or None for an
/// ordinary marker.
///
/// TWO SUFFIXES AND ONE ANSWER. A publish writes `<name>.new.<pid>` beside the
/// marker it is about to rename over, and a sweep writes `<name>.sweep.<pid>`
/// when it takes one to remove it. Both are one run's private working name,
/// both carry that run's own process id, and a sweep has to tell them from the
/// markers it is there to judge.
///
/// THE PID IS WHAT MAKES IT DECIDABLE, and matching the bare suffix was not.
/// Pane ids and session ids are opaque words from another program, and both
/// alphabets admit a dot: a pane called `a.new.b` produced a lease file every
/// sweep stepped over, so it aged out never, while a working file whose own run
/// had died was never collected either. A name is a working file only when what
/// follows the LAST such marker is a positive process id, which is a name only
/// this crate's own writers produce.
pub fn working_owner(name: &str) -> Option<&str> {
    let (_, owner) = name
        .rsplit_once(WORKING_PENDING)
        .or_else(|| name.rsplit_once(WORKING_SWEEP))?;
    (crate::parse_count(owner)? > 0).then_some(owner)
}

/// The two working-file markers, in the spelling their writers use.
const WORKING_PENDING: &str = ".new.";
const WORKING_SWEEP: &str = ".sweep.";

/// One run's private name for a marker it has taken to remove.
pub fn sweep_claim(directory: &std::path::Path, name: &str, pid: u32) -> std::path::PathBuf {
    directory.join(format!("{name}{WORKING_SWEEP}{pid}"))
}

/// Whether any wait is still live.
pub fn any_blocked(marker_epochs: &[u64], now: u64, max_age_secs: u64) -> bool {
    marker_epochs
        .iter()
        .any(|at| marker_is_live(*at, now, max_age_secs))
}

/// Whether one epoch is still inside its bound.
///
/// ONE PREDICATE FOR EVERY AGED MARKER IN THIS MODULE, because each of them has
/// two readers that must agree: the aggregate that lights a lamp, and the sweep
/// that DELETES what has aged out. Two spellings of "expired" would be a marker
/// the aggregate ignored and the sweep kept, accumulating forever, or one the
/// sweep removed while the aggregate was still lighting a lamp for it.
///
/// BOTH EDGES CLOSED: exactly at the bound is still live. A MARKER FROM THE
/// FUTURE IS LIVE TOO, because a clock that stepped backwards is not a wait that
/// ended, and the saturating subtraction reads it as zero seconds old rather
/// than as an enormous age that would delete it.
pub fn marker_is_live(at: u64, now: u64, max_age_secs: u64) -> bool {
    now.saturating_sub(at) <= max_age_secs
}

/// One HELD state, and the four of them in the order they outrank each other.
///
/// THE DECLARATION ORDER IS THE RANK, and `active_held` pushes in that fixed
/// order rather than sorting: nothing here compares one `Held` to another at
/// runtime. Blocked is on top, which is the operator's own ruling: a question
/// waiting on them outranks work in progress, and work in progress outranks
/// news about work that has already finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Blocked,
    Looping,
    UnreadFailure,
    UnreadSuccess,
}

impl Held {
    /// The ROUTABLE word this state is carried by. The two unread flavours
    /// answer the same word, which is what makes a lamp carry both or neither.
    pub fn behaviour(self) -> crate::config::Behaviour {
        match self {
            Held::Blocked => crate::config::Behaviour::Blocked,
            Held::Looping => crate::config::Behaviour::Looping,
            Held::UnreadFailure | Held::UnreadSuccess => crate::config::Behaviour::Unread,
        }
    }
}

/// What the house is holding this tick, one field per state.
///
/// A NAMED STRUCT rather than three positional values, two of which are bools:
/// a transposition would be a lamp showing the wrong state and nothing would
/// catch it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct House {
    pub blocked: bool,
    pub looping: bool,
    pub unread: Option<Unread>,
}

/// Every state the house is holding, most urgent first.
///
/// A LIST RATHER THAN ONE STATE, which is the whole difference from the shipped
/// design: the house holds all of them at once and each LAMP resolves which one
/// it shows, so a blue lamp and a violet lamp can be lit at the same moment
/// because they are routed for different words.
///
/// THE PUSHES ARE IN RANK ORDER and there is no sort behind them. One was here
/// and could never change the answer, which is exactly the code a reader trusts
/// and a mutation walks straight through. What pins the order instead is the
/// test that asserts the whole vector, so pushing out of order is red.
pub fn active_held(house: &House) -> Vec<Held> {
    let mut held = Vec::new();
    if house.blocked {
        held.push(Held::Blocked);
    }
    if house.looping {
        held.push(Held::Looping);
    }
    match house.unread {
        Some(Unread::Failure) => held.push(Held::UnreadFailure),
        Some(Unread::Success) => held.push(Held::UnreadSuccess),
        None => {}
    }
    held
}

/// What ONE lamp shows: the most urgent active state it is routed for, or
/// nothing.
///
/// THE LAMP'S OWN ROUTING IS THE FILTER, so a state nothing routes to that lamp
/// leaves it dark rather than falling through to a lamp that was not asked. That
/// is what lets one house state reach three lamps saying different things.
pub fn shown(active: &[Held], shows: &[crate::config::Behaviour]) -> Option<Held> {
    active
        .iter()
        .copied()
        .find(|held| shows.contains(&held.behaviour()))
}

/// Whether a PULSE fires on one lamp.
///
/// A HELD STATE PREEMPTS A PULSE ON THE LAMP THAT IS HOLDING IT, which is the
/// operator's "dedicated, but it helps out when free" ruling generalised: a lamp
/// dedicated to the held states joins the pulse lamps whenever none of them is
/// active, and stops joining the moment one is. The pulse still fires on every
/// OTHER lamp routed for it, so nothing is lost, and the held state is not
/// interrupted by a four-second blink it would have to be re-armed after.
pub fn pulse_fires(
    shows: &[crate::config::Behaviour],
    behaviour: crate::config::Behaviour,
    lamp_is_held: bool,
) -> bool {
    shows.contains(&behaviour) && !lamp_is_held
}

/// One brightness the lamp is asked to fade to, and when the fade is issued.
///
/// `start_ms` IS FROM THE TICK'S OWN START, not from the fade before it, because
/// the driver sleeps against one clock: a per-fade delay accumulates every
/// sleep's own overshoot and the breath drifts past the interval it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fade {
    pub brightness: u8,
    pub start_ms: u64,
}

/// How far before a fade ends the next one is issued.
///
/// THE SEAMLESS TURN-AROUND, operator-locked on a real lamp: the next fade is
/// issued slightly BEFORE the previous one ends, so the lamp never sits at
/// either end of the breath. Fifty milliseconds is the figure that was set and
/// looked at; nothing here measured what a lead of zero looks like.
pub const FADE_LEAD_MS: u64 = 50;

/// Which end of a breath a lamp is fading TOWARD, or landed ON.
///
/// TWO VALUES, NEVER A BARE BOOL, because this crosses a file boundary (the
/// held record's `:h`/`:l` suffix): a field that only this module reads can
/// afford to be self-documenting at the call site instead of at its
/// declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    High,
    Low,
}

/// Where a breath resumes: the millisecond its first fade is due, measured
/// from THIS tick's own start, and which end it moves toward first.
///
/// A ZERO-VALUED `Resume` (due at once, moving toward low first) REPRODUCES
/// THE ORIGINAL, UNBROKEN BREATH: a lamp with no record to resume from is a
/// lamp that has never breathed, and starting it down at the tick's first
/// millisecond is the only honest answer for one.
///
/// A NAMED STRUCT, NOT TWO POSITIONAL `u64`S, because a resume built with the
/// fields swapped would compile and breathe the wrong way from the wrong
/// moment; the two are never interchangeable so the type keeps them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resume {
    pub first_due_ms: u64,
    pub from_high: bool,
}

impl Default for Resume {
    fn default() -> Self {
        Resume {
            first_due_ms: 0,
            from_high: true,
        }
    }
}

/// The whole breath one tick issues: the fades, in order, with the second one
/// leading the first by `FADE_LEAD_MS` and so on.
///
/// EVERY FADE IS ISSUED STRICTLY INSIDE THE BUDGET, AND THE LAST ONE ENDS
/// AFTER IT. That is what makes the breath seamless rather than paused at its
/// peak: the driver used to stop issuing once a fade's whole DURATION could
/// not fit, which left the lamp holding an end for whatever was left of the
/// interval (a third of it, at the shipped refresh). Ending the schedule at
/// the last ISSUE instead means the lamp is still moving when this tick's
/// child exits; the fade in flight simply keeps running on the bridge with no
/// child left to interrupt it, and the next tick's own first fade is timed to
/// take over `FADE_LEAD_MS` before that one would have ended (`resume_from`
/// reads that from the held record). The residual pause this leaves is bounded
/// by one step of slack plus the next tick's own resolve and the daemon's
/// second of scheduling slop, worst case, and it is zero on most ticks: the
/// two resolves do not cancel every time, so the bound is a ceiling and not
/// an average.
///
/// A RESUME SHIFTS EVERY FADE'S DUE MILLISECOND by `first_due_ms` and its
/// FIRST TARGET by `from_high` (moving toward `low` when `from_high`, and
/// toward `high` otherwise), so the schedule this tick issues is the next leg
/// of the breath the previous tick was already running, not a fresh one
/// restarted at the interval's zero.
///
/// A SCHEDULE THAT WOULD START AT OR PAST THE BUDGET IS EMPTY, which is the
/// same honest answer a schedule with no room for even one fade always gave:
/// the lamp keeps whatever it was last told and the next tick, with its whole
/// interval ahead of it, picks the breath back up.
pub fn breath_fades(budget_ms: u64, breath: &crate::config::Breath, resume: Resume) -> Vec<Fade> {
    let step_ms = breath.duration_ms.saturating_sub(FADE_LEAD_MS).max(1);
    if resume.first_due_ms >= budget_ms {
        return Vec::new();
    }
    let remaining_ms = budget_ms - resume.first_due_ms;
    let count = remaining_ms.div_ceil(step_ms);
    (0..count)
        .map(|index| Fade {
            brightness: if (index % 2 == 0) == resume.from_high {
                breath.low
            } else {
                breath.high
            },
            start_ms: resume.first_due_ms + index * step_ms,
        })
        .collect()
}

/// One lamp's line in the held record: the fixture path, and where in its
/// breath it left off.
///
/// `resume` IS `None` FOR A BARE PATH, which is a lamp the record holds with
/// no phase attached: a fresh arm, a phase write a race stood down, or a
/// token an older build or a hand edit left without one. All three read the
/// same way, as a breath that starts fresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldEntry {
    pub path: String,
    pub resume: Option<(u64, End)>,
}

impl HeldEntry {
    /// A lamp held with no phase recorded for it.
    pub fn bare(path: impl Into<String>) -> HeldEntry {
        HeldEntry {
            path: path.into(),
            resume: None,
        }
    }
}

/// One held-record token, rendered: the bare path, or the path with its
/// phase, `@<end-unix-ms>:h` or `@<end-unix-ms>:l`.
///
/// `@` AND `:` NEITHER APPEAR IN A FIXTURE PATH (`light/<id>` or
/// `grouped_light/<id>`, the id a bridge-issued UUID), so the token round
/// trips through the same whitespace-separated line the bare record always
/// used, with nothing to escape.
pub fn render_held_token(entry: &HeldEntry) -> String {
    match entry.resume {
        Some((end_unix_ms, End::High)) => format!("{}@{end_unix_ms}:h", entry.path),
        Some((end_unix_ms, End::Low)) => format!("{}@{end_unix_ms}:l", entry.path),
        None => entry.path.clone(),
    }
}

/// One held-record token, parsed.
///
/// A MALFORMED SUFFIX IS NO PHASE, NEVER UNREADABLE: the record as a whole
/// already has an unreadable answer (`None`, held_lamps' own `Err(_)` arm) for
/// a file nothing here could open at all, and a token that arrived from an
/// older build, a hand edit, or a write this tick's own guard cut short is a
/// path this run still knows how to put out. Losing the phase costs one fade
/// of resume; inventing an unreadable path would cost the lamp.
pub fn parse_held_token(token: &str) -> HeldEntry {
    let Some((path, suffix)) = token.split_once('@') else {
        return HeldEntry::bare(token);
    };
    let phase = suffix.split_once(':').and_then(|(end_ms, flag)| {
        let end_unix_ms: u64 = end_ms.parse().ok()?;
        let end = match flag {
            "h" => End::High,
            "l" => End::Low,
            _ => return None,
        };
        Some((end_unix_ms, end))
    });
    match phase {
        Some(resume) => HeldEntry {
            path: path.to_string(),
            resume: Some(resume),
        },
        None => HeldEntry::bare(path),
    }
}

/// The `Resume` a lamp's next breath starts from, off what its held entry
/// last recorded.
///
/// `first_due_ms` IS THE RECORDED END, LESS THE SEAMLESS LEAD, LESS NOW,
/// SATURATING AT ZERO: the previous tick's last fade does not finish landing
/// on the bridge until that instant, and the next one has to be issued
/// `FADE_LEAD_MS` before it, exactly as every fade inside one tick already is.
/// A `now_ms` past that moment (a tick that ran late, or the bridge holding
/// the lamp at its recorded end since nothing else has moved it) saturates to
/// zero: due at once, not due in the past.
///
/// NO ENTRY AND NO PHASE BOTH GIVE THE DEFAULT `Resume`, which starts the
/// breath down at once: the lamp has never breathed, or something else put it
/// somewhere this record does not describe (an external switch, a killed
/// child's bare token, a dim-window shape change), and starting fresh from the
/// low end costs at most one fade of motion, never a pause.
pub fn resume_from(entry: Option<&HeldEntry>, now_ms: u64) -> Resume {
    let Some((end_unix_ms, end)) = entry.and_then(|entry| entry.resume) else {
        return Resume::default();
    };
    Resume {
        first_due_ms: end_unix_ms
            .saturating_sub(now_ms)
            .saturating_sub(FADE_LEAD_MS),
        from_high: matches!(end, End::High),
    }
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
/// IT READS `pulse::LAMP_BLOCKED`, the list the lamps already carry, and NOT
/// `missed_notifications::NEEDS_YOU`, which correctly includes `failed`. A dead
/// turn is red, not blue, and it is not a wait anybody can end.
pub fn blocked_marker_action(event_state: &str) -> Action {
    if crate::pulse::LAMP_BLOCKED.contains(&event_state) {
        Action::Start
    } else {
        Action::End
    }
}

/// Where the needs markers live: one file per waiting session.
pub fn blocked_dir(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("lights-blocked")
}

/// One session's marker path, or None for a session id that cannot become a
/// filename.
///
/// THE SESSION ID AND NOT THE PANE, and the difference is a path escape.
/// `pane_is_safe` permits `..` because a pane id becomes a shell WORD, never a
/// filename; `session_id_is_safe` forbids it and already backs a filename in
/// this same directory (`session-<id>.start`). Reusing it writes no new
/// predicate and opens no new door.
pub fn blocked_marker(state_dir: &std::path::Path, session_id: &str) -> Option<std::path::PathBuf> {
    crate::safety::session_id_is_safe(session_id).then(|| blocked_dir(state_dir).join(session_id))
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
/// The vocabulary is the caller's `known`, which is every name a mute can
/// ENFORCE at any of the three levels.
///
/// `off` IS ALLOWED OVER ANY NAME, because it can only remove. A place muted
/// yesterday and dropped from the config today would otherwise be a mute
/// nothing could clear, which is the state the refusal exists to prevent rather
/// than to create.
///
/// THE DURATION IS `quiet::parse_duration`'S, refusal and all, so a second
/// spelling of "how long" cannot exist and neither can a second set of bounds.
pub fn quiet_command(
    arguments: &[String],
    known: &[String],
    until_quiet_ends: Option<u64>,
) -> Result<QuietCommand, String> {
    match arguments {
        [] => Ok(QuietCommand::Report),
        [place, word] if word == "off" => Ok(QuietCommand::Unmute {
            place: place.clone(),
        }),
        [place] => {
            if !known.iter().any(|name| name == place) {
                return Err(unmutable(place, known));
            }
            // NO SCHEDULE IS A REFUSAL, never a guessed duration. A bare mute
            // means "until my quiet hours end", and a machine that has not said
            // when those are has not said how long this mute lasts; picking a
            // length would be a mute the operator did not ask for, ending at an
            // hour they cannot predict.
            let Some(seconds) = until_quiet_ends else {
                return Err(NO_SCHEDULE.to_string());
            };
            Ok(QuietCommand::Mute {
                place: place.clone(),
                seconds,
            })
        }
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
            "pns: lights quiet takes a place, optionally with a duration or \
                  off, or nothing at all"
                .to_string(),
        ),
    }
}

/// Why a bare mute cannot be set on a machine with no quiet hours.
const NO_SCHEDULE: &str = "pns: lights quiet: a bare mute lasts until your quiet \
hours end, and `[plugins.hue] quiet_hours` states none; give a duration instead, \
or set that key";

/// How long a BARE mute lasts: from now until the operator's quiet hours end.
///
/// THE SCHEDULE IS `[plugins.hue] quiet_hours` and there is no second one. A
/// mute typed at bedtime is about the operator's night, not about one room's
/// own dim window, and a room's window is a rendering rule that has nothing to
/// say about how long a by-hand silence should last.
///
/// NONE WHEN EITHER READING IS MISSING. No schedule is the refusal above; no
/// clock is a mute nothing could time, and the caller already refuses without
/// one.
///
/// NOW AT THE END MINUTE IS A WHOLE DAY, not nothing. The window ends at this
/// second, so the next end is tomorrow's; a mute of zero seconds is not a mute,
/// and the operator asked for one.
pub fn bare_mute_secs(ends_at: Option<u16>, minutes_now: Option<u16>) -> Option<u64> {
    let (ends_at, now) = (ends_at?, minutes_now?);
    const DAY: u64 = 24 * 60;
    let until = (u64::from(ends_at) + DAY - u64::from(now)) % DAY;
    Some(if until == 0 { DAY } else { until } * 60)
}

/// Why one name cannot be muted, and what can be instead.
///
/// THE ALTERNATIVES ARE LISTED, because the name refused is often one the
/// operator is reading off their own config file or off the bridge's app, and
/// nothing on either page says which names a mute can reach. A refusal that
/// only repeats what was typed sends them back to whichever of the two misled
/// them.
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
        "pns: lights quiet: {place:?} is no lamp, room or zone this can quiet; \
         {reaches}"
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
        Action, End, FADE_LEAD_MS, Fade, Held, HeldEntry, House, LOOP_USAGE, Loop, LoopCommand,
        MAX_MUTED_PLACES, Muted, News, QuietCommand, Resume, Say, Streak, Unread, WORKING,
        active_held, any_blocked, any_working, bare_mute_secs, blocked_marker,
        blocked_marker_action, breath_fades, last_interaction, lease_marker, loop_command,
        loop_running, muted_after, muted_entries, muted_places, muted_report, news_after,
        next_streak, parse_held_token, parse_news, parse_streak, pulse_fires, quiet_command,
        render_held_token, render_muted, render_news, render_streak, resume_from, say, shown,
        unread_arming, working_owner, workspace_agent_statuses,
    };
    use crate::config::Behaviour;

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
    fn a_working_file_is_told_from_a_marker_by_the_process_id_that_owns_it() {
        // THE COLLISION THIS EXISTS TO CLOSE. Pane ids and session ids are
        // opaque words from another program and both alphabets admit a dot, so
        // a name matched on the bare suffix put a real marker beyond every
        // sweep: it aged out never and its lamp could not be released.
        assert_eq!(working_owner("s1.new.4321"), Some("4321"));
        assert_eq!(working_owner("wW:p21.sweep.99"), Some("99"));
        assert_eq!(
            working_owner("a.new.b"),
            None,
            "a pane whose own name spells the suffix is a MARKER, not a publish"
        );
        for marker in [
            "s1",
            "wW:p21",
            "a.new.",
            "a.new.0",
            "a.sweep.-1",
            "a.new.b.c",
        ] {
            assert_eq!(working_owner(marker), None, "{marker:?} is a marker");
        }
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

    /// A run of work that started `ago` seconds before now.
    fn streak_from(ago: u64) -> Streak {
        Streak {
            since: NOW - ago,
            last_seen: NOW,
        }
    }

    // --- the news record ----------------------------------------------------

    #[test]
    fn the_news_record_survives_as_one_line_and_anything_else_is_no_news() {
        let both = News {
            done_at: Some(1_000),
            failed_at: Some(1_200),
        };
        assert_eq!(render_news(&both), "1000 1200");
        assert_eq!(parse_news("1000 1200"), Some(both));
        // ZERO IS "NOT YET", both ways round, so the record round-trips through
        // a state file that has only ever seen one kind of event.
        let only_done = News {
            done_at: Some(1_000),
            failed_at: None,
        };
        assert_eq!(render_news(&only_done), "1000 0");
        assert_eq!(parse_news("1000 0"), Some(only_done));
        assert_eq!(parse_news("0 0"), Some(News::default()));
        // REFUSED, NEVER GUESSED AT, and the fail direction is dark: a file some
        // other hand rewrote yields no news, so nothing arms.
        for garbled in [
            "",
            "1000",
            "1000 1200 1400",
            "x 1200",
            "1000 x",
            " 1000 1200",
        ] {
            assert_eq!(parse_news(garbled), None, "{garbled:?} is not news");
        }
    }

    #[test]
    fn the_news_record_only_ever_moves_an_epoch_forward() {
        // TWO PROCESSES WRITE THIS RECORD, and they are two events landing
        // together: an agent that finished beside one that died. Each reads,
        // changes its own field and publishes the whole line, so the slower
        // reader can put an OLDER second back over a newer one. What that costs
        // is the unread lamp's colour: a failure recorded at the newer second
        // and then overwritten with the older one is red the lamp never shows,
        // or a success armed five minutes before it should be.
        let held = News {
            done_at: Some(2_000),
            failed_at: Some(2_100),
        };
        assert_eq!(
            news_after(held, Behaviour::Done, 1_000),
            Some(held),
            "a run publishing late leaves the newer second where it is"
        );
        assert_eq!(
            news_after(held, Behaviour::Failed, 1_000),
            Some(held),
            "and so does the other kind"
        );
        assert_eq!(
            news_after(held, Behaviour::Done, 2_000),
            Some(held),
            "the same second is not forward either, so a repeat writes nothing new"
        );
    }

    #[test]
    fn only_a_finished_or_a_dead_turn_is_news_and_a_wait_is_not() {
        let held = News {
            done_at: Some(1_000),
            failed_at: Some(1_100),
        };
        assert_eq!(
            news_after(held, Behaviour::Done, 2_000),
            Some(News {
                done_at: Some(2_000),
                failed_at: Some(1_100)
            }),
            "a finished turn moves its own epoch and leaves the other where it was"
        );
        assert_eq!(
            news_after(held, Behaviour::Failed, 2_000),
            Some(News {
                done_at: Some(1_000),
                failed_at: Some(2_000)
            }),
            "and a dead one moves the other"
        );
        // A WAIT IS NOT NEWS. It is a question still on screen, which is the
        // blocked lamp's own business; recording it here would arm the unread
        // lamp about something nobody has missed.
        for not_news in [Behaviour::Blocked, Behaviour::Unread, Behaviour::Looping] {
            assert_eq!(
                news_after(held, not_news, 2_000),
                None,
                "{not_news:?} is not news"
            );
        }
    }

    // --- the unread lamp ----------------------------------------------------

    const AFTER: u64 = 300;

    fn news(done_ago: Option<u64>, failed_ago: Option<u64>) -> News {
        News {
            done_at: done_ago.map(|ago| NOW - ago),
            failed_at: failed_ago.map(|ago| NOW - ago),
        }
    }

    #[test]
    fn unread_arms_on_news_the_operator_has_not_been_back_for_and_on_nothing_else() {
        const IDLE: bool = false;
        const BUSY: bool = true;
        let long_ago = Some(NOW - 5_000);
        assert_eq!(
            unread_arming(&news(Some(AFTER), None), long_ago, IDLE, NOW, AFTER),
            Some(Unread::Success),
            "news newer than the last interaction, with nothing running: the lamp arms"
        );
        assert_eq!(
            unread_arming(&news(Some(AFTER), None), long_ago, BUSY, NOW, AFTER),
            None,
            "the same news with something working is the loop lamp's business"
        );
        assert_eq!(
            unread_arming(
                &news(Some(AFTER), None),
                Some(NOW - AFTER + 1),
                IDLE,
                NOW,
                AFTER
            ),
            None,
            "an interaction AFTER the news is the operator having seen it"
        );
        assert_eq!(
            unread_arming(
                &news(Some(AFTER), None),
                Some(NOW - AFTER),
                IDLE,
                NOW,
                AFTER
            ),
            None,
            "news exactly AT the interaction edge is not newer than it; dark on a tie"
        );
        assert_eq!(
            unread_arming(&news(Some(AFTER), None), None, IDLE, NOW, AFTER),
            None,
            "no interaction at all is no proof the news is unseen, so the lamp stays dark"
        );
        assert_eq!(
            unread_arming(&News::default(), long_ago, IDLE, NOW, AFTER),
            None,
            "and a record with nothing in it arms nothing"
        );
    }

    #[test]
    fn success_news_waits_out_its_delay_and_failure_news_does_not() {
        let long_ago = Some(NOW - 5_000);
        assert_eq!(
            unread_arming(&news(Some(AFTER - 1), None), long_ago, false, NOW, AFTER),
            None,
            "one second under the delay, a result the operator may still be looking at"
        );
        assert_eq!(
            unread_arming(&news(Some(AFTER), None), long_ago, false, NOW, AFTER),
            Some(Unread::Success),
            "exactly at it, it arms: news that old HAS waited that long"
        );
        // FAILURE HAS NO DELAY AT ALL, which is the operator's own ruling: the
        // sooner they know a run died, the better.
        assert_eq!(
            unread_arming(&news(None, Some(0)), long_ago, false, NOW, AFTER),
            Some(Unread::Failure),
            "a failure this second arms this second"
        );
        // RED WINS WHEN BOTH ARE PENDING, whichever is fresher, because showing
        // the calmer of the two would hide the one that needs answering.
        assert_eq!(
            unread_arming(&news(Some(AFTER), Some(0)), long_ago, false, NOW, AFTER),
            Some(Unread::Failure),
            "a failure outranks a success that has waited out its whole delay"
        );
        assert_eq!(
            unread_arming(&news(Some(0), Some(AFTER)), long_ago, false, NOW, AFTER),
            Some(Unread::Failure),
            "and it still outranks it when the success is the fresher of the two"
        );
        // A CLOCK BEHIND THE NEWS HAS NO AGE IN IT, so a machine whose clock
        // stepped back does not read a huge age through a wrapping subtraction.
        assert_eq!(
            unread_arming(
                &News {
                    done_at: Some(NOW + 500),
                    failed_at: None
                },
                long_ago,
                false,
                NOW,
                AFTER
            ),
            None,
            "a now before the news has no elapsed time in it"
        );
        // AND A FAILURE FROM THE FUTURE ARMS NOTHING EITHER, which is the same
        // rule for the flavour that has no age test of its own. The record only
        // ever moves FORWARD, so a clock that stepped backwards leaves an epoch
        // nothing later will pull back: read as ordinary news it is newer than
        // every interaction there will ever be, and the lamp would hold red
        // until wall time caught up with it.
        assert_eq!(
            unread_arming(
                &News {
                    done_at: None,
                    failed_at: Some(NOW + 500)
                },
                long_ago,
                false,
                NOW,
                AFTER
            ),
            None,
            "a failure the clock says has not happened yet arms no lamp"
        );
        // AND STILL NOT WITH NO DELAY AT ALL. `after_secs` may be zero, and a
        // saturated age of zero passes a zero threshold, so this edge is where
        // "no elapsed time" and "an elapsed time of zero" stop agreeing.
        assert_eq!(
            unread_arming(
                &News {
                    done_at: Some(NOW + 500),
                    failed_at: None
                },
                long_ago,
                false,
                NOW,
                0
            ),
            None,
            "a stepped-back clock cannot arm through a zero threshold"
        );
    }

    #[test]
    fn the_interaction_edge_is_the_freshest_of_the_three_roads() {
        // THE FRESHEST WINS, whichever road it is. The stalest would arm the
        // unread lamp about news the operator already saw through the road they
        // were actually using.
        assert_eq!(
            last_interaction(Some(100), Some(9_500), Some(9_000), NOW),
            Some(NOW - 100),
            "the desk's idle age counts back from now, and here it is freshest"
        );
        assert_eq!(
            last_interaction(Some(2_000), Some(9_500), Some(9_600), NOW),
            Some(9_600),
            "and here the phone marker is"
        );
        assert_eq!(
            last_interaction(None, Some(9_500), None, NOW),
            Some(9_500),
            "one readable road is enough"
        );
        assert_eq!(
            last_interaction(None, None, None, NOW),
            None,
            "and no road at all proves nothing, so the lamp stays dark"
        );
        assert_eq!(
            last_interaction(Some(NOW + 5_000), None, None, NOW),
            Some(0),
            "an idle age longer than the clock is an interaction at the epoch, \
             never a wrapped one in the far future"
        );
    }

    // --- the loop lamp ------------------------------------------------------

    const THRESHOLD: u64 = 360;
    const LEASE_TIMEOUT: u64 = 3_900;

    /// One reading, with everything not under test set to nothing happening.
    fn running<'reading>(
        streak: Option<&'reading Streak>,
        agents_working: bool,
        leases: &'reading [u64],
    ) -> Loop<'reading> {
        Loop {
            streak,
            agents_working,
            shell_since: None,
            leases,
            now: NOW,
            threshold_secs: THRESHOLD,
            lease_timeout_secs: LEASE_TIMEOUT,
        }
    }

    #[test]
    fn a_shell_command_is_measured_from_its_own_start_and_not_from_an_agents_streak() {
        // TWO SOURCES, TWO CLOCKS, and one shared streak could not serve both.
        // The shell publishes the second its command STARTED, which is an exact
        // start nothing has to infer; an agent gives a status word and nothing
        // else, so its run is timed from the first tick that read it working.
        //
        // POOLED, THEY BORROWED EACH OTHER'S TIME IN BOTH DIRECTIONS. The
        // streak outlives the work by the grace that covers an agent's turn
        // gap, so a fresh five-second command starting inside that grace
        // inherited the streak and armed the lamp at once; and a build that had
        // already been running for ten minutes when the streak was empty was
        // clocked from now and had to wait out the whole threshold again.
        let stale = Streak {
            since: NOW - 5_000,
            last_seen: NOW - 60,
        };
        assert!(
            !loop_running(&Loop {
                shell_since: Some(NOW - 5),
                ..running(Some(&stale), false, &[])
            }),
            "a five-second command cannot inherit an agent's finished run"
        );
        assert!(
            loop_running(&Loop {
                shell_since: Some(NOW - THRESHOLD),
                ..running(None, false, &[])
            }),
            "and a build already past the threshold arms from its OWN start, \
             with no streak behind it and nothing to wait out again"
        );
        assert!(
            !loop_running(&Loop {
                shell_since: Some(NOW - THRESHOLD + 1),
                ..running(None, false, &[])
            }),
            "one second under it is not a loop yet: the same closed edge"
        );
        // AND THE AGENT'S OWN RUN IS NOT DESTROYED BY A FRESH COMMAND, which is
        // the mirror of the first case and the reason this is two readings
        // rather than one taken over the earlier of them.
        let long = streak_from(THRESHOLD);
        assert!(
            loop_running(&Loop {
                shell_since: Some(NOW),
                ..running(Some(&long), true, &[])
            }),
            "an agent ten minutes in keeps its lamp when somebody runs `ls`"
        );
        // A CLOCK BEHIND THE COMMAND HAS NO ELAPSED TIME IN IT.
        assert!(
            !loop_running(&Loop {
                shell_since: Some(NOW + 500),
                ..running(None, false, &[])
            }),
            "a now before the command started has no elapsed time in it"
        );
    }

    #[test]
    fn work_past_the_threshold_arms_the_loop_lamp_and_both_edges_are_closed() {
        let under = streak_from(THRESHOLD - 1);
        let at = streak_from(THRESHOLD);
        assert!(
            !loop_running(&running(Some(&under), true, &[])),
            "one second under the threshold is not a loop yet"
        );
        assert!(
            loop_running(&running(Some(&at), true, &[])),
            "exactly at it, it arms"
        );
        assert!(
            !loop_running(&running(None, true, &[])),
            "work with no streak behind it has no duration to measure"
        );
        // BOTH HALVES, which is the condition as written: something is working
        // AND the run is old enough. The streak deliberately OUTLIVES the work
        // by the grace that covers the gap between a loop's turns, so a reading
        // of the streak alone keeps claiming work in progress for minutes after
        // everything went idle.
        assert!(
            !loop_running(&running(Some(&at), false, &[])),
            "a streak still inside its grace is not work that is still running"
        );
        // A CLOCK BEHIND THE STREAK IS NOT A LONG RUN.
        let future = Streak {
            since: NOW + 500,
            last_seen: NOW + 500,
        };
        assert!(
            !loop_running(&running(Some(&future), true, &[])),
            "a now before the streak began has no elapsed time in it"
        );
    }

    #[test]
    fn a_live_lease_arms_the_loop_lamp_with_nothing_working_and_an_expired_one_does_not() {
        let idle = streak_from(0);
        assert!(
            loop_running(&running(None, false, &[NOW - LEASE_TIMEOUT])),
            "exactly at the timeout is still live: both edges closed"
        );
        assert!(
            !loop_running(&running(None, false, &[NOW - LEASE_TIMEOUT - 1])),
            "one second past it, an abandoned lease can no longer hold the lamp"
        );
        assert!(
            loop_running(&running(Some(&idle), false, &[NOW - 5_000, NOW])),
            "one live lease among expired ones is enough, and it needs no work behind it"
        );
        assert!(
            !loop_running(&running(None, false, &[])),
            "and no lease at all with nothing working is a dark lamp"
        );
    }

    // --- the blocked lamp ---------------------------------------------------

    #[test]
    fn a_live_wait_holds_the_blocked_lamp_and_an_abandoned_one_stops_holding_it() {
        const BOUND: u64 = 1_800;
        assert!(
            any_blocked(&[NOW - 5_000, NOW - 400], NOW, BOUND),
            "one live marker among expired ones is a wait"
        );
        assert!(
            any_blocked(&[NOW - BOUND], NOW, BOUND),
            "exactly at the bound is still live: both edges closed"
        );
        assert!(
            !any_blocked(&[NOW - BOUND - 1], NOW, BOUND),
            "one second past it, an abandoned session can no longer hold a lamp blue"
        );
        assert!(!any_blocked(&[], NOW, BOUND), "no marker is no wait");
        // A MARKER FROM THE FUTURE IS LIVE. A clock that stepped backwards is
        // not a wait that ended, and the saturating subtraction reads it as
        // zero seconds old rather than as an age that would delete it.
        assert!(any_blocked(&[NOW + 500], NOW, BOUND));
    }

    // --- the loop lease -----------------------------------------------------

    #[test]
    fn a_lease_is_keyed_to_the_pane_it_was_typed_in_and_refused_when_there_is_none() {
        assert_eq!(
            loop_command("begin", &[], Some("wW:p21")),
            Ok(LoopCommand::Begin("wW:p21".to_string())),
            "the ordinary case takes the pane out of the environment and needs no \
             argument at all"
        );
        assert_eq!(
            loop_command("end", &[], Some("wW:p21")),
            Ok(LoopCommand::End("wW:p21".to_string())),
        );
        assert_eq!(
            loop_command(
                "begin",
                &["--pane".to_string(), "wW:p9".to_string()],
                Some("wW:p21")
            ),
            Ok(LoopCommand::Begin("wW:p9".to_string())),
            "and an explicit pane beats the environment, which is how a lease is \
             taken for a pane other than this one"
        );
        // REFUSED, NEVER GUESSED. A lease keyed to a pane whose ordinary traffic
        // will never renew it breathes for the whole timeout with nothing behind
        // it, which is the opposite of a liveness signal.
        for absent in [None, Some("")] {
            assert_eq!(
                loop_command("begin", &[], absent),
                Err(
                    "pns: loop: no HERDR_PANE_ID in this environment, so there is no \
                     pane to key the lease to; run it inside the pane, or name one \
                     with --pane"
                        .to_string()
                ),
                "env pane {absent:?}"
            );
        }
    }

    #[test]
    fn a_pane_that_cannot_name_a_file_and_an_argument_this_does_not_know_are_refused() {
        assert_eq!(
            loop_command("begin", &["--pane".to_string(), "../x".to_string()], None),
            Err("pns: loop: \"../x\" is not a pane id this can key a lease to".to_string()),
            "the path-escape guard, through the predicate that backs the filename"
        );
        for arguments in [
            vec!["--pain".to_string(), "wW:p9".to_string()],
            vec!["wW:p9".to_string()],
            vec![],
        ] {
            let refused = if arguments.is_empty() {
                loop_command("resume", &arguments, Some("wW:p21"))
            } else {
                loop_command("begin", &arguments, Some("wW:p21"))
            };
            assert_eq!(
                refused,
                Err(LOOP_USAGE.to_string()),
                "arguments: {arguments:?}"
            );
        }
    }

    #[test]
    fn a_pane_id_that_cannot_be_a_filename_names_no_lease_at_all() {
        let state = std::path::Path::new("/state");
        assert_eq!(
            lease_marker(state, "wW:p21"),
            Some(state.join("lights-loop").join("wW:p21")),
            "herdr's own id names a file inside the lease directory, colon and all"
        );
        for refused in ["..", "../etc/passwd", "a/b", "", "a b"] {
            assert_eq!(
                lease_marker(state, refused),
                None,
                "{refused:?} must name no lease"
            );
        }
    }

    // --- per-lamp arbitration -----------------------------------------------

    /// Every held state at once, which is what makes the ranking observable.
    const ALL_HELD: House = House {
        blocked: true,
        looping: true,
        unread: Some(Unread::Failure),
    };

    fn shows(behaviours: &[Behaviour]) -> Vec<Behaviour> {
        behaviours.to_vec()
    }

    #[test]
    fn every_held_state_is_active_at_once_and_they_rank_blocked_loop_then_unread() {
        assert_eq!(
            active_held(&ALL_HELD),
            vec![Held::Blocked, Held::Looping, Held::UnreadFailure],
            "the house holds all of them at once, most urgent first"
        );
        assert_eq!(
            active_held(&House {
                unread: Some(Unread::Success),
                ..ALL_HELD
            }),
            vec![Held::Blocked, Held::Looping, Held::UnreadSuccess],
            "and the unread flavour is the one the arming answered"
        );
        assert_eq!(
            active_held(&House::default()),
            Vec::new(),
            "a house holding nothing is a dark house"
        );
    }

    #[test]
    fn one_lamp_shows_the_most_urgent_state_it_is_routed_for_and_nothing_it_is_not() {
        let active = active_held(&ALL_HELD);
        assert_eq!(
            shown(&active, &shows(&[Behaviour::Blocked, Behaviour::Unread])),
            Some(Held::Blocked),
            "a lamp routed for both shows the more urgent"
        );
        assert_eq!(
            shown(&active, &shows(&[Behaviour::Unread])),
            Some(Held::UnreadFailure),
            "a lamp routed for only the calmer one shows that, which is how one \
             house state reaches two lamps saying different things"
        );
        assert_eq!(
            shown(&active, &shows(&[Behaviour::Done, Behaviour::Failed])),
            None,
            "a pulse-only lamp holds no state at all"
        );
        assert_eq!(
            shown(&[], &shows(&[Behaviour::Blocked])),
            None,
            "and a routed lamp with nothing active is dark"
        );
    }

    #[test]
    fn a_pulse_fires_on_a_lamp_it_is_routed_for_unless_a_held_state_has_that_lamp() {
        const FREE: bool = false;
        const HELD: bool = true;
        assert!(
            pulse_fires(
                &shows(&[Behaviour::Done, Behaviour::Failed]),
                Behaviour::Done,
                FREE
            ),
            "a routed lamp with no state on it flashes"
        );
        assert!(
            !pulse_fires(&shows(&[Behaviour::Done]), Behaviour::Failed, FREE),
            "and a lamp routed for one pulse does not carry the other"
        );
        // THE DEDICATED LAMP, which is the operator's "it helps out when free"
        // ruling generalised: it joins the pulse lamps whenever no held state
        // has it, and stops the moment one does.
        assert!(
            !pulse_fires(
                &shows(&[Behaviour::Done, Behaviour::Blocked]),
                Behaviour::Done,
                HELD
            ),
            "a held state preempts the pulse on the lamp that is holding it"
        );
        assert!(
            !pulse_fires(&shows(&[Behaviour::Blocked]), Behaviour::Done, FREE),
            "and a lamp that is not routed for the pulse never flashes, held or free"
        );
    }

    // --- the breath driver --------------------------------------------------

    /// A whole twelve-second interval, in the milliseconds the driver budgets
    /// in: the shipped refresh with nothing yet spent resolving the map.
    const FULL_INTERVAL_MS: u64 = 12_000;

    /// The locked blocked shape: two-second fades between 100 and 30.
    const BLOCKED: crate::config::Breath = crate::config::Breath {
        duration_ms: 2000,
        high: 100,
        low: 30,
    };

    /// The locked unread and loop shape: four-second fades between 60 and 10.
    const SLOW: crate::config::Breath = crate::config::Breath {
        duration_ms: 4000,
        high: 60,
        low: 10,
    };

    #[test]
    fn a_zero_resume_reproduces_the_original_breath_with_one_more_fade_added() {
        // THE ORIGINAL SIX-FADE VECTOR, PRESERVED AS A PREFIX. The seamless
        // schedule does not restart the breath, it simply keeps issuing into
        // the slack the old, stop-at-the-peak schedule left unused.
        let fades = breath_fades(FULL_INTERVAL_MS, &BLOCKED, Resume::default());
        assert_eq!(
            fades,
            vec![
                Fade {
                    brightness: 30,
                    start_ms: 0
                },
                Fade {
                    brightness: 100,
                    start_ms: 1_950
                },
                Fade {
                    brightness: 30,
                    start_ms: 3_900
                },
                Fade {
                    brightness: 100,
                    start_ms: 5_850
                },
                Fade {
                    brightness: 30,
                    start_ms: 7_800
                },
                Fade {
                    brightness: 100,
                    start_ms: 9_750
                },
                Fade {
                    brightness: 30,
                    start_ms: 11_700
                },
            ],
            "three full cycles of the locked blocked shape, plus the seventh \
             fade the seamless schedule now fits into a twelve-second interval"
        );
    }

    #[test]
    fn each_fade_leads_the_one_before_it_so_the_lamp_never_pauses_at_an_end() {
        let fades = breath_fades(FULL_INTERVAL_MS, &BLOCKED, Resume::default());
        for pair in fades.windows(2) {
            assert_eq!(
                pair[1].start_ms - pair[0].start_ms,
                BLOCKED.duration_ms - FADE_LEAD_MS,
                "the next fade is issued FADE_LEAD_MS before the previous one ends"
            );
        }
    }

    #[test]
    fn every_last_fade_is_issued_inside_the_budget_and_lands_after_it() {
        // THE LAW, NOT A COINCIDENCE (verified at every quarter-second budget
        // the config's own bounds ever hand this function): the slack before
        // the last issue always sits in (0, step], and that fade's own
        // duration always carries the lamp past the budget it was issued
        // in. A schedule that instead FIT the last fade's whole duration
        // inside the budget (the old, stop-at-the-peak shape, and a
        // completion-fitted rewrite of this one) fails the second assertion
        // at every budget, and the old EVEN-rounded count fails the first at
        // 11_500ms.
        for breath in [BLOCKED, SLOW] {
            let step_ms = breath.duration_ms - FADE_LEAD_MS;
            let mut budget_ms = 8_000;
            while budget_ms <= 12_000 {
                let fades = breath_fades(budget_ms, &breath, Resume::default());
                let last = fades.last().expect("8s or more is never empty");
                let slack = budget_ms - last.start_ms;
                assert!(
                    slack > 0 && slack <= step_ms,
                    "{}ms fades at budget {budget_ms}ms: slack {slack}ms is outside \
                     (0, {step_ms}]",
                    breath.duration_ms
                );
                assert!(
                    last.start_ms + breath.duration_ms > budget_ms,
                    "{}ms fades at budget {budget_ms}ms: the last fade must still be \
                     running when the budget ends, and it ends at {}ms",
                    breath.duration_ms,
                    last.start_ms + breath.duration_ms
                );
                budget_ms += 250;
            }
        }
    }

    #[test]
    fn a_resumed_breath_moves_toward_low_from_high_and_toward_high_from_low() {
        let from_the_peak = breath_fades(
            FULL_INTERVAL_MS,
            &BLOCKED,
            Resume {
                first_due_ms: 0,
                from_high: true,
            },
        );
        assert_eq!(
            from_the_peak.first().map(|fade| fade.brightness),
            Some(BLOCKED.low),
            "a lamp resuming from the high end moves down first"
        );
        let from_the_floor = breath_fades(
            FULL_INTERVAL_MS,
            &BLOCKED,
            Resume {
                first_due_ms: 0,
                from_high: false,
            },
        );
        assert_eq!(
            from_the_floor.first().map(|fade| fade.brightness),
            Some(BLOCKED.high),
            "and vice versa"
        );
    }

    #[test]
    fn a_resumes_first_due_ms_shifts_every_fades_start_by_the_same_amount() {
        let shifted = breath_fades(
            FULL_INTERVAL_MS,
            &BLOCKED,
            Resume {
                first_due_ms: 500,
                from_high: true,
            },
        );
        let unshifted = breath_fades(FULL_INTERVAL_MS - 500, &BLOCKED, Resume::default());
        let shifted_starts: Vec<u64> = shifted.iter().map(|fade| fade.start_ms - 500).collect();
        let unshifted_starts: Vec<u64> = unshifted.iter().map(|fade| fade.start_ms).collect();
        assert_eq!(
            shifted_starts, unshifted_starts,
            "a resume due 500ms late issues the same schedule 500ms later, against \
             a budget 500ms shorter"
        );
    }

    #[test]
    fn a_budget_that_cannot_fit_even_one_fade_is_empty() {
        assert!(breath_fades(0, &BLOCKED, Resume::default()).is_empty());
        assert!(
            breath_fades(
                1_000,
                &BLOCKED,
                Resume {
                    first_due_ms: 1_000,
                    from_high: true
                }
            )
            .is_empty(),
            "a resume due at or past the budget has nowhere left to fade"
        );
    }

    #[test]
    fn the_dim_form_is_the_same_cadence_at_the_faintest_levels_the_hardware_has() {
        // THE DIM SHAPE IS NOT A SPECIAL CASE. It is the same driver over
        // different numbers, which is what makes "dimmed" one more shape rather
        // than a second code path that can drift.
        let dim = crate::config::Breath {
            duration_ms: 3000,
            high: 7,
            low: 1,
        };
        let fades = breath_fades(FULL_INTERVAL_MS, &dim, Resume::default());
        assert_eq!(
            fades,
            vec![
                Fade {
                    brightness: 1,
                    start_ms: 0
                },
                Fade {
                    brightness: 7,
                    start_ms: 2_950
                },
                Fade {
                    brightness: 1,
                    start_ms: 5_900
                },
                Fade {
                    brightness: 7,
                    start_ms: 8_850
                },
                Fade {
                    brightness: 1,
                    start_ms: 11_800
                },
            ],
        );
    }

    // --- the held record's phase --------------------------------------------

    #[test]
    fn a_held_entrys_phase_round_trips_through_its_rendered_token() {
        let high = HeldEntry {
            path: "light/l1".to_string(),
            resume: Some((1_234_567, End::High)),
        };
        assert_eq!(render_held_token(&high), "light/l1@1234567:h");
        assert_eq!(parse_held_token("light/l1@1234567:h"), high);

        let low = HeldEntry {
            path: "light/l1".to_string(),
            resume: Some((1_234_567, End::Low)),
        };
        assert_eq!(render_held_token(&low), "light/l1@1234567:l");
        assert_eq!(parse_held_token("light/l1@1234567:l"), low);
    }

    #[test]
    fn a_bare_token_reads_as_no_phase_and_a_malformed_one_falls_back_to_bare() {
        assert_eq!(
            parse_held_token("light/l1"),
            HeldEntry::bare("light/l1"),
            "a token with no `@` at all is a lamp with no phase recorded"
        );
        for malformed in [
            "light/l1@notanumber:h",
            "light/l1@1234567:sideways",
            "light/l1@1234567",
            "light/l1@",
        ] {
            assert_eq!(
                parse_held_token(malformed),
                HeldEntry::bare("light/l1"),
                "{malformed} is unreadable, never unparseable: it reads as no phase"
            );
        }
    }

    #[test]
    fn resuming_off_no_entry_or_no_phase_starts_the_breath_fresh() {
        assert_eq!(resume_from(None, 1_000), Resume::default());
        assert_eq!(
            resume_from(Some(&HeldEntry::bare("light/l1")), 1_000),
            Resume::default(),
            "a bare entry is a lamp this record holds with no phase recorded"
        );
    }

    #[test]
    fn resuming_off_a_recorded_phase_shifts_the_next_fade_and_flips_its_direction() {
        let held = HeldEntry {
            path: "light/l1".to_string(),
            resume: Some((13_700, End::Low)),
        };
        assert_eq!(
            resume_from(Some(&held), 12_400),
            Resume {
                first_due_ms: 1_250,
                from_high: false
            },
            "due FADE_LEAD_MS before the recorded end, moving away from the end it \
             landed on"
        );
        // A `now_ms` past the recorded end saturates at zero rather than going
        // negative: due at once, not due in the past.
        assert_eq!(
            resume_from(Some(&held), 20_000),
            Resume {
                first_due_ms: 0,
                from_high: false
            }
        );
    }

    #[test]
    fn a_blocked_event_starts_a_wait_and_every_other_event_ends_one() {
        for waiting in crate::pulse::LAMP_BLOCKED {
            assert_eq!(
                blocked_marker_action(waiting),
                Action::Start,
                "{waiting} is an agent waiting on the operator"
            );
        }
        for ended in ["done", "failed", "stale", "", "anything-else"] {
            assert_eq!(
                blocked_marker_action(ended),
                Action::End,
                "{ended} is a later event from that session, so the wait is over"
            );
        }
    }

    #[test]
    fn a_session_id_that_cannot_be_a_filename_names_no_marker_at_all() {
        let state = std::path::Path::new("/state");
        assert_eq!(
            blocked_marker(state, "sess-123"),
            Some(state.join("lights-blocked").join("sess-123")),
            "an ordinary id names a file inside the needs directory"
        );
        // THE PATH-ESCAPE GUARD, through the predicate that already backs
        // `session-<id>.start` in this same directory rather than a second one.
        for refused in ["..", "../etc/passwd", "a/b", "", "a:b", "a b"] {
            assert_eq!(
                blocked_marker(state, refused),
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
                quiet_command(&typed_at("3F - Studio", typed), &known, ONE_HOUR),
                Err(format!(
                    "pns: quiet duration {typed:?} is outside 1s to 24h"
                )),
                "typed: {typed:?}"
            );
        }
        for typed in ["30", "", "1d", " 5m"] {
            assert_eq!(
                quiet_command(&typed_at("3F - Studio", typed), &known, ONE_HOUR),
                Err(format!(
                    "pns: quiet duration {typed:?} is not <count><s|m|h>"
                )),
                "typed: {typed:?}"
            );
        }
        assert_eq!(
            quiet_command(&typed_at("3F - Studio", "30m"), &known, ONE_HOUR),
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
            quiet_command(&typed_at("3F - Nowhere", "30m"), &known, ONE_HOUR),
            Err(
                "pns: lights quiet: \"3F - Nowhere\" is no lamp, room or zone \
                 this can quiet; a mute reaches \"3F - Studio\", \
                 \"3F - Studio - HCL3\""
                    .to_string()
            ),
            "a place nothing in the config names"
        );
        assert_eq!(
            quiet_command(&typed_at("3f - studio", "30m"), &known, ONE_HOUR),
            Err(
                "pns: lights quiet: \"3f - studio\" is no lamp, room or zone \
                 this can quiet; a mute reaches \"3F - Studio\", \
                 \"3F - Studio - HCL3\""
                    .to_string()
            ),
            "and a case-folded one is a typo rather than a name to forgive, \
             which is how the bridge listing reads it too"
        );
        assert_eq!(
            quiet_command(&typed_at("3F - Studio - HCL3", "30m"), &known, ONE_HOUR),
            Ok(QuietCommand::Mute {
                place: "3F - Studio - HCL3".to_string(),
                seconds: 1_800,
            }),
            "the control: a lamp the config names is stored"
        );
        assert_eq!(
            quiet_command(&typed_at("3F - Nowhere", "off"), &known, ONE_HOUR),
            Ok(QuietCommand::Unmute {
                place: "3F - Nowhere".to_string(),
            }),
            "and `off` is allowed over any name, because it can only remove: a \
             place muted yesterday and dropped from the config today would \
             otherwise be a mute nothing could clear"
        );
        assert_eq!(
            quiet_command(&[], &known, ONE_HOUR),
            Ok(QuietCommand::Report),
            "no argument reports and mutes nothing"
        );
        assert_eq!(
            quiet_command(
                &typed_at("3F - Studio - HCL1", "30m"),
                &places(&[]),
                ONE_HOUR
            ),
            Err(
                "pns: lights quiet: \"3F - Studio - HCL1\" is no lamp, room or zone \
                 this can quiet; this config claims no lamp at all, so there is \
                 nothing a mute could reach"
                    .to_string()
            ),
            "and a config that claims nothing says so rather than trailing off \
             after `a mute reaches`"
        );
        let arguments = vec![
            "3F - Studio".to_string(),
            "30m".to_string(),
            "x".to_string(),
        ];
        assert_eq!(
            quiet_command(&arguments, &known, ONE_HOUR),
            Err(
                "pns: lights quiet takes a place, optionally with a duration \
                 or off, or nothing at all"
                    .to_string()
            ),
            "arguments: {arguments:?}"
        );
    }

    /// A schedule an hour away, which is what a bare mute reads.
    const ONE_HOUR: Option<u64> = Some(3_600);

    #[test]
    fn a_bare_mute_lasts_until_the_operators_quiet_hours_end() {
        let known = places(&["3F - Studio"]);
        assert_eq!(
            quiet_command(&[places(&["3F - Studio"])[0].clone()], &known, ONE_HOUR),
            Ok(QuietCommand::Mute {
                place: "3F - Studio".to_string(),
                seconds: 3_600,
            }),
            "no duration typed: the schedule says how long"
        );
        // NO SCHEDULE IS A REFUSAL, never a guessed length: picking one would be
        // a mute the operator did not ask for, ending at an hour they cannot
        // predict.
        assert_eq!(
            quiet_command(&places(&["3F - Studio"]), &known, None),
            Err(
                "pns: lights quiet: a bare mute lasts until your quiet hours end, \
                 and `[plugins.hue] quiet_hours` states none; give a duration \
                 instead, or set that key"
                    .to_string()
            ),
        );
        // AND AN UNKNOWN PLACE IS STILL REFUSED BY NAME on the bare form, which
        // is the same order the two-word form checks in: a typo must not become
        // a mute nothing will ever match.
        assert_eq!(
            quiet_command(&places(&["3F - Nowhere"]), &known, ONE_HOUR),
            Err(unmutable_sentence("3F - Nowhere", &known)),
        );
    }

    #[test]
    fn how_long_a_bare_mute_runs_is_the_minutes_from_now_to_the_windows_end() {
        // 22:00 to 07:00, which is the window every room in the operator's own
        // config carries.
        const ENDS_AT_0700: Option<u16> = Some(7 * 60);
        assert_eq!(
            bare_mute_secs(ENDS_AT_0700, Some(23 * 60)),
            Some(8 * 3_600),
            "typed at 23:00, the mute runs to 07:00: eight hours over midnight"
        );
        assert_eq!(
            bare_mute_secs(ENDS_AT_0700, Some(6 * 60)),
            Some(3_600),
            "and typed at 06:00 it runs one hour, which is the rest of the window"
        );
        assert_eq!(
            bare_mute_secs(ENDS_AT_0700, Some(15 * 60)),
            Some(16 * 3_600),
            "typed outside the window it still runs to the next end, which is what \
             `until my quiet hours end` says"
        );
        // NOW AT THE END MINUTE IS A WHOLE DAY, not nothing: the window ends
        // this second, so the next end is tomorrow's, and a mute of zero seconds
        // is not a mute.
        assert_eq!(bare_mute_secs(ENDS_AT_0700, Some(7 * 60)), Some(24 * 3_600));
        assert_eq!(
            bare_mute_secs(None, Some(23 * 60)),
            None,
            "no schedule is no bare mute"
        );
        assert_eq!(
            bare_mute_secs(ENDS_AT_0700, None),
            None,
            "and neither is a clock this run cannot read"
        );
        // IT NEVER EXCEEDS THE DURATION CAP the typed form is held to, which is
        // what keeps one command from having two sets of bounds.
        assert!(bare_mute_secs(ENDS_AT_0700, Some(7 * 60 + 1)) <= Some(24 * 3_600));
    }

    /// The refusal `quiet_command` gives for a place nothing names, so a test
    /// asserting it does not restate the sentence.
    fn unmutable_sentence(place: &str, known: &[String]) -> String {
        match quiet_command(&places(&[place]), known, Some(1)) {
            Err(said) => said,
            other => panic!("expected a refusal, got {other:?}"),
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
