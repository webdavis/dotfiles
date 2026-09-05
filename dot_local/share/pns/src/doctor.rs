//! `pns doctor`: what one test send through every configured channel found.
//!
//! POLICY ONLY, and every function here is a total function of its arguments:
//! no config, no clock, no environment, no network, no printing. The binary
//! reads the world, sends through the engine's own wiring, and hands what came
//! back to these to shape.
//!
//! THE CENSUS IS THE WHOLE ROSTER, never the selection. A plugin the config
//! left off has to be visibly absent BY CHOICE, or the report answers "what is
//! on" when the operator asked "what will reach me", which is the narrower
//! predicate this project keeps re-finding.

use crate::registry::{PluginKind, Registration, Selection};

/// One registered plugin and what checking it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Check {
    /// The plugin's config-table name, which is also how its line is labelled.
    pub plugin: &'static str,
    pub kind: CheckKind,
}

/// What a check does, decided from the registration and the selection alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// One test event through this channel's own delivery path.
    Send,
    /// A signal to the lights, which no event dispatches: counted in rooms,
    /// because the bridge acknowledges no write.
    Pulse,
    /// A reading rather than a send: what the room sensor currently says.
    Presence,
    /// Nothing to check, and why.
    Skipped(&'static str),
}

/// What loading the config found, as far as the census is concerned.
///
/// IT DECIDES ONE SENTENCE: what a registered plugin the selection left out is
/// reported with. The three states are three different edits, and one wording
/// covering them sends two thirds of the operators to the wrong one. "Not
/// enabled in the config" used to be the only one, which was harmless while a
/// machine with no config ran the whole roster and nothing was ever skipped on
/// it; the core fallback made that sentence the ORDINARY report on a fresh
/// machine, pointing the operator at a file that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigState {
    /// A config was read, so a plugin outside the selection is one it did not
    /// switch on.
    Read,
    /// There is no config file, so the core is all that runs.
    Absent,
    /// A config file exists and could not be read, so the core is all that
    /// runs. It is told apart from `Absent` because one is fixed by writing a
    /// file and the other by repairing one.
    Unreadable,
}

/// Why a registered plugin was not checked: the config never switched it on.
const NOT_ENABLED: &str = "not enabled in the config";

/// Why it was not checked on a machine that has no config at all.
const NO_CONFIG: &str = "no config file, so only the core runs";

/// And why on one whose config could not be read.
const UNREADABLE_CONFIG: &str = "the config could not be read, so only the core runs";

/// Why a selected plugin was not checked: it is an input, and no leg can reach
/// it whatever the config says.
const A_SENSOR: &str = "a sensor and never a delivery destination";

/// What one check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It arrived, and the channel said this about it.
    Sent(String),
    /// It arrived, and the channel had nothing to say. An executable channel
    /// is silent by design, so claiming success for it would be claiming what
    /// the code does not provide.
    SentUnreported,
    /// It did not arrive, and the channel said this about that.
    Failed(String),
    /// The lights, and how many rooms were signalled.
    Signalled(usize),
    /// What the room sensor reads right now, in every state it can be in, and
    /// the phrase the narrowing ring last recorded (`None` for a ring with
    /// nothing in it).
    Presence(crate::presence::PresenceStatus, Option<String>),
    /// Nothing was checked, and why.
    Skipped(&'static str),
}

/// One check per registration, in registration order, whatever the config
/// selected.
pub fn checks(registered: &Selection, selected: &Selection, config: ConfigState) -> Vec<Check> {
    registered
        .iter()
        .map(|entry| Check {
            plugin: entry.name,
            kind: kind_of(entry, selected, config),
        })
        .collect()
}

/// Why a plugin outside the selection was left out, in the words that are true
/// of THIS machine.
fn not_selected(config: ConfigState) -> &'static str {
    match config {
        ConfigState::Read => NOT_ENABLED,
        ConfigState::Absent => NO_CONFIG,
        ConfigState::Unreadable => UNREADABLE_CONFIG,
    }
}

/// What checking one registration means, given what the config selected.
///
/// NOT SELECTED IS ASKED FIRST, so a sensor the config never switched on reads
/// as absent by choice rather than as the kind it would have been.
fn kind_of(entry: &Registration, selected: &Selection, config: ConfigState) -> CheckKind {
    if !selected.iter().any(|chosen| chosen.name == entry.name) {
        return CheckKind::Skipped(not_selected(config));
    }
    match entry.kind {
        // THE ONE SENSOR WITH SOMETHING TO REPORT. Nothing is sent to it and
        // nothing ever will be, but its reading is the one thing about it an
        // operator cannot see any other way, and a bare `skipped, a sensor`
        // line would leave a machine whose bridge stopped answering looking
        // exactly like one that is fine.
        PluginKind::Sensor if entry.name == crate::registry::PRESENCE => CheckKind::Presence,
        PluginKind::Sensor => CheckKind::Skipped(A_SENSOR),
        // A channel the binary drives in its own mode is checkable, just not
        // as a leg: no event routes to it, so a send would never happen and
        // reporting it as skipped would hide the one destination hardest to
        // verify any other way.
        PluginKind::Channel(routing) if !routing.event_dispatched => CheckKind::Pulse,
        PluginKind::Channel(_) => CheckKind::Send,
    }
}

/// The one line this check earned.
pub fn line(check: &Check, outcome: &Outcome) -> String {
    let plugin = check.plugin;
    match outcome {
        Outcome::Sent(said) => format!("{plugin}: sent, {said}"),
        Outcome::SentUnreported => format!("{plugin}: sent, this channel reports no outcome"),
        Outcome::Failed(said) => format!("{plugin}: FAILED, {said}"),
        // NEITHER CLAIM IS MADE. Zero rooms is a bridge that answered no
        // listing OR a configured name nothing matched, and the line names
        // both rather than picking one; a count above zero says the rooms were
        // addressed and stops there, because the bridge acknowledges no write.
        Outcome::Signalled(0) => format!(
            "{plugin}: FAILED, signalled no rooms \
             (no room listing from the bridge, or no configured room name matched)"
        ),
        Outcome::Signalled(1) => format!("{plugin}: signalled 1 room ({WATCH_FOR_IT})"),
        Outcome::Signalled(rooms) => format!("{plugin}: signalled {rooms} rooms ({WATCH_FOR_IT})"),
        Outcome::Skipped(reason) => format!("{plugin}: skipped, {reason}"),
        Outcome::Presence(status, last_narrowing) => {
            presence_said(plugin, status, last_narrowing.as_deref())
        }
    }
}

/// The room sensor's line, in every state the reading can be in.
///
/// UNKNOWN NAMES WHICH KIND OF UNKNOWN, because the five are five different
/// things to go and fix: nothing published yet, a daemon or a bridge that
/// stopped, a clock, a wrong epoch, and a room the config does not watch.
fn presence_said(
    plugin: &str,
    status: &crate::presence::PresenceStatus,
    last_narrowing: Option<&str>,
) -> String {
    use crate::presence::PresenceStatus;
    let reading = match status {
        PresenceStatus::Room { room, age_secs } => {
            format!("{} ({age_secs}s ago)", shown_room(room))
        }
        // THE BRIDGE ANSWERED AND ANSWERED "NOT THERE", which is a different
        // fact from not knowing and is worth its own word.
        PresenceStatus::Nowhere { poll_age_secs } => {
            format!("nowhere (poll {poll_age_secs}s ago)")
        }
        PresenceStatus::Unknown(reason) => {
            format!("unknown ({})", crate::presence::unreadable_said(reason))
        }
    };
    // WHAT THE LAMPS DID WITH IT, which the reading alone does not say: the
    // desk overrules a room, a room holding no lamp falls back, and an
    // operator staring at a lamp in the wrong room needs to see which. It is
    // read back out of a state file, so it crosses the same filter the room
    // name above does.
    match last_narrowing {
        Some(narrowing) => format!(
            "{plugin}: {reading}; last narrowed {}",
            printable(narrowing)
        ),
        None => format!("{plugin}: {reading}"),
    }
}

/// The room name, made safe to put on a terminal.
///
/// THE BRIDGE CHOSE THIS TEXT, exactly as moshi chose the sentence beside it,
/// so it crosses the same filter: an unfiltered newline in a room name forges
/// a second `pns doctor:` line the operator would read as pns's own verdict.
/// A name that filters away to nothing is NAMED AS SUCH rather than printed
/// as a blank, which would read as a room whose name is empty.
fn shown_room(room: &str) -> String {
    let shown = printable(room);
    if shown.trim().is_empty() {
        return "a room whose name will not print".to_string();
    }
    shown
}

/// What the operator has to do to confirm a pulse, since nothing else can.
const WATCH_FOR_IT: &str = "watch for the flash; the bridge acknowledges no write";

/// The last line: how the whole run went.
pub fn summary(outcomes: &[Outcome]) -> String {
    let count = |wanted: Verdict| outcomes.iter().filter(|o| verdict(o) == wanted).count();
    format!(
        "pns doctor: {} sent, {} failed, {} skipped",
        count(Verdict::Sent),
        count(Verdict::Failed),
        count(Verdict::Skipped)
    )
}

/// What the shell learns.
///
/// NOT THE ALWAYS-EXIT-0 CONTRACT'S TERRITORY: that covers the hook and
/// notification paths, where a non-zero exit fails the turn being reported on.
/// This is hand typed and is never a hook.
///
/// THE PAIRING IS AN ARGUMENT RATHER THAN A SECOND CODE THE CALLER COMBINES,
/// which is the same rule the summary and this function already share: two
/// contributors decided at one point cannot disagree, and two decided at two
/// call sites eventually will.
pub fn exit_code(outcomes: &[Outcome], pairing: &PairingReport) -> i32 {
    if outcomes
        .iter()
        .any(|outcome| verdict(outcome) == Verdict::Failed)
    {
        return 1;
    }
    // AN UNPAIRED HOST IS A DEAD APPROVAL PATH, and it is the one pairing
    // state that moves this: the check only reaches it on a machine where
    // moshi-hook is installed and answering, and there an unregistered host
    // means every card is going nowhere while the census reports the mobile
    // channel green over its webhook. The other three states could not check
    // and are inert, so a machine that does not use moshi still exits 0.
    if pairing.pairing == Pairing::Unpaired {
        return 1;
    }
    // A CHECK WITH NOTHING TO CHECK MUST NEVER REPORT GREEN, which is the same
    // ruling the mute took: reporting success for something that is not in
    // effect is the worst outcome available.
    i32::from(
        !outcomes
            .iter()
            .any(|outcome| verdict(outcome) == Verdict::Sent),
    )
}

/// What `moshi-hook status` said about this host, in the only two shapes pns
/// is willing to state: a graded local fact, and moshi's own sentence about
/// the server relayed word for word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingReport {
    pub pairing: Pairing,
    /// moshi's `server:` sentence AS MOSHI WROTE IT, RELAYED AND NEVER
    /// GRADED. Held raw: the printable filter and the relay cap belong to
    /// `pairing_lines`, which is the point the sentence becomes something
    /// printed, and putting them here as well would be two places to disagree
    /// about what is safe to print.
    /// `None` when moshi printed no such line, which is what an unpaired host
    /// prints today and what a moshi that renamed the line would print: both
    /// degrade to no relay and nothing else moves.
    pub server: Option<String>,
}

/// What the LOCAL pairing material says, which is all `status --json` knows.
///
/// `Paired` PROVES LESS THAN IT SOUNDS LIKE, and the line built from it must
/// never read as "approvals work". It says this host has pairing material on
/// disk and that moshi answered about it. It does NOT prove the running daemon
/// is serving that pairing (a re-pair mints a new host id while the live
/// daemon keeps the old one, and no daemon-side evidence is readable from
/// here), and it does not prove an approval will round trip, which needs a
/// human tapping a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pairing {
    /// moshi answered `paired: true`, and named this host these two ways.
    Paired {
        host_id: String,
        display_name: String,
    },
    /// moshi answered `paired: false`.
    Unpaired,
    /// moshi answered something with no `paired` in it.
    Unreadable,
    /// moshi did not answer at all.
    NoAnswer,
}

/// The pairing, read off `status --json`, and the server sentence, read off
/// plain `status`. Either is `None` when that call gave no answer.
pub fn pairing_report(json: Option<&str>, plain: Option<&str>) -> PairingReport {
    PairingReport {
        pairing: pairing_of(json),
        server: plain
            .filter(|plain| within_cap(plain))
            .and_then(server_said),
    }
}

/// Whether an answer is small enough to be looked at.
///
/// THE SPAWN'S CEILING IS NOT THIS CHECK'S CAP, and the room between them is
/// what this function lives in. The reader bounds bytes as well as time now,
/// but it bounds them at `PAIRING_READ_MAX`, which is deliberately wider than
/// `ANSWER_MAX`: an answer between the two ARRIVED and is refused HERE, with
/// "moshi-hook answered something this cannot read", while one past the
/// reader's own ceiling never arrives and is reported as no answer at all.
/// Without this check a moshi-hook that answered at length inside its time
/// window would hand the whole thing to serde on one leg and have every line
/// of it scanned on the other. It is checked BEFORE either, which is the only
/// point where it means anything, and it is checked HERE rather than in the
/// shared bounded spawn: every other caller of that spawn reads a different
/// tool, and one of them is a condenser whose whole job is to answer at length.
fn within_cap(answer: &str) -> bool {
    answer.len() <= ANSWER_MAX
}

/// How much of an answer this check will read. Thousands of times the captured
/// 0.3.3 answer, which is a couple of hundred bytes, and still far short of
/// anything worth parsing by accident.
///
/// THE READER IS ASKED FOR TWICE THIS, at the `run_bounded` calls in the
/// composition root (`PAIRING_READ_MAX`), which is what keeps an over-cap
/// answer DISTINGUISHABLE from one exactly at the cap: a reader that stopped at
/// the cap itself would hand `within_cap` a truncated answer that passes, and
/// the refusal there would never fire again. The doubling rather than a single
/// byte of headroom is deliberate and is argued at `PAIRING_READ_MAX` itself;
/// what matters here is that this constant does NOT set the reader's ceiling,
/// so moving it does not move that ceiling with it.
pub const ANSWER_MAX: usize = 1024 * 1024;

/// What moshi said about the server, taken off the ONE line that begins with
/// the label at column zero.
///
/// NOTHING HERE MATCHES ON THE SENTENCE. pns has no stable way to tell "Moshi
/// Pro attached" from "host does not belong to this user token", and a prefix
/// or substring rule over moshi's prose would fail in the dangerous direction
/// the day the wording changes. The operator reads moshi's own words instead.
fn server_said(plain: &str) -> Option<String> {
    plain
        .lines()
        .find_map(|line| line.strip_prefix(SERVER_LABEL))
        .map(|said| said.trim().to_string())
        .filter(|said| !said.is_empty())
}

/// The pairing check's own lines, in the order they are printed: what pns
/// graded, then what moshi said, when moshi said anything.
pub fn pairing_lines(report: &PairingReport) -> Vec<String> {
    let mut lines = vec![format!(
        "{PREFIX}moshi pairing: {}",
        said_of(&report.pairing)
    )];
    if let Some(said) = report
        .server
        .as_deref()
        .map(printable)
        .filter(|said| !said.is_empty())
    {
        // ATTRIBUTED, because pns is not making this claim and could not
        // check it: the sentence is moshi's and the label says so.
        lines.push(format!("{PREFIX}moshi says: {said}"));
    }
    lines
}

/// Somebody else's text, made safe to put on a terminal, and capped. EVERY
/// string moshi chose goes through this: the relayed server sentence and the
/// two identity fields alike.
///
/// FILTERED AT THE POINT IT BECOMES A LINE, which is the only place that can
/// promise it: the report holds what moshi said, and this is what decides what
/// may be printed.
///
/// THE NEWLINE IS THE LOAD-BEARING ONE. An unfiltered newline would print a
/// second `pns doctor:` line that the operator would read as pns's own
/// verdict, and a report that can be made to lie about itself is worse than no
/// relay at all. The carriage return is the one that survives being split into
/// lines and returns a terminal's cursor to column zero for whatever follows
/// to overwrite the prefix with. Escapes, bells and every other control byte
/// go the same way, and so does anything outside ASCII, which is also what
/// makes the cap safe: a character is dropped whole, so the count can never
/// land inside a multi-byte sequence.
///
/// This does NOT reuse the decision log's identity filter, and the difference
/// is the point: that rule judges a short identity token that becomes a key's
/// value and replaces the whole thing when it fails, while this judges a
/// relayed English sentence full of spaces, parentheses, quotes and colons.
/// One predicate for both would have to be the wider of the two, which is the
/// narrower one weakened.
fn printable(said: &str) -> String {
    said.chars()
        .filter(|character| *character == ' ' || character.is_ascii_graphic())
        .take(RELAY_MAX)
        .collect()
}

/// How much of somebody else's sentence this report is willing to carry. An
/// unbounded relay is an unbounded line in a report pns is responsible for.
const RELAY_MAX: usize = 200;

/// The one sentence each state has earned. EVERY ONE OF THEM IS BOUNDED BY
/// WHAT THIS CHECK CAN SEE: the paired line says who this host is paired as
/// and stops, and the three that could not answer say so rather than reading
/// as a verdict either way.
fn said_of(pairing: &Pairing) -> String {
    match pairing {
        // FILTERED THE SAME WAY THE RELAYED SENTENCE IS, because they are the
        // same kind of thing: strings another program chose, printed on the
        // operator's terminal inside a line pns signs its own name to. An
        // unfiltered newline in a `displayName` forges a `pns doctor:` line
        // exactly as one in the server sentence does.
        Pairing::Paired {
            host_id,
            display_name,
        } => format!(
            "paired as {} ({}).",
            printable(display_name),
            printable(host_id)
        ),
        // THE REMEDY IS IN THE LINE, because this is the state the whole check
        // exists for and it is invisible everywhere else: the census reports
        // the moshi channel green over its webhook the whole time, while every
        // approval card is going nowhere.
        Pairing::Unpaired => "this host is NOT paired, so every approval card is dead \
             until `moshi-hook pair` runs."
            .to_string(),
        Pairing::Unreadable => "moshi-hook answered something this cannot read.".to_string(),
        // BOTH EXPLANATIONS AND NEITHER CLAIM. The bounded spawn cannot tell
        // an absent binary from one that hung or one that exited non-zero, and
        // a machine that simply does not use moshi must not fail its doctor
        // forever, so this costs nothing on the exit code either.
        Pairing::NoAnswer => "moshi-hook did not answer (not installed, or it did not \
             answer in time), so the approval path could not be checked."
            .to_string(),
    }
}

/// How every line the doctor prints for itself is addressed.
const PREFIX: &str = "pns doctor: ";

/// What the doctor found about the lamps.
///
/// SIX STATES AND NO GRADE. This section reports; it never moves the exit
/// code, for the reason the decision section does not: a dark lamp is not a
/// broken notifier, and the exit code is what the operator's automation reads
/// as "notifications are broken".
pub enum LightsReport {
    /// No `[lights]` table: the state every machine was in before it existed.
    Off,
    /// A table, and no `[plugins.hue]` table at all. Told apart from the
    /// switch below because they are different jobs: one config was never
    /// finished, the other was finished and turned off, and sending an
    /// operator to flip a switch that does not exist is a wrong direction
    /// they will act on.
    HueMissing,
    /// A table, with hue's own switch off. ONE SWITCH: hue is the transport
    /// and lights is the policy, so a policy with no transport lights nothing.
    HueDisabled,
    /// A table and an enabled hue, with no bridge and key to dial. Told apart
    /// from the state below for `hue_resolves`' own reason: one is a config to
    /// fix and the other is a network to fix.
    NoBridge,
    /// A bridge that answered no listing at all.
    Unreachable,
    Resolved(crate::channels::hue::Routing),
}

/// The lamps' own lines: how many lamps carry each behaviour, what could not be
/// resolved, and what was refused outright.
///
/// COUNTS AND NAMES ONLY, following the missed journal's structural privacy
/// rule: no colours, no session ids, no detail text.
pub fn lights_lines(report: &LightsReport) -> Vec<String> {
    let routing = match report {
        LightsReport::Off => {
            return vec![format!(
                "{PREFIX}lights: off in the config, so the pulse uses the [plugins.hue] rooms"
            )];
        }
        LightsReport::HueMissing => {
            return vec![format!(
                "{PREFIX}lights: configured, but there is no [plugins.hue] table to \
                 light them through"
            )];
        }
        LightsReport::HueDisabled => {
            return vec![format!(
                "{PREFIX}lights: configured, but [plugins.hue] enabled is false, so nothing lights"
            )];
        }
        LightsReport::NoBridge => {
            return vec![format!(
                "{PREFIX}lights: no [plugins.hue] bridge and key, so no lamp could be resolved"
            )];
        }
        LightsReport::Unreachable => {
            return vec![format!(
                "{PREFIX}lights: the bridge listed nothing, so no lamp resolved"
            )];
        }
        LightsReport::Resolved(routing) => routing,
    };

    // PER BEHAVIOUR RATHER THAN PER LAMP, because the question an operator opens
    // this section with is "did the thing I routed reach a bulb", and a lamp
    // count answers a different one. A behaviour NOTHING carries is listed at
    // zero, because "the word I wrote is missing from the report" is not a state
    // anybody should have to infer from an absence.
    let counted: Vec<String> = crate::config::BEHAVIOUR_WORDS
        .iter()
        .map(|(word, behaviour)| {
            let lamps = routing
                .lamps
                .iter()
                .filter(|routed| routed.shows.contains(behaviour))
                .count();
            format!("{word} {lamps}")
        })
        .collect();
    let mut lines = vec![format!("{PREFIX}lights: {}", counted.join(", "))];
    // THE SENTENCE ITSELF IS THE CHANNEL'S, so the tick reports an unresolved
    // lamp in the same words this does and only the prefix differs.
    for missing in &routing.unresolved {
        lines.push(format!(
            "{PREFIX}{}",
            crate::channels::hue::missing_sentence(missing)
        ));
    }
    for refusal in &routing.refusals {
        lines.push(format!("{PREFIX}{refusal}"));
    }
    lines
}

/// moshi's own label for the one line carrying a server verdict. A LINE
/// PREFIX, never a substring: moshi indents its detail lines, and a substring
/// rule would quote whichever of them said the word first.
const SERVER_LABEL: &str = "server:";

/// The pairing `status --json` described, and NOTHING ELSE OFF THAT OBJECT.
/// Three keys are read; `hooks` in particular is deliberately not one of them.
fn pairing_of(json: Option<&str>) -> Pairing {
    // NO ANSWER IS ITS OWN STATE AND NOTHING GUESSES PAST IT. The bounded
    // spawn answers `None` for a binary that is absent, one that hung past its
    // deadline and one that exited non-zero, and nothing downstream may claim
    // to know which of the three it was.
    let Some(json) = json else {
        return Pairing::NoAnswer;
    };
    // AN ANSWER TOO BIG TO READ IS AN ANSWER THIS CANNOT READ, which is a
    // state this already has a line for. It is NOT no-answer: moshi-hook ran
    // and said something, and the honest report is that pns declined to read
    // it rather than that nothing arrived.
    if !within_cap(json) {
        return Pairing::Unreadable;
    }
    let Ok(answer) = serde_json::from_str::<serde_json::Value>(json) else {
        return Pairing::Unreadable;
    };
    match answer.get(PAIRED).and_then(serde_json::Value::as_bool) {
        Some(true) => Pairing::Paired {
            host_id: named(&answer, "hostId"),
            display_name: named(&answer, "displayName"),
        },
        Some(false) => Pairing::Unpaired,
        // A key that is absent, or holds something other than a bool, is an
        // answer this cannot read. It is NOT read as unpaired: guessing the
        // one state that earns an exit 1 out of a shape nobody recognized is
        // how a doctor starts failing healthy machines.
        None => Pairing::Unreadable,
    }
}

/// One string moshi named, or the honest admission that it named none. The
/// measured 0.3.3 answer always carries both alongside `paired: true`, so this
/// is the shape nobody has seen rather than a case to design around.
fn named(answer: &serde_json::Value, key: &str) -> String {
    answer
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(NOT_REPORTED)
        .to_string()
}

/// The one key that moves the exit code, spelled once.
const PAIRED: &str = "paired";

/// What stands in for an identifier moshi did not name.
const NOT_REPORTED: &str = "not reported";

/// The three buckets every outcome falls into, decided ONCE so the summary's
/// counts and the exit code cannot read the same run differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Sent,
    Failed,
    Skipped,
}

fn verdict(outcome: &Outcome) -> Verdict {
    match outcome {
        Outcome::Sent(_) | Outcome::SentUnreported => Verdict::Sent,
        Outcome::Failed(_) => Verdict::Failed,
        // A PULSE THAT REACHED NO ROOM REACHED NOTHING. It is the shape every
        // hue misconfiguration takes, and an enabled channel that could not be
        // attempted is exactly what the exit contract calls a failure.
        Outcome::Signalled(0) => Verdict::Failed,
        Outcome::Signalled(_) => Verdict::Sent,
        Outcome::Skipped(_) => Verdict::Skipped,
        // A READING IS NEVER A SEND, in any state. Nothing was delivered
        // through the sensor and nothing failed to be, so it counts with the
        // checks that had nothing to send rather than moving the exit code
        // in either direction: a bridge that stopped answering costs the
        // lights their narrowing, and no notification at all.
        Outcome::Presence(..) => Verdict::Skipped,
    }
}

/// The daemon's own line in the doctor's tail run: whether the clock is
/// running, in the five states it can honestly be in.
///
/// FIVE RATHER THAN FOUR, because "off in the config" is two different facts
/// depending on whether a process is still beating, and the operator who just
/// turned the switch off is standing in exactly that state.
///
/// IT NEVER MOVES THE EXIT CODE, in any state, including the dead one. The
/// doctor's code is what an operator's automation reads as "notifications are
/// broken", and a daemon that is down costs ambient features rather than a
/// card. Reporting it as a broken notifier would be the fail-open sin's
/// mirror: a true alarm about the wrong thing, in a place that already means
/// something else. That is why this returns a String and is never an input to
/// `exit_code`.
///
/// IT COUNTS JOBS AND NEVER NAMES THEM, following the missed journal's
/// structural privacy rule: the count answers "is anything scheduled" and the
/// contents are a reader nobody asked for.
pub fn daemon_line(
    enabled: bool,
    beat: Option<crate::daemon::Heartbeat>,
    now: Option<u64>,
    jobs: usize,
) -> String {
    // AN AGE THAT CANNOT BE TAKEN IS NOT A FRESH BEAT. No clock, and a beat
    // stamped after now, both leave nothing to compare, and vouching for a
    // daemon on the strength of a timestamp nothing could grade is the
    // identity-is-not-presence mistake with a file standing in for the process.
    let age = beat.and_then(|beat| now.and_then(|now| now.checked_sub(beat.at)));
    let beating = age.is_some_and(|age| age <= crate::daemon::HEARTBEAT_STALE_SECS);
    if !enabled {
        // THE CONFIG IS NOT THE PROCESS. Nothing bounces the launchd job when
        // the config changes, so a daemon started while the switch was on keeps
        // running after it is turned off, and it keeps firing jobs. Reporting
        // it as simply off would be this line saying the opposite of the truth
        // in the one state an operator turned the switch to reach.
        return match beat {
            Some(beat) if beating => format!(
                "{PREFIX}the daemon is off in the config, but pid {} is still beating; \
                 bootout (or wait) to stop it",
                beat.pid
            ),
            _ => format!("{PREFIX}the daemon is off in the config"),
        };
    }
    let Some(beat) = beat else {
        return format!("{PREFIX}the daemon is enabled and has not run yet");
    };
    match age {
        Some(age) if age <= crate::daemon::HEARTBEAT_STALE_SECS => format!(
            "{PREFIX}the daemon is running, pid {}, {jobs} job{} scheduled",
            beat.pid,
            if jobs == 1 { "" } else { "s" }
        ),
        Some(age) => format!(
            "{PREFIX}the daemon is enabled, its last beat was {age}s ago, so it is not running"
        ),
        None => format!(
            "{PREFIX}the daemon is enabled, its last beat was an unknown time ago, \
             so it is not running"
        ),
    }
}

/// The nag's own line in the doctor's tail run: what the schedule is, said in
/// the unit the card will say it in.
///
/// IT REPORTS THE CONFIG ONLY AND DOES NOT GRADE THE DAEMON. A nag with a dead
/// daemon never fires, which is a true and important thing to say, but
/// `daemon_line` one row above already says the daemon is not running, from the
/// heartbeat, and two lines deriving one fact is how they drift apart. THE
/// PLACEMENT IS THE WHOLE MITIGATION: the two read as one paragraph.
///
/// IT DOES NOT MOVE THE EXIT CODE, for `focus_line`'s reason: a nag being off
/// is not a fault, and the doctor's exit code is what an operator's automation
/// reads as "notifications are broken".
///
/// TWO STATES AND NOT THREE. No table and `after_secs = 0` are the SAME
/// statement in this config, so telling them apart here would be the doctor
/// inventing a distinction the parser does not carry.
pub fn nag_line(after_secs: u64) -> String {
    match after_secs {
        0 => format!("{PREFIX}the nag is off (no `[nag] after_secs`)"),
        seconds => format!(
            "{PREFIX}an unanswered approval is carded again after {}",
            crate::nag::waited(seconds)
        ),
    }
}

#[cfg(test)]
mod nag_tests {
    use super::nag_line;

    #[test]
    fn the_nag_line_names_the_schedule_or_says_the_feature_is_off() {
        assert_eq!(
            nag_line(0),
            "pns doctor: the nag is off (no `[nag] after_secs`)"
        );
        assert_eq!(
            nag_line(300),
            "pns doctor: an unanswered approval is carded again after 5m"
        );
        // THE SAME UNIT THE CARD USES, so "carded again after 30s" and "still
        // waiting 30s" are one operator reading one number twice.
        assert_eq!(
            nag_line(30),
            "pns doctor: an unanswered approval is carded again after 30s"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        A_SENSOR, Check, CheckKind, ConfigState, LightsReport, NO_CONFIG, NOT_ENABLED, Outcome,
        Pairing, PairingReport, UNREADABLE_CONFIG, checks, exit_code, lights_lines, line,
        pairing_lines, pairing_report, summary,
    };
    use crate::channels::hue as pns_hue;
    use crate::config::{Behaviour, parse_config};
    use crate::presence::{PresenceStatus, Unreadable};
    use crate::registry::{Registry, Selection, roster};

    /// The roster's own selection for a config, both halves the census takes.
    fn census(config_text: &str) -> (Registry, Selection, Selection) {
        let registry = roster();
        let selected = registry
            .enabled(&parse_config(config_text).unwrap())
            .unwrap();
        let registered = registry.all();
        (registry, registered, selected)
    }

    fn kind_for(config_text: &str, plugin: &str) -> CheckKind {
        let (_, registered, selected) = census(config_text);
        checks(&registered, &selected, ConfigState::Read)
            .into_iter()
            .find(|check| check.plugin == plugin)
            .unwrap_or_else(|| panic!("{plugin} is registered"))
            .kind
    }

    // --- the room sensor -----------------------------------------------------

    /// `line` for a presence reading, which is the only outcome that check
    /// takes.
    fn presence_line_for(status: PresenceStatus) -> String {
        presence_line_with(status, None)
    }

    /// The same line, with whatever the narrowing ring last recorded.
    fn presence_line_with(status: PresenceStatus, last_narrowing: Option<&str>) -> String {
        line(
            &Check {
                plugin: "presence",
                kind: CheckKind::Presence,
            },
            &Outcome::Presence(status, last_narrowing.map(str::to_string)),
        )
    }

    #[test]
    fn the_room_sensor_line_names_what_the_last_decision_narrowed_the_lamps_to() {
        // The reading alone does not say what the lamps DID with it: the desk
        // overrules a room, an empty room falls back, and an operator staring
        // at a lamp in the wrong room needs to see which of those happened.
        assert_eq!(
            presence_line_with(
                PresenceStatus::Room {
                    room: "2F - Kitchen".to_string(),
                    age_secs: 4,
                },
                Some(r#"nothing (at the desk, and no desk_room says which room that is)"#),
            ),
            "presence: 2F - Kitchen (4s ago); last narrowed nothing (at the desk, \
             and no desk_room says which room that is)"
        );
        // AND A RING WITH NOTHING IN IT SAYS NOTHING, rather than claiming a
        // narrowing never decided: presence off, or on and never yet consulted.
        assert_eq!(
            presence_line_for(PresenceStatus::Nowhere { poll_age_secs: 3 }),
            "presence: nowhere (poll 3s ago)"
        );
    }

    #[test]
    fn the_selected_room_sensor_is_a_reading_rather_than_the_sensor_skip() {
        // The router is the sensor with nothing to report and keeps the skip;
        // this one has a reading, and a bare "a sensor" line would leave a
        // machine whose bridge died looking like one that is fine.
        let config = "[plugins.presence]\nenabled = true\ntype = \"hue\"\n\
                      [plugins.hue]\nenabled = true\n";
        assert_eq!(kind_for(config, "presence"), CheckKind::Presence);
        assert_eq!(kind_for(config, "router"), CheckKind::Skipped(NOT_ENABLED));
    }

    #[test]
    fn a_room_sensor_the_config_never_switched_on_is_still_a_skip() {
        // NOT SELECTED IS ASKED FIRST, or a plugin nobody enabled would print
        // a reading and read as switched on.
        assert_eq!(
            kind_for("[plugins.hermes]\nenabled = true\n", "presence"),
            CheckKind::Skipped(NOT_ENABLED)
        );
    }

    #[test]
    fn a_known_room_is_named_with_the_age_of_its_motion_edge() {
        assert_eq!(
            presence_line_for(PresenceStatus::Room {
                room: "3F - Studio".to_string(),
                age_secs: 4,
            }),
            "presence: 3F - Studio (4s ago)"
        );
    }

    #[test]
    fn a_fresh_poll_that_found_nobody_says_nowhere_rather_than_unknown() {
        assert_eq!(
            presence_line_for(PresenceStatus::Nowhere { poll_age_secs: 3 }),
            "presence: nowhere (poll 3s ago)"
        );
    }

    #[test]
    fn every_way_of_not_knowing_says_which_way_it_is() {
        // FIVE DIFFERENT EDITS: nothing published yet, a daemon or bridge that
        // stopped, a clock, a wrong epoch, and a room nobody watches. One
        // wording for all of them sends four operators in five to the wrong
        // file.
        assert_eq!(
            presence_line_for(PresenceStatus::Unknown(Unreadable::NoReading)),
            "presence: unknown (no reading)"
        );
        assert_eq!(
            presence_line_for(PresenceStatus::Unknown(Unreadable::NoClock)),
            "presence: unknown (the clock could not be read)"
        );
        assert_eq!(
            presence_line_for(PresenceStatus::Unknown(Unreadable::Stale {
                poll_age_secs: 42
            })),
            "presence: unknown (stale, poll 42s old)"
        );
        assert_eq!(
            presence_line_for(PresenceStatus::Unknown(Unreadable::Future)),
            "presence: unknown (future epoch)"
        );
        assert_eq!(
            presence_line_for(PresenceStatus::Unknown(Unreadable::NotWatched)),
            "presence: unknown (the reported room is not one this config watches)"
        );
    }

    #[test]
    fn a_room_name_the_bridge_chose_is_filtered_before_it_reaches_the_terminal() {
        // An unfiltered newline forges a second `pns doctor:` line the operator
        // reads as pns's own verdict, and an escape rewrites the ones above it.
        let said = presence_line_for(PresenceStatus::Room {
            room: "3F\n\u{1b}[2Kpns doctor: all clear".to_string(),
            age_secs: 1,
        });
        assert_eq!(said.lines().count(), 1, "{said}");
        assert!(!said.contains('\u{1b}'), "{said}");
        // AND A NAME THAT FILTERS AWAY TO NOTHING IS NAMED, never printed as a
        // blank that reads as a room with no name.
        assert_eq!(
            presence_line_for(PresenceStatus::Room {
                room: "\u{30ad}\u{30c3}\u{30c1}\u{30f3}".to_string(),
                age_secs: 1,
            }),
            "presence: a room whose name will not print (1s ago)"
        );
    }

    #[test]
    fn a_reading_is_never_counted_as_a_send_however_good_it_is() {
        // Nothing was delivered through a sensor and nothing failed to be, so
        // a green reading must not be what makes `pns doctor` exit 0.
        let outcomes = vec![Outcome::Presence(
            PresenceStatus::Room {
                room: "3F - Studio".to_string(),
                age_secs: 1,
            },
            None,
        )];
        assert_eq!(
            summary(&outcomes),
            "pns doctor: 0 sent, 0 failed, 1 skipped"
        );
        assert_eq!(exit_code(&outcomes, &pairing_report(None, None)), 1);
    }

    // --- the census ----------------------------------------------------------

    #[test]
    fn the_check_list_holds_one_entry_per_registration_in_registration_order() {
        // WITH NOTHING ENABLED, so a census that walked the SELECTION would
        // return an empty report and lose every plugin at once. Registration
        // order is delivery order, and the report is read against the config.
        let (registry, registered, selected) = census("");
        assert_eq!(
            checks(&registered, &selected, ConfigState::Read)
                .iter()
                .map(|check| check.plugin)
                .collect::<Vec<_>>(),
            registry.names(),
            "a report cannot silently omit a plugin"
        );
    }

    #[test]
    fn a_registered_plugin_the_config_did_not_enable_is_a_skip_that_says_which() {
        // BOTH WAYS a config declines a plugin: never naming it, and naming it
        // switched off. Neither is an error and both have to be visible, or
        // the operator reads a short report as a complete one.
        assert_eq!(
            kind_for("[plugins.hermes]\nenabled = true\n", "mobile"),
            CheckKind::Skipped(NOT_ENABLED)
        );
        assert_eq!(
            kind_for("[plugins.mobile]\nenabled = false\n", "mobile"),
            CheckKind::Skipped(NOT_ENABLED)
        );
    }

    #[test]
    fn a_plugin_the_selection_left_out_is_skipped_in_words_true_of_this_machine() {
        // THREE STATES, THREE EDITS. "Not enabled in the config" is a lie on a
        // machine with no config: it points the operator at a file that does
        // not exist, and it became the ORDINARY report there the moment the
        // fallback narrowed from the whole roster to the core. The unreadable
        // config is its own state again, because one is fixed by writing a
        // file and the other by repairing one.
        let registry = roster();
        let registered = registry.all();
        let core = registry.core();
        let reason = |config| {
            checks(&registered, &core, config)
                .into_iter()
                .find(|check| check.plugin == "hermes")
                .expect("hermes is registered")
                .kind
        };
        assert_eq!(reason(ConfigState::Read), CheckKind::Skipped(NOT_ENABLED));
        assert_eq!(reason(ConfigState::Absent), CheckKind::Skipped(NO_CONFIG));
        assert_eq!(
            reason(ConfigState::Unreadable),
            CheckKind::Skipped(UNREADABLE_CONFIG)
        );
        // AND THE THREE ARE DIFFERENT SENTENCES, which is the whole point: a
        // constant accidentally pointed at another would pass every equality
        // above and report one state as another.
        assert_eq!(
            [NOT_ENABLED, NO_CONFIG, UNREADABLE_CONFIG]
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            3
        );
    }

    #[test]
    fn a_selected_sensor_is_a_skip_because_no_leg_can_ever_reach_one() {
        assert_eq!(
            kind_for("[plugins.router]\nenabled = true\n", "router"),
            CheckKind::Skipped(A_SENSOR)
        );
    }

    #[test]
    fn a_selected_channel_no_event_dispatches_is_a_pulse_rather_than_a_send() {
        assert_eq!(
            kind_for("[plugins.hue]\nenabled = true\n", "hue"),
            CheckKind::Pulse
        );
    }

    #[test]
    fn a_selected_event_dispatched_channel_is_a_send() {
        for plugin in ["mobile", "macos-banner", "hermes"] {
            assert_eq!(
                kind_for(
                    "[plugins.mobile]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n\
                     [plugins.hermes]\nenabled = true\n",
                    plugin
                ),
                CheckKind::Send,
                "plugin: {plugin}"
            );
        }
    }

    // --- the report ----------------------------------------------------------

    #[test]
    fn a_line_names_its_plugin_and_its_outcome_and_a_failure_quotes_the_channel() {
        let hermes = Check {
            plugin: "hermes",
            kind: CheckKind::Send,
        };
        assert_eq!(
            line(&hermes, &Outcome::Sent("posted HTTP 200".to_string())),
            "hermes: sent, posted HTTP 200"
        );
        assert_eq!(
            line(
                &hermes,
                &Outcome::Failed("post FAILED HTTP 401".to_string())
            ),
            "hermes: FAILED, post FAILED HTTP 401",
            "the channel's own sentence, verbatim: a doctor that paraphrased \
             would be a second wording of one answer"
        );
        assert_eq!(
            line(&hermes, &Outcome::SentUnreported),
            "hermes: sent, this channel reports no outcome"
        );
        let router = Check {
            plugin: "router",
            kind: CheckKind::Skipped(A_SENSOR),
        };
        assert_eq!(
            line(&router, &Outcome::Skipped(A_SENSOR)),
            "router: skipped, a sensor and never a delivery destination"
        );
    }

    #[test]
    fn the_pulse_line_claims_neither_a_flash_nor_a_cause_it_cannot_know() {
        let hue = Check {
            plugin: "hue",
            kind: CheckKind::Pulse,
        };
        assert_eq!(
            line(&hue, &Outcome::Signalled(2)),
            "hue: signalled 2 rooms (watch for the flash; the bridge acknowledges no write)"
        );
        assert_eq!(
            line(&hue, &Outcome::Signalled(1)),
            "hue: signalled 1 room (watch for the flash; the bridge acknowledges no write)"
        );
        assert_eq!(
            line(&hue, &Outcome::Signalled(0)),
            "hue: FAILED, signalled no rooms \
             (no room listing from the bridge, or no configured room name matched)",
            "zero names both causes rather than choosing one, and no count claims the \
             lights actually flashed"
        );
    }

    #[test]
    fn the_summary_counts_every_check_exactly_once() {
        let outcomes = [
            Outcome::Skipped(A_SENSOR),
            Outcome::Sent("posted HTTP 200".to_string()),
            Outcome::SentUnreported,
            Outcome::Failed("post FAILED HTTP 401".to_string()),
            Outcome::Signalled(2),
            Outcome::Signalled(0),
            Outcome::Skipped(NOT_ENABLED),
        ];
        let summarized = summary(&outcomes);
        assert_eq!(summarized, "pns doctor: 3 sent, 2 failed, 2 skipped");
        let counted: usize = summarized
            .split_whitespace()
            .filter_map(|word| word.parse::<usize>().ok())
            .sum();
        assert_eq!(
            counted,
            outcomes.len(),
            "a check that fell into no bucket is a plugin the summary lost"
        );
    }

    // --- the lights section --------------------------------------------------

    /// A routing with three lamps, each carrying a different behaviour set, so
    /// the per-behaviour counts below can differ from each other.
    fn routing() -> pns_hue::Routing {
        let lamp = |name: &str| pns_hue::Lamp {
            id: format!("id-{name}"),
            name: name.to_string(),
            room: None,
            zones: Vec::new(),
        };
        pns_hue::Routing {
            lamps: vec![
                pns_hue::Routed {
                    lamp: lamp("HCL1"),
                    shows: vec![Behaviour::Done, Behaviour::Failed],
                    dim: None,
                },
                pns_hue::Routed {
                    lamp: lamp("HCL2"),
                    shows: vec![Behaviour::Done, Behaviour::Failed],
                    dim: None,
                },
                pns_hue::Routed {
                    lamp: lamp("HCL3"),
                    shows: vec![Behaviour::Blocked, Behaviour::Unread],
                    dim: None,
                },
            ],
            unresolved: Vec::new(),
            refusals: Vec::new(),
        }
    }

    #[test]
    fn the_lights_section_says_which_of_its_six_states_the_config_is_in() {
        assert_eq!(
            lights_lines(&LightsReport::Off),
            vec![
                "pns doctor: lights: off in the config, so the pulse uses the \
                 [plugins.hue] rooms"
            ],
            "no table is the state every machine was in before this table existed"
        );
        assert_eq!(
            lights_lines(&LightsReport::HueMissing),
            vec![
                "pns doctor: lights: configured, but there is no [plugins.hue] \
                 table to light them through"
            ],
            "A TABLE THAT WAS NEVER WRITTEN IS NOT A SWITCH SOMEONE TURNED OFF. \
             One is a config that is half finished and the other is a decision, \
             and telling an operator to go flip a switch that is not there is the \
             kind of wrong direction they act on"
        );
        assert_eq!(
            lights_lines(&LightsReport::HueDisabled),
            vec![
                "pns doctor: lights: configured, but [plugins.hue] enabled is false, \
                 so nothing lights"
            ],
            "ONE SWITCH, and the doctor is where an operator sees it is off"
        );
        assert_eq!(
            lights_lines(&LightsReport::NoBridge),
            vec![
                "pns doctor: lights: no [plugins.hue] bridge and key, so no lamp \
                 could be resolved"
            ],
            "a config that named no bridge is not a bridge that answered nothing"
        );
        assert_eq!(
            lights_lines(&LightsReport::Unreachable),
            vec!["pns doctor: lights: the bridge listed nothing, so no lamp resolved"],
            "a bridge that answered nothing is not a config that named nothing"
        );
        assert_eq!(
            lights_lines(&LightsReport::Resolved(routing())),
            vec!["pns doctor: lights: done 2, failed 2, blocked 1, unread 1, loop 0"],
            "PER BEHAVIOUR, which is the question an operator opens this section \
             with: did the thing I routed reach a bulb. A behaviour nothing carries \
             is listed at zero rather than left out, because an absence reads as fine"
        );
    }

    #[test]
    fn an_unresolved_name_and_a_refused_declaration_each_get_their_own_line() {
        let mut map = routing();
        map.unresolved = vec![
            pns_hue::Unresolved {
                level: "lamp".to_string(),
                name: "3F - Studio - HCL9".to_string(),
                kind: pns_hue::Missing::NotOnBridge,
            },
            pns_hue::Unresolved {
                level: "room".to_string(),
                name: "3F - Cupboard".to_string(),
                kind: pns_hue::Missing::AddressedNothing,
            },
        ];
        map.refusals = vec!["lights: `HCL1` is covered by 2 zone declarations".to_string()];
        assert_eq!(
            lights_lines(&LightsReport::Resolved(map)),
            vec![
                "pns doctor: lights: done 2, failed 2, blocked 1, unread 1, loop 0",
                "pns doctor: lights: `3F - Studio - HCL9` (lamp) is not on the bridge",
                "pns doctor: lights: `3F - Cupboard` (room) is on the bridge, but it \
                 holds no lamp",
                "pns doctor: lights: `HCL1` is covered by 2 zone declarations",
            ],
            "every miss is named with the level that wrote it, in the words of the \
             miss it actually was, and every refusal in the channel's own words"
        );
    }

    #[test]
    fn every_lights_state_says_something_rather_than_printing_nothing() {
        // WHAT THIS PINS, and only this: a section that reports and never
        // grades has one way to fail silently, which is a state that produces
        // no line at all, leaving the operator to read an absence as "fine".
        for report in [
            LightsReport::Off,
            LightsReport::HueMissing,
            LightsReport::HueDisabled,
            LightsReport::NoBridge,
            LightsReport::Unreachable,
            LightsReport::Resolved(routing()),
            LightsReport::Resolved(pns_hue::Routing::default()),
        ] {
            assert!(
                !lights_lines(&report).is_empty(),
                "every state says something, the empty map included"
            );
        }
    }

    // --- the exit contract ---------------------------------------------------

    #[test]
    fn only_a_run_that_sent_something_and_failed_nothing_exits_zero() {
        // THE SENDS ALONE, which is what the inert pairing below holds fixed:
        // a report that could not be checked moves nothing, so every case here
        // is decided by its outcomes exactly as it was before the pairing
        // check existed.
        let no_pairing_answer = pairing_report(None, None);
        assert_eq!(
            exit_code(
                &[
                    Outcome::Sent("posted HTTP 200".to_string()),
                    Outcome::Skipped(NOT_ENABLED),
                ],
                &no_pairing_answer
            ),
            0
        );
        assert_eq!(
            exit_code(&[Outcome::SentUnreported], &no_pairing_answer),
            0,
            "a channel that reports no outcome was still handed the event"
        );
        assert_eq!(exit_code(&[Outcome::Signalled(3)], &no_pairing_answer), 0);
        assert_eq!(
            exit_code(
                &[
                    Outcome::Sent("posted HTTP 200".to_string()),
                    Outcome::Failed("post FAILED HTTP 401".to_string()),
                ],
                &no_pairing_answer
            ),
            1,
            "one failure is enough, however much else worked"
        );
        assert_eq!(
            exit_code(&[Outcome::Signalled(0)], &no_pairing_answer),
            1,
            "a pulse that reached no room reached nothing"
        );
        assert_eq!(
            exit_code(
                &[Outcome::Skipped(NOT_ENABLED), Outcome::Skipped(A_SENSOR)],
                &no_pairing_answer
            ),
            1,
            "a run with nothing to check must never report green"
        );
        assert_eq!(
            exit_code(&[], &no_pairing_answer),
            1,
            "and neither must an empty one"
        );
    }

    // --- the moshi pairing check ---------------------------------------------

    #[test]
    fn a_pairing_built_from_no_answer_claims_neither_paired_nor_unpaired() {
        let report = pairing_report(None, None);
        assert_eq!(
            report.pairing,
            Pairing::NoAnswer,
            "no answer is its own state, never a guess at one"
        );
        assert_eq!(report.server, None, "and there is nothing to relay either");
    }

    /// `moshi-hook status --json` on this machine, moshi-hook 0.3.3, healthy.
    ///
    /// The three values the capture elided are elided here too (`hooks`,
    /// `logPath`, `socketPath`): NOTHING READS THEM, and `hooks` in particular
    /// is deliberately out of scope, because on this machine it reports the
    /// claude and codex hooks as stale BY DESIGN under the single-submitter
    /// rule, so a check that graded it would page a permanent false alarm.
    const PAIRED_JSON: &str = r#"{"baseUrl":"https://api.getmoshi.app/api/v1",
        "displayName":"dresden","hooks":[],
        "hostId":"host_b14dd2bb0b1f45899d9eaa81a71ff874","logPath":"...",
        "paired":true,"platform":"macos","secretStore":"keychain","socketPath":"..."}"#;

    /// The same call measured with `HOME` pointed at an empty directory: the
    /// answer is `paired: false` and carries no host id at all.
    const UNPAIRED_JSON: &str = r#"{"baseUrl":"https://api.getmoshi.app/api/v1","hooks":[],
        "logPath":"...","paired":false,"platform":"macos","secretStore":"keychain",
        "socketPath":"..."}"#;

    #[test]
    fn a_paired_answer_carries_back_the_host_id_and_display_name_moshi_named() {
        assert_eq!(
            pairing_report(Some(PAIRED_JSON), None).pairing,
            Pairing::Paired {
                host_id: "host_b14dd2bb0b1f45899d9eaa81a71ff874".to_string(),
                display_name: "dresden".to_string(),
            },
            "both come back VERBATIM: a doctor that abbreviated moshi's own \
             identifiers would be a second spelling of one answer, and the host \
             id is the thing an operator compares against the phone"
        );

        // A SHAPE NOBODY HAS SEEN. Every measured `paired: true` carries both
        // names, so this is the fallback rather than a case to design around,
        // and it says the identifier is missing instead of rendering an empty
        // parenthesis the operator would read as a host id they misread.
        assert_eq!(
            pairing_report(Some(r#"{"paired":true}"#), None).pairing,
            Pairing::Paired {
                host_id: "not reported".to_string(),
                display_name: "not reported".to_string(),
            }
        );
    }

    #[test]
    fn an_unpaired_answer_is_unpaired_rather_than_unreadable() {
        assert_eq!(
            pairing_report(Some(UNPAIRED_JSON), None).pairing,
            Pairing::Unpaired,
            "an answer naming no host is still an ANSWER: reading it as \
             unreadable would make the one state that earns an exit 1 inert"
        );
    }

    #[test]
    fn json_that_will_not_parse_or_names_no_paired_key_claims_neither() {
        for answer in [
            "",
            "not json at all",
            "{",
            r#"{"displayName":"dresden"}"#,
            r#"{"paired":"yes"}"#,
        ] {
            assert_eq!(
                pairing_report(Some(answer), None).pairing,
                Pairing::Unreadable,
                "answer: {answer:?}"
            );
        }
    }

    /// `moshi-hook status` (plain), healthy, on this machine. This shape is the
    /// only one carrying a server verdict at all: the JSON answer above is
    /// local-only and measured to perform no network I/O.
    const PAIRED_PLAIN: &str = "status:       paired\n\
         host id:      host_b14dd2bb0b1f45899d9eaa81a71ff874\n\
         display name: dresden\n\
         server:       Moshi Pro attached (usage scope: license)\n";

    #[test]
    fn the_server_line_is_relayed_as_moshis_own_words_with_the_label_removed() {
        assert_eq!(
            pairing_report(Some(PAIRED_JSON), Some(PAIRED_PLAIN))
                .server
                .as_deref(),
            Some("Moshi Pro attached (usage scope: license)"),
            "moshi's own sentence, VERBATIM. pns has no stable way to tell this \
             apart from a host that does not belong to the user token, and any \
             match on the prose would fail in the dangerous direction the day \
             moshi rewords it: a healthy machine failing its doctor, or a real \
             break going unreported"
        );
    }

    #[test]
    fn only_a_server_line_at_column_zero_is_relayed() {
        // moshi's own output indents continuation and detail lines, so a
        // relay anchored on a substring would quote whichever of them said
        // the word first and attribute it to the server.
        let indented_first = "status:       paired\n  server: an indented line\n\
             server:       the server line\n";
        assert_eq!(
            pairing_report(None, Some(indented_first)).server.as_deref(),
            Some("the server line"),
            "the label is a line PREFIX, never a substring anywhere in the line"
        );
        let only_indented = "status:       paired\n  server: an indented line\n";
        assert_eq!(
            pairing_report(None, Some(only_indented)).server,
            None,
            "and an indented line alone is no server verdict at all"
        );
    }

    /// The label the relayed line carries, which is how the report attributes
    /// the sentence to moshi rather than to pns.
    const MOSHI_SAYS: &str = "moshi says";

    #[test]
    fn plain_output_with_no_server_line_relays_nothing_rather_than_an_empty_line() {
        // AN UNPAIRED HOST PRINTS NO `server:` LINE AT ALL, measured, and a
        // future moshi that renamed or dropped the line would print none
        // either. That degradation is the SAFE direction: no relay, and
        // nothing else about the report moves.
        let unpaired_plain = "status:       unpaired\n";
        let report = pairing_report(Some(PAIRED_JSON), Some(unpaired_plain));
        assert_eq!(report.server, None);
        assert!(
            !pairing_lines(&report)
                .iter()
                .any(|line| line.contains(MOSHI_SAYS)),
            "a relay with nothing to relay is an absent line, never a labelled \
             blank one: {:?}",
            pairing_lines(&report)
        );

        // And a label with nothing after it is not a verdict either.
        let empty_value = "status:       paired\nserver:       \n";
        let report = pairing_report(Some(PAIRED_JSON), Some(empty_value));
        assert_eq!(report.server, None);
        assert!(
            !pairing_lines(&report)
                .iter()
                .any(|line| line.contains(MOSHI_SAYS)),
            "{:?}",
            pairing_lines(&report)
        );
    }

    /// What a relayed line is addressed as, so a test can strip it back off.
    const RELAY_OPENING: &str = "pns doctor: moshi says: ";

    #[test]
    fn a_relayed_value_carrying_a_newline_or_a_control_byte_cannot_forge_a_report_line() {
        // THE WHOLE POINT OF THE FILTER. This is third-party text going
        // straight to a terminal, and an unfiltered newline in it would print
        // a second `pns doctor:` line that the operator would read as pns's
        // own verdict. A report that can be made to lie about itself is worse
        // than no relay at all.
        let forged = PairingReport {
            pairing: Pairing::Unpaired,
            server: Some(
                "attached\npns doctor: 9 sent, 0 failed, 0 skipped\r\u{1b}[2Kok\u{7}".to_string(),
            ),
        };
        let lines = pairing_lines(&forged);
        assert_eq!(lines.len(), 2, "the relay forged a line: {lines:?}");
        assert_eq!(
            lines[1],
            "pns doctor: moshi says: attachedpns doctor: 9 sent, 0 failed, 0 skipped[2Kok",
            "the newline, the carriage return, the escape and the bell are all \
             gone, and what is left is visibly inside one relayed line"
        );

        // AND THE SAME THROUGH THE READING PATH. A carriage return is the one
        // that survives being split into lines, and on a terminal it returns
        // the cursor to column zero for whatever follows to overwrite the
        // report's own prefix with.
        let read = pairing_report(None, Some("server:       up\rpns doctor: forged\n"));
        let lines = pairing_lines(&read);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(
            !lines[1].contains('\r'),
            "a carriage return reached the terminal: {:?}",
            lines[1]
        );
    }

    #[test]
    fn an_identity_moshi_named_cannot_forge_a_report_line_either() {
        // THE IDENTITY FIELDS ARE THIRD-PARTY TEXT TOO, and they reach the
        // same terminal by the same route. MEASURED: a `displayName` carrying
        // a newline and a forged summary printed
        // `pns doctor: 9 sent, 0 failed, 0 skipped` as its own line inside the
        // real report, which is the exact forgery the relay filter exists to
        // stop one field to the left.
        let forged = "{\"paired\":true,\
             \"displayName\":\"dresd\u{e9}n\\npns doctor: 9 sent, 0 failed, 0 skipped\\r\\u001b[2K\",\
             \"hostId\":\"host_1\\nforged\"}";
        let lines = pairing_lines(&pairing_report(Some(forged), None));

        assert_eq!(
            lines.iter().flat_map(|line| line.lines()).count(),
            1,
            "an identity forged a report line: {lines:?}"
        );
        assert_eq!(
            lines[0],
            "pns doctor: moshi pairing: paired as dresdnpns doctor: 9 sent, 0 failed, \
             0 skipped[2K (host_1forged).",
            "BOTH fields go through the SAME filter the relayed sentence does: \
             the newline, the carriage return and the escape are gone, the \
             non-ASCII character is gone with them, and what is left is \
             visibly inside the one line pns wrote"
        );
        assert!(
            !lines[0].chars().any(char::is_control),
            "a control byte reached the terminal: {:?}",
            lines[0]
        );
    }

    #[test]
    fn an_over_long_relayed_value_stops_at_the_cap() {
        let report = PairingReport {
            pairing: Pairing::Unpaired,
            server: Some("x".repeat(500)),
        };
        let lines = pairing_lines(&report);
        let relayed = lines[1]
            .strip_prefix(RELAY_OPENING)
            .unwrap_or_else(|| panic!("{:?}", lines[1]));
        assert_eq!(
            relayed.chars().count(),
            200,
            "an unbounded relay is an unbounded line in somebody else's report"
        );

        // COUNTED IN CHARACTERS AND FILTERED FIRST, so the cap can never land
        // inside a multi-byte sequence: a character outside printable ASCII is
        // gone before anything is counted.
        let multibyte = PairingReport {
            pairing: Pairing::Unpaired,
            server: Some("\u{e9}".repeat(300)),
        };
        assert_eq!(
            pairing_lines(&multibyte).len(),
            1,
            "nothing printable survived, so there is nothing to relay"
        );
    }

    #[test]
    fn the_paired_line_names_the_host_and_claims_nothing_about_approvals() {
        let lines = pairing_lines(&pairing_report(Some(PAIRED_JSON), Some(PAIRED_PLAIN)));
        assert_eq!(
            lines,
            [
                "pns doctor: moshi pairing: paired as dresden \
                 (host_b14dd2bb0b1f45899d9eaa81a71ff874).",
                "pns doctor: moshi says: Moshi Pro attached (usage scope: license)",
            ]
        );
        // IT SAYS WHO THIS HOST IS PAIRED AS AND STOPS THERE. A re-pair mints
        // a new host id while the live daemon keeps serving the old one, and
        // an approval only really round trips when a human taps a card:
        // neither is visible from here, so neither may be implied.
        for overclaim in ["approvals work", "working", "will reach", "healthy"] {
            assert!(
                !lines[0].contains(overclaim),
                "the line claims {overclaim:?}, which this check cannot see: {:?}",
                lines[0]
            );
        }
    }

    #[test]
    fn the_unpaired_line_says_the_cards_are_dead_and_names_the_command_that_fixes_it() {
        let lines = pairing_lines(&pairing_report(Some(UNPAIRED_JSON), None));
        assert_eq!(
            lines,
            [
                "pns doctor: moshi pairing: this host is NOT paired, so every \
              approval card is dead until `moshi-hook pair` runs."
            ],
            "the remedy is IN THE LINE: this is the state the whole check \
             exists for, and the census reports the moshi channel green over \
             its webhook the entire time"
        );
    }

    #[test]
    fn the_no_answer_line_offers_both_explanations_and_commits_to_neither() {
        let lines = pairing_lines(&pairing_report(None, None));
        assert_eq!(
            lines,
            ["pns doctor: moshi pairing: moshi-hook did not answer (not \
              installed, or it did not answer in time), so the approval path \
              could not be checked."],
            "the bounded spawn cannot tell an absent binary from one that hung \
             or exited non-zero, so the line names two explanations and picks \
             neither"
        );

        // The fourth state, and the last one with a line of its own: moshi
        // answered, and the answer was a shape this does not recognize.
        assert_eq!(
            pairing_lines(&pairing_report(Some("{"), None)),
            [
                "pns doctor: moshi pairing: moshi-hook answered something this \
              cannot read."
            ]
        );
    }

    #[test]
    fn an_unpaired_host_alone_earns_the_exit_code_a_one() {
        // THE JUDGEMENT CALL. It only fires on a machine moshi-hook is
        // installed and answering on, which is a machine that set moshi up,
        // and on one of those an unregistered host means every approval card
        // is dead while the census reports the moshi channel green over its
        // webhook. That gap is the entire reason this check exists apart from
        // the census.
        let every_send_green = [Outcome::Sent("posted HTTP 200".to_string())];
        assert_eq!(
            exit_code(&every_send_green, &pairing_report(Some(PAIRED_JSON), None)),
            0,
            "the control: the same sends with a healthy pairing"
        );
        assert_eq!(
            exit_code(
                &every_send_green,
                &pairing_report(Some(UNPAIRED_JSON), None)
            ),
            1,
            "the pairing ALONE moved it, with nothing else changed"
        );
    }

    #[test]
    fn a_no_answer_or_unreadable_pairing_leaves_a_green_run_exiting_zero() {
        // A MACHINE THAT DOES NOT USE MOSHI MUST NOT FAIL ITS DOCTOR FOREVER,
        // and neither must one whose moshi answered a shape this cannot read:
        // both are "could not check", and a check that could not run is not a
        // failure it found.
        let every_send_green = [Outcome::Sent("posted HTTP 200".to_string())];
        for could_not_check in [pairing_report(None, None), pairing_report(Some("{"), None)] {
            assert_eq!(
                exit_code(&every_send_green, &could_not_check),
                0,
                "{could_not_check:?}"
            );
        }
    }

    #[test]
    fn a_failed_send_still_exits_one_when_the_pairing_is_healthy() {
        // NEITHER READER OVERRIDES THE OTHER. A healthy pairing cannot mask a
        // send that failed, and it cannot turn a run with nothing to check
        // green either.
        let healthy = pairing_report(Some(PAIRED_JSON), Some(PAIRED_PLAIN));
        assert_eq!(
            exit_code(
                &[
                    Outcome::Sent("posted HTTP 200".to_string()),
                    Outcome::Failed("post FAILED HTTP 401".to_string()),
                ],
                &healthy
            ),
            1
        );
        assert_eq!(
            exit_code(&[Outcome::Skipped(NOT_ENABLED)], &healthy),
            1,
            "a run with nothing to check must never report green, whatever \
             the pairing says"
        );
    }
}

#[cfg(test)]
mod daemon_tests {
    use super::daemon_line;
    use crate::daemon::{HEARTBEAT_STALE_SECS, Heartbeat, parse_heartbeat, render_heartbeat};

    const NOW: u64 = 1_700_000_000;

    /// Four of the five states, each with its own sentence. The fifth, a switch
    /// turned off while the process is still beating, is next door, because it
    /// is the one an operator reaches by ACTING rather than by waiting.
    ///
    /// NONE OF THEM MOVES THE EXIT CODE, which is asserted where the exit code
    /// is decided: `exit_code` takes outcomes and a pairing report and this
    /// line is neither, so a dead daemon structurally cannot read as a broken
    /// notifier. The binary suite pins the same thing end to end.
    #[test]
    fn the_daemons_doctor_line_tells_the_truth_in_four_states() {
        assert_eq!(
            daemon_line(false, None, Some(NOW), 0),
            "pns doctor: the daemon is off in the config"
        );
        assert_eq!(
            daemon_line(true, None, Some(NOW), 0),
            "pns doctor: the daemon is enabled and has not run yet"
        );
        let stale = Heartbeat {
            pid: 4321,
            at: NOW - HEARTBEAT_STALE_SECS - 1,
        };
        assert_eq!(
            daemon_line(true, Some(stale), Some(NOW), 2),
            format!(
                "pns doctor: the daemon is enabled, its last beat was {}s ago, \
                 so it is not running",
                HEARTBEAT_STALE_SECS + 1
            )
        );
        let fresh = Heartbeat {
            pid: 4321,
            at: NOW - HEARTBEAT_STALE_SECS,
        };
        assert_eq!(
            daemon_line(true, Some(fresh), Some(NOW), 2),
            "pns doctor: the daemon is running, pid 4321, 2 jobs scheduled"
        );
        assert_eq!(
            daemon_line(true, Some(fresh), Some(NOW), 1),
            "pns doctor: the daemon is running, pid 4321, 1 job scheduled"
        );
    }

    /// OFF IN THE CONFIG IS NOT THE SAME FACT AS STOPPED.
    ///
    /// Nothing bounces the launchd job when the config changes, so a daemon
    /// started while the switch was on keeps running and keeps firing after it
    /// is turned off. The operator who just flipped it is standing in exactly
    /// that state, and a line that said only "off in the config" would be
    /// telling them the opposite of the truth at the one moment they looked.
    #[test]
    fn a_daemon_switched_off_but_still_beating_is_reported_as_still_beating() {
        let beating = Heartbeat { pid: 991, at: NOW };
        assert_eq!(
            daemon_line(false, Some(beating), Some(NOW), 3),
            "pns doctor: the daemon is off in the config, but pid 991 is still beating; \
             bootout (or wait) to stop it"
        );
        // A BEAT TOO OLD TO VOUCH FOR IS NOT A RUNNING PROCESS, so the plain
        // sentence is what an operator who stopped it days ago still reads.
        let stale = Heartbeat {
            pid: 991,
            at: NOW - HEARTBEAT_STALE_SECS - 1,
        };
        assert_eq!(
            daemon_line(false, Some(stale), Some(NOW), 0),
            "pns doctor: the daemon is off in the config"
        );
    }

    /// A beat this machine cannot grade is NOT RUNNING rather than running.
    ///
    /// FAIL TOWARDS THE HONEST REPORT: no clock, or a beat stamped in the
    /// future, both mean the age is not a number, and claiming a daemon is
    /// alive on the strength of a timestamp nothing could compare is the
    /// identity-is-not-presence mistake with a file standing in for the pid.
    #[test]
    fn a_heartbeat_whose_age_cannot_be_taken_reads_as_not_running() {
        let beat = Heartbeat { pid: 7, at: NOW };
        for (case, now) in [
            ("no clock", None),
            ("a beat from the future", Some(NOW - 5)),
        ] {
            let line = daemon_line(true, Some(beat), now, 0);
            assert!(
                line.contains("not running") && line.contains("an unknown time"),
                "{case}: {line}"
            );
        }
    }

    /// The heartbeat file's own round trip, since the doctor's whole reading
    /// arrives through it.
    #[test]
    fn a_heartbeat_round_trips_and_anything_else_is_no_heartbeat_at_all() {
        let beat = Heartbeat { pid: 4321, at: NOW };
        assert_eq!(parse_heartbeat(&render_heartbeat(&beat)), Some(beat));
        for not_a_beat in ["", "4321", "4321 soon", "nobody 1700000000", "0 1700000000"] {
            assert_eq!(parse_heartbeat(not_a_beat), None, "case: {not_a_beat}");
        }
    }
}
