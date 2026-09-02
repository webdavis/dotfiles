//! The config edge: `~/.config/pns/config.toml` decides which plugins run.
//!
//! The file SELECTS; it never defines. Every plugin is compiled in, disabled
//! until its table says `enabled = true`, so a machine runs exactly what its
//! config names and nothing else. The settings inside a plugin's table are
//! free-form here: this layer proves the shape, the registry interprets the
//! contents, and neither knows the other's plugin names.
//!
//! `[recap]`, `[focus]`, `[daemon]` and `[lights]` are the four top-level
//! tables that are not plugins: four booleans, two counts, one argument list,
//! one list of Focus mode names and the lamp policy's own scalars and maps,
//! all read by THIS layer. Because it reads them, it can judge them, so an
//! unknown key inside any of them, a count that is not a threshold, and a
//! summarizer that is not a list of command words are refused rather than
//! passed along the way a plugin's settings are.
//!
//! Failure directions, each pinned by a test: a MALFORMED file is a loud
//! error and never a silent empty config, because a typo that turns every
//! notification off must not pass quietly; a MISSING file is its own honest
//! outcome, distinct from both error and emptiness, so the caller can say
//! "unconfigured" instead of guessing; unknown top-level keys are refused,
//! so `[plugin.hue]` cannot silently disable what `[plugins.hue]` enables.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// One plugin's slice of the config: the selection flag, and its settings
/// with the flag itself removed, because `enabled` belongs to this layer and
/// everything else belongs to the plugin.
#[derive(Debug, PartialEq)]
pub struct PluginEntry {
    pub enabled: bool,
    pub settings: toml::Table,
}

/// The recap's three delivery switches, its volume threshold, and the command
/// it hands the window to.
///
/// ABSENT IS ALL ON, which is what makes the table optional: a machine that
/// never writes one behaves exactly as it did before the table existed. Each
/// boolean gates ONLY its own delivery, so recap-only and card-only are both
/// valid configurations and neither implies the other.
///
/// THE DEFAULT IS WRITTEN OUT rather than derived. `#[derive(Default)]` reads
/// a bool as false, which would take every delivery away from every machine
/// whose config was written before this table existed, and it would do it
/// silently.
///
/// `min_events` IS A KEY RATHER THAN A CONSTANT because nobody can calibrate it
/// yet: the locked volume threshold carries a tilde, and the machine it was
/// written for has no history to measure. The recap prints the window's real
/// count in its own header every time, so one week of real recaps settles the
/// number without a rebuild.
///
/// `summarizer` IS ARGV AND NEVER A SHELL STRING, which is what makes it a
/// backend switch rather than a plugin: nothing is interpreted, so there is no
/// quoting rule and no injection surface, and a different backend is simply a
/// different array. UNSET IS A WORKING SETTING, and the common one: with no
/// summarizer the recap posts the plain mechanical lists.
///
/// `repos` AND `review_notes` ARE THE TWO SOURCES PNS CANNOT FIND ON ITS OWN,
/// which is why they are keys and why an absent one is the working setting.
/// The engine knows project NAMES off a working directory and nothing about
/// which repository they are, and the review notes are one operator's own
/// pipeline directory. UNSET MEANS THE SOURCE IS NEVER READ AT ALL: no `gh` is
/// spawned and no directory is opened, which is the fence that makes both
/// sections opt-in rather than merely empty.
///
/// ONE NAMED VALUE, never a row of loose booleans. Four of the eight fields are
/// bools or counts; spread through a call they would sit adjacent and a swap
/// would go unnoticed, and named fields cannot be transposed. It is CLONE
/// rather than Copy only because the argv is a `Vec`, and the composition root
/// clones it once off a borrowed config.
#[derive(Debug, Clone, PartialEq)]
pub struct Recap {
    pub replay_card: bool,
    pub digest: bool,
    pub digest_as_thread: bool,
    pub min_events: usize,
    pub summarizer: Option<Vec<String>>,
    pub summarizer_deadline_secs: u64,
    pub repos: Vec<String>,
    pub review_notes: Option<String>,
}

impl Default for Recap {
    fn default() -> Self {
        Recap {
            replay_card: true,
            digest: true,
            digest_as_thread: true,
            min_events: DEFAULT_MIN_EVENTS,
            summarizer: None,
            summarizer_deadline_secs: DEFAULT_SUMMARIZER_DEADLINE_SECS,
            repos: Vec::new(),
            review_notes: None,
        }
    }
}

/// The lamps' policy: how often a state is re-armed, what each of the five
/// behaviours looks like, and which lamps carry which of them.
///
/// THE TABLE IS OPTIONAL AND ITS ABSENCE IS NOT ITS DEFAULT, which is why the
/// config holds an `Option` of this rather than the struct. A machine with no
/// `[lights]` table keeps the room-based pulse it has always had; a machine
/// with an empty one has asked for the lamps and named no lamp yet. Those are
/// different states and the doctor says different things about them.
///
/// THE DEFAULT IS WRITTEN OUT rather than derived, for `Recap`'s reason: a
/// derived `u64` is zero, and zero is refused by every one of these keys, so a
/// derive would make the empty table unrepresentable through its own parser.
///
/// ONLY THE KNOBS THAT APPLY TO A BEHAVIOUR EXIST (operator ruling): a pulse
/// has a duration and one brightness, a breathing state has a duration and two
/// ends, and some of them carry one knob more besides (unread's delay, loop's
/// threshold and lease, blocked's give-up backstop). There is no dead knob
/// anywhere for a reader to set and watch do nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Lights {
    pub refresh_secs: u64,
    pub done: Pulse,
    pub failed: Pulse,
    pub blocked: Blocked,
    pub unread: Unread,
    /// `[lights.loop]`. NOT SPELLED `r#loop` AT THE FIELD, because every reader
    /// would then carry the raw identifier through; the TOML key is `loop` and
    /// the mapping is stated once, in `parse_lights`.
    pub looping: Looping,
    /// The one dim FORM, shared by every behaviour that runs dimmed, because
    /// the operator locked one shape rather than one per behaviour. WHICH
    /// behaviours run it is a per-target opt-in, not a knob here.
    pub dim: Breath,
    pub lamps: BTreeMap<String, Target>,
    pub rooms: BTreeMap<String, Target>,
    pub zones: BTreeMap<String, Target>,
}

/// A blink: how long the bridge runs it, and how bright.
///
/// NO LOW, because a pulse has no low to run to. That is the config ruling
/// applied at the type level rather than in a comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pulse {
    pub duration_ms: u64,
    pub brightness: u8,
}

/// A breath: how long ONE fade takes, and the two ends it fades between.
///
/// `high` IS THE PEAK AND IS WHERE A BREATH STOPS. The driver finishes its
/// in-flight cycle at the peak so the next tick resumes from there, which is
/// why `low` above `high` is refused at load rather than rendered upside down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breath {
    pub duration_ms: u64,
    pub high: u8,
    pub low: u8,
}

/// The blocked lamp: its breath, plus how long an unanswered wait may hold it
/// before the daemon gives up on an abandoned session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocked {
    pub breath: Breath,
    pub give_up_after_secs: u64,
}

/// The unread lamp: its breath, plus how old SUCCESS news must be before it
/// arms. Failure news arms with no delay at all and has no knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unread {
    pub breath: Breath,
    pub after_secs: u64,
}

/// The loop lamp: its breath, how long work must run before the automatic
/// trigger arms, and how long a hand-taken lease survives without renewal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Looping {
    pub breath: Breath,
    pub threshold_secs: u64,
    pub lease_timeout_secs: u64,
}

/// One declaration, at one of the three levels, and the questions it answers.
///
/// EACH FIELD IS ONE QUESTION, resolved independently of the others: a lamp's
/// own declaration can state which behaviours it carries and say nothing about
/// dimming, and its room's window still applies. `Option` is what spells "said
/// nothing" for the behaviour set; the dim question is stated exactly when
/// `dim_window` is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Target {
    pub shows: Option<Vec<Behaviour>>,
    pub dim_window: Option<String>,
    /// The behaviours that run their DIM FORM inside that window. Everything
    /// else the target carries is suppressed there, which is what makes a
    /// window with an empty list a room that goes dark for the night with no
    /// second mode to spell it.
    pub dim_behaviours: Vec<Behaviour>,
}

/// What a lamp can say. A CLOSED SET, which is the whole reason `[lights]` is
/// judged here instead of passed through as a plugin's free-form settings: a
/// `shows` list holding a word nothing matches is a lamp that stays dark while
/// the operator is sure they routed it, with no message anywhere.
///
/// `Unread` IS ONE WORD AND CARRIES TWO COLOURS. Its success and failure
/// flavours always ride the same lamp, so a config cannot route one without the
/// other and there is no spelling for trying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Behaviour {
    Done,
    Failed,
    Blocked,
    Unread,
    Looping,
}

/// The five words, in the spelling a config uses, and the order the refusal
/// lists them in.
pub const BEHAVIOUR_WORDS: [(&str, Behaviour); 5] = [
    ("done", Behaviour::Done),
    ("failed", Behaviour::Failed),
    ("blocked", Behaviour::Blocked),
    ("unread", Behaviour::Unread),
    ("loop", Behaviour::Looping),
];

impl Default for Lights {
    fn default() -> Self {
        Lights {
            refresh_secs: DEFAULT_REFRESH_SECS,
            done: DEFAULT_DONE,
            failed: DEFAULT_FAILED,
            blocked: Blocked {
                breath: DEFAULT_BLOCKED,
                give_up_after_secs: DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS,
            },
            unread: Unread {
                breath: DEFAULT_UNREAD_BREATH,
                after_secs: DEFAULT_UNREAD_AFTER_SECS,
            },
            looping: Looping {
                breath: DEFAULT_LOOP_BREATH,
                threshold_secs: DEFAULT_LOOP_THRESHOLD_SECS,
                lease_timeout_secs: DEFAULT_LEASE_TIMEOUT_SECS,
            },
            dim: DEFAULT_DIM,
            lamps: BTreeMap::new(),
            rooms: BTreeMap::new(),
            zones: BTreeMap::new(),
        }
    }
}

/// How often a lamp holding a state is re-armed.
///
/// TWELVE, and it is a breath budget rather than a round number: the tick's own
/// driver fades a breathing lamp for its whole interval and stops at the peak,
/// so the interval is what decides how many fades fit between two ticks. Twelve
/// seconds holds three two-second cycles or one four-second one, which is what
/// the locked shapes were measured at.
const DEFAULT_REFRESH_SECS: u64 = 12;

/// The floor under it, and it is the TRANSPORT DEADLINE rather than a round
/// number: a tick makes bounded bridge calls whose own limit is ten seconds
/// (`BRIDGE_DEADLINE`), so an interval shorter than one call can start a tick
/// while the last one is still dialling. Below this the knob is asking for a
/// pile of children rather than a faster lamp.
const MIN_REFRESH_SECS: u64 = 10;

/// And the ceiling: THE LONGEST A TICK'S CHILD IS ALLOWED TO LIVE.
///
/// THIRTY, AND IT IS THE DAEMON'S OWN BOUND read back rather than a round
/// number. The daemon kills a job's child after `CHILD_TICKS` of its own tick
/// (thirty, at the production tick of one second), and a breathing tick now
/// SLEEPS for most of its interval issuing fades. So an interval past that is an
/// interval whose breath is cut off part way through with nothing said anywhere:
/// the lamp freezes at whatever brightness the last fade reached and sits there
/// until the next tick. Refusing the interval at load is what keeps the two
/// numbers from disagreeing silently.
///
/// IT IS ALSO UNDER THE ORDINARY LEASE, which the old ceiling was: the tick is
/// registered with `until` at least as far as its own first due second, so a
/// refresh longer than that lease would EXTEND it, and the two lease lengths
/// would stop being the fixed numbers they are documented as.
///
/// THIRTY SECONDS IS NOT A NARROW LAMP EITHER. It holds seven full cycles of the
/// locked blocked shape and three of the slow one, so nothing an operator would
/// want is out of reach above it.
const MAX_REFRESH_SECS: u64 = 30;

/// The five locked shapes. EVERY NUMBER HERE WAS SET ON A REAL LAMP under the
/// operator's observe-adjust-lock protocol (2026-08-31 and 2026-09-01), so a
/// change to one of them is a change to something that was looked at, not a
/// tuning.
const DEFAULT_DONE: Pulse = Pulse {
    duration_ms: 4000,
    brightness: 100,
};
const DEFAULT_FAILED: Pulse = Pulse {
    duration_ms: 4000,
    brightness: 100,
};
const DEFAULT_BLOCKED: Breath = Breath {
    duration_ms: 2000,
    high: 100,
    low: 30,
};
const DEFAULT_UNREAD_BREATH: Breath = Breath {
    duration_ms: 4000,
    high: 60,
    low: 10,
};
const DEFAULT_LOOP_BREATH: Breath = Breath {
    duration_ms: 4000,
    high: 60,
    low: 10,
};

/// The dim form: the same seamless cadence at the faintest levels the hardware
/// has. Drill D4 measured a lamp asked for one percent reporting 1.19, which is
/// its own floor rather than a rounding.
const DEFAULT_DIM: Breath = Breath {
    duration_ms: 3000,
    high: 7,
    low: 1,
};

/// How old SUCCESS news must be before the unread lamp arms: five minutes, so a
/// result the operator is already looking at does not light a lamp about itself.
/// FAILURE news has no such delay and no knob.
const DEFAULT_UNREAD_AFTER_SECS: u64 = 300;

/// How long an unanswered wait may hold the blocked lamp before the daemon
/// gives up on an abandoned session (operator ruling 2026-09-01).
///
/// SIXTEEN HOURS, AND IT IS STILL A BACKSTOP RATHER THAN AN EXPIRY. The locked
/// behaviour is blue breathing CONTINUOUS UNTIL THE OPERATOR ANSWERS, so any
/// bound at all is a departure from it and the only honest job left for one is
/// releasing a bulb from a session that will never come back. Sixteen hours
/// outlasts a long day away and still gives the bulb back before the next one
/// starts. The ORDINARY end is not this at all: the session's next event
/// clears the marker, whatever the hour.
pub(crate) const DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS: u64 = 16 * 60 * 60;

/// How long work must run continuously before the loop lamp arms itself.
const DEFAULT_LOOP_THRESHOLD_SECS: u64 = 300;

/// How long a hand-taken loop lease survives with nothing renewing it.
///
/// SIXTY-FIVE MINUTES, and the number comes from what renews it: the lease is
/// refreshed by the calling pane's ordinary hook traffic, and the harness's own
/// wakeup scheduler clamps a sleep to 3600 seconds, so the longest legitimate
/// gap between two events from a live loop is an hour. A timeout at the hour
/// itself would drop a lease that was about to be renewed.
const DEFAULT_LEASE_TIMEOUT_SECS: u64 = 3900;

/// How long any threshold or timeout in this table may be. A day, which is the
/// bound the working streak already carried: work that has been going for
/// longer has stalled, and a threshold past it describes a lamp that never
/// lights at all.
const MIN_THRESHOLD_SECS: u64 = 1;
const MAX_THRESHOLD_SECS: u64 = 86_400;

/// The floor under a lease timeout. A minute, because the lease is renewed by
/// event traffic and anything shorter drops a live loop between two turns.
///
/// SHARED WITH THE BLOCKED BACKSTOP'S OWN FLOOR, which needs no separate
/// number: a minute is the same floor for the same reason, a value too small
/// to mean anything below the granularity real event traffic arrives at.
const MIN_LEASE_TIMEOUT_SECS: u64 = 60;

/// The ceiling on the blocked backstop alone. Every OTHER threshold or timeout
/// in this table caps at a day (`MAX_THRESHOLD_SECS`), but an abandoned wait
/// can span a weekend away, so this one gets a week instead of sharing that
/// ceiling.
const MAX_GIVE_UP_AFTER_SECS: u64 = 7 * 24 * 60 * 60;

/// How long ONE fade may take, in milliseconds.
///
/// THE CEILING IS WHAT MAKES THE DRIVER TOTAL. A breath stops at the peak, so
/// the shortest honest run is a fade down and a fade back, and both have to fit
/// inside `MIN_REFRESH_SECS`. At five seconds a pair fits ten with room to
/// spare; past it a tick could be asked for a cycle longer than the interval it
/// has, and the driver would have to either overrun the next tick or stop
/// somewhere other than the peak.
const MIN_FADE_MS: u64 = 200;
const MAX_FADE_MS: u64 = 5000;

/// Percent, so the two ends are the two ends. ZERO IS REFUSED rather than read
/// as off: a dark signal is a lamp that says nothing, and the way to say
/// nothing is to leave the behaviour off that lamp's `shows` list.
const MIN_BRIGHTNESS: u8 = 1;
const MAX_BRIGHTNESS: u8 = 100;

/// How many events a window needs before a recap is worth the operator's
/// attention. The operator's own stated figure; see `Recap`.
const DEFAULT_MIN_EVENTS: usize = 8;

/// How long the summarizer may take before the recap gives up on it and posts
/// the plain lists.
///
/// FOUR MINUTES, and it is generous on purpose. WHAT IT COVERS IS GENERATION,
/// not a model load. Measured with `ollama run qwen3.5:4b` on one machine (an
/// M1 under load): a cold model load cost about 5.5 seconds, paid once, while
/// a full three-call episode took about 114.6 seconds, of which roughly 113.9
/// was tokens being generated at about eleven a second. Prefill was 185
/// milliseconds for 2,050 tokens, so the whole bill is the LENGTH OF THE
/// ANSWER and every other term rounds to noise. Nobody is waiting on it,
/// because the caller is the detached process the event path never joined.
///
/// THE SECONDS ARE ONE MACHINE ON ONE EVENING. What is durable is the shape
/// (prefill free, generation everything, the load small and paid once); the
/// figures are here to be recalibrated by whoever next tunes this number, and
/// no test encodes one. A backend that generates less is what makes this
/// faster, and the config file's own comment carries how.
///
/// ZERO IS ACCEPTED AND IS NOT A TRAP, unlike `min_events`'s zero. A deadline
/// of nothing simply cannot be met, so the recap falls to the plain lists and
/// SAYS it did, which is the same outcome as any other summarizer that does not
/// answer. Nothing silently changes shape, so there is nothing to refuse.
const DEFAULT_SUMMARIZER_DEADLINE_SECS: u64 = 240;

/// The most any summarizer may be given. ONE HOUR, which is fifteen times the
/// default, so no honest backend on any machine meets it; see `seconds` for the
/// two failures that live past it.
const MAX_SUMMARIZER_DEADLINE_SECS: u64 = 3600;

/// How long pns waits for moshi to acknowledge a submission before returning
/// no opinion.
///
/// FIVE SECONDS, and it is the crate's own house number for a local pipe that
/// should have been instant: `payload_deadline` bounds the same kind of thing
/// on the same hook with the same figure. THE WAIT IS A REGISTRATION, NOT A
/// HUMAN WAIT (measured 2026-08-29): `moshi-hook` writes one line to its
/// daemon's socket and returns as soon as the daemon answers, roughly a tenth
/// of a second, and the operator's own decision arrives later and by another
/// road. So five seconds is about thirty times the observed round trip, and a
/// wait past it is a daemon that stopped answering rather than an operator
/// taking their time.
pub const DEFAULT_SUBMIT_DEADLINE_SECS: u64 = 5;

/// The most that wait may be given. ONE HOUR, mirroring the summarizer's
/// ceiling rather than the harness's own PermissionRequest limit: another
/// tool's number is not ours to hard-code, and Codex's differs. There is no
/// off switch, because an unbounded wait is the defect and "off" would be a
/// key whose only function is to restore it.
const MAX_SUBMIT_DEADLINE_SECS: u64 = 3600;

/// EVERY KEY EVERY TABLE SERVES, table by table: the one statement of this
/// schema's vocabulary, and the source of both the refusal that names a
/// mistyped key and the list of what to write instead.
///
/// ONE ROSTER RATHER THAN A LISTING PER REFUSAL, because the failure it exists
/// to prevent is a listing that drifted: a key added to a parse arm and not to
/// the sentence that names the alternatives leaves an operator reading a
/// refusal that omits the key they wanted. Every table checks this before it
/// dispatches, so a key that is not declared here does not work at all, and a
/// key declared here with no arm to read it is refused by that arm and caught
/// by the walk in this module's own tests. Both drifts are red rather than
/// quiet.
///
/// THE PLUGIN TABLES ARE IN IT and their settings are no longer free-form,
/// which is the one behaviour change: a plugin's near miss (`room` for `rooms`,
/// `tokens` for `token`) used to reach the plugin as a setting it did not
/// recognize and cost a destination silently. A table for a plugin nothing
/// registered is NOT here and stays free-form, because this layer has no
/// vocabulary to judge a plugin that does not exist; the registry refuses the
/// NAME, which is the defect in that case.
///
/// THE NESTED ROW IS A PREFIX. `[lights.lamp.<name>]`, `[lights.room.<name>]`
/// and `[lights.zone.<name>]` carry the operator's own names, so the roster
/// holds the part that is the schema's (one row for all three levels) and the
/// refusal names the whole path.
///
/// THE FIRST ROW IS THE FILE'S OWN TOP LEVEL, whose vocabulary is the six TABLE
/// names. It is a row like any other so that the refusal an operator gets for a
/// misspelled or a MOVED table prints from the same source every other refusal
/// prints from, and so the walks in this module's tests reach the outermost
/// level too. It is looked up by `TOP_LEVEL` rather than by a name, because it
/// is the one level with no heading to write.
///
/// NO LENGTH IS DECLARED. A row added to a fixed-size array is a two-place
/// edit, and the count says nothing a reader needs.
pub const TABLE_KEYS: &[(&str, &[&str])] = &[
    (
        TOP_LEVEL,
        &["daemon", "focus", "lights", "nag", "plugins", "recap"],
    ),
    (
        "recap",
        &[
            "digest",
            "digest_as_thread",
            "min_events",
            "replay_card",
            "repos",
            "review_notes",
            "summarizer",
            "summarizer_deadline_secs",
        ],
    ),
    ("focus", &["silence"]),
    ("daemon", &["enabled"]),
    ("nag", &["after_secs"]),
    (
        "lights",
        &[
            "blocked",
            "dim",
            "done",
            "failed",
            "lamp",
            "loop",
            "refresh_secs",
            "room",
            "unread",
            "zone",
        ],
    ),
    ("lights.done", &["brightness", "duration_ms"]),
    ("lights.failed", &["brightness", "duration_ms"]),
    (
        "lights.blocked",
        &["duration_ms", "give_up_after_secs", "high", "low"],
    ),
    ("lights.dim", &["duration_ms", "high", "low"]),
    (
        "lights.unread",
        &["after_secs", "duration_ms", "high", "low"],
    ),
    (
        "lights.loop",
        &[
            "duration_ms",
            "high",
            "lease_timeout_secs",
            "low",
            "threshold_secs",
        ],
    ),
    (TARGET_KEYS, &["dim_behaviours", "dim_window", "shows"]),
    ("plugins.hermes", &["enabled", "key"]),
    (
        "plugins.hue",
        &["bridge", "enabled", "key", "quiet_hours", "rooms"],
    ),
    ("plugins.macos-banner", &["enabled"]),
    (
        "plugins.mobile",
        &[
            "enabled",
            "mobile_watch_card",
            "submit_deadline_secs",
            "token",
            "type",
        ],
    ),
    (
        "plugins.router",
        &[
            "api_key",
            "device_hostname",
            "device_ipv4",
            "device_mac",
            "enabled",
            "router_url",
            "stale_alert_channel",
            "type",
        ],
    ),
];

/// The roster row for the file's own top level. THE EMPTY NAME, because that
/// level has no heading: an operator writes `[recap]`, never a bracket around
/// the file itself, so there is no name a lookup could use.
pub const TOP_LEVEL: &str = "";

/// A chezmoi-templated text with its actions taken out: a directive standing
/// on its own line goes with the line, and an action inside a value becomes
/// `placeholder`.
///
/// THE SHARED STUB, lifted out of this module's own test for the shipped
/// template so `config_text`'s tests can fake-render a secret action the same
/// way: a rendered secret action carries no author quotes of its own (`|
/// toToml` supplies them once chezmoi resolves the value), so `placeholder`
/// must be a quoted string for the substituted text to stand in for what
/// chezmoi would actually have produced. A round-trip test has to stand in
/// for chezmoi before it hands the text to `parse_config`, and one stub is
/// what keeps that standing-in from drifting between the two callers.
///
/// ONLY THAT ONE ACTION IS STOOD IN FOR. An action in value position must
/// read exactly `{{ (keepassxc "<entry>").<field> | toToml }}`, the text
/// `config_text::secret_action` writes; anything else is refused. Swapping a
/// quoted placeholder in for ANY action would let a template line that
/// dropped `| toToml` keep every template test green while chezmoi splices
/// the raw vault bytes in unquoted.
///
/// NOT TEST-ONLY: `pns-config-render` calls this at runtime too, to stand in
/// for chezmoi before self-parsing its own render, so it returns a refusal
/// naming the offender rather than panicking.
pub fn strip_chezmoi_actions(text: &str, placeholder: &str) -> Result<String, String> {
    let mut lines = Vec::new();
    for line in text
        .lines()
        .filter(|line| !line.trim_start().starts_with("{{-"))
    {
        let mut rendered = line.to_string();
        while let Some(start) = rendered.find("{{") {
            let Some(end) = rendered[start..]
                .find("}}")
                .map(|offset| start + offset + 2)
            else {
                return Err(format!(
                    "a chezmoi action is not closed on its own line: {rendered}"
                ));
            };
            let action = &rendered[start..end];
            let is_secret_action = action
                .strip_prefix("{{ (keepassxc \"")
                .and_then(|rest| rest.split_once("\")."))
                .is_some_and(|(entry, rest)| {
                    !entry.contains('"')
                        && crate::config_text::SECRET_FIELDS
                            .iter()
                            .any(|field| rest == format!("{field} | toToml }}}}"))
                });
            if !is_secret_action {
                return Err(format!("not a `| toToml` secret action: {action}"));
            }
            rendered.replace_range(start..end, placeholder);
        }
        lines.push(rendered);
    }
    Ok(lines.join("\n"))
}

/// How many `key = value` pairs a config-shaped text documents, commented
/// lines included, having checked each one against the roster row of the
/// heading above it.
///
/// THE SCAN READS THE COMMENTED LINES TOO, which is the half a parse cannot
/// reach: most of a documented config is documentation, and a key documented
/// there but refused by the code is a line an operator uncomments and then
/// cannot load.
///
/// ONE SCANNER FOR BOTH TEXTS HELD TO THIS SCHEMA, the shipped template and
/// what `pns setup` composes. Two copies would be two things to keep in
/// agreement with the roster, which is the drift the roster itself exists to
/// prevent, and the count is returned rather than pinned here because only the
/// template has a number worth pinning.
///
/// WHITESPACE-EXACT in two places (`# ` and ` = `), which is what the
/// template's own count is a fence around: a text writing `key= value` on a run
/// of lines drops exactly that run and nothing else says so.
#[cfg(test)]
pub(crate) fn documented_keys_the_roster_serves(text: &str) -> usize {
    let mut table = String::new();
    let mut found = 0;
    for line in text.lines() {
        let bare = line.strip_prefix("# ").unwrap_or(line);
        if let Some(heading) = bare
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            table = heading.to_string();
            continue;
        }
        let Some((key, _)) = bare.split_once(" = ") else {
            continue;
        };
        if !key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            || key.is_empty()
        {
            continue;
        }
        // A nested table carries the operator's own name; the roster holds
        // the prefix, the way the refusals do.
        let roster_table = match table.split('.').collect::<Vec<_>>()[..] {
            ["lights", "lamp" | "room" | "zone", ..] => TARGET_KEYS.to_string(),
            _ => table.clone(),
        };
        let serves = keys_of(&roster_table)
            .unwrap_or_else(|| panic!("it writes `[{table}]`, which no table serves"));
        assert!(
            serves.contains(&key),
            "it documents `{key}` under `[{table}]`, which does not serve it"
        );
        found += 1;
    }
    found
}

/// The roster row EVERY target declaration shares, whichever of the three
/// levels wrote it.
///
/// ONE ROW FOR THREE LEVELS, because the vocabulary is the same at all of them:
/// a lamp, a room and a zone answer the same questions and differ only in how
/// specific they are. Three rows would be one list to keep in agreement with
/// two others, which is the drift this roster exists to prevent.
pub(crate) const TARGET_KEYS: &str = "lights.<level>";

/// What one table serves, or `None` for a table this schema has no vocabulary
/// for (a plugin nothing registered; see `TABLE_KEYS`).
fn keys_of(table: &str) -> Option<&'static [&'static str]> {
    TABLE_KEYS
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, keys)| *keys)
}

/// Whether a table admits a key, refusing it BY NAME and with the whole
/// vocabulary spelled out when it does not.
///
/// THE TWO NAMES ARE DIFFERENT ARGUMENTS because a nested table's roster row
/// is a prefix and its refusal has to name the path the operator wrote: an
/// operator told `lights.<level>` has no `dim_windows` would go looking for a
/// table they never typed.
fn admits(roster_table: &str, shown_table: &str, key: &str) -> Result<(), ConfigError> {
    match keys_of(roster_table) {
        Some(serves) if !serves.contains(&key) => Err(unknown_key(roster_table, shown_table, key)),
        _ => Ok(()),
    }
}

/// `admits` for a table whose refusal names the table ITSELF, which is every
/// row but the two nested ones. The two-name form earns itself where the names
/// differ and reads as noise where they cannot, so the call site says which
/// case it is rather than repeating an argument.
fn admits_flat(table: &str, key: &str) -> Result<(), ConfigError> {
    admits(table, table, key)
}

/// The refusal itself, naming the table, the key, and the whole vocabulary.
///
/// THE LISTING IS THE POINT. A refusal that only says a key is unknown leaves
/// an operator guessing at the spelling, and guessing is what produced the
/// mistyped key; the alternatives are two words away in the same sentence.
fn unknown_key(roster_table: &str, shown_table: &str, key: &str) -> ConfigError {
    ConfigError::Invalid(format!(
        "unknown `{shown_table}` key `{key}`; the table serves {}",
        keys_of(roster_table).unwrap_or_default().join(", ")
    ))
}

/// The whole parsed file. Ordered, so listings and errors are deterministic.
///
/// THE DEFAULT IS WRITTEN OUT rather than derived, for `Recap`'s own reason
/// one type up: `daemon_enabled` is true when nothing says otherwise, and a
/// derived bool would read false and take the clock away from every machine
/// whose config was written before the table existed.
#[derive(Debug, PartialEq)]
pub struct Config {
    pub plugins: BTreeMap<String, PluginEntry>,
    pub recap: Recap,
    /// `[focus] silence`: the Focus MODE NAMES that mean it, each written
    /// either as the name Control Center shows or as a raw `modeIdentifier`.
    ///
    /// EMPTY IS THE FEATURE OFF, which is what makes the table optional and
    /// what every machine that never wrote one gets. There is no `enabled`
    /// key: naming no mode and switching the feature off are the same
    /// statement, and a second way to say it is a second thing to disagree.
    pub focus_silence: Vec<String>,
    /// `[daemon] enabled`: whether `pns daemon run` stays up and ticks.
    ///
    /// DEFAULT ON, which is the opposite of `[focus]` and of every plugin, and
    /// the difference is that this switch delivers nothing. An idle daemon
    /// reads one empty directory a second. Default OFF would put every feature
    /// that rides the clock behind TWO switches, so an operator who enabled the
    /// feature and saw nothing would have to discover a second, invisible one.
    pub daemon_enabled: bool,
    /// `[nag] after_secs`: how long an unanswered approval waits before it is
    /// carded a second time, in seconds. ZERO IS THE FEATURE OFF.
    ///
    /// ONE KEY THAT IS THE SWITCH AND THE SCHEDULE, which is `[focus]
    /// silence`'s own precedent: naming no schedule and switching off are one
    /// statement, so there is no second `enabled` key that can disagree with
    /// the first.
    ///
    /// DEFAULT OFF, unlike `[daemon]` beside it, and the difference is that
    /// this one INTERRUPTS. It also needs three separate operator steps before
    /// it works (an apply for the hook declaration, the daemon running, and
    /// this key), and a default-on feature that silently does nothing until all
    /// three are done is a mystery rather than a default.
    pub nag_after_secs: u64,
    /// `[lights]`: the lamp policy, or None when no table was written.
    ///
    /// BOXED because it is the largest thing in here and almost no machine has
    /// one: measured, the table is 72 of this struct's bytes and the whole
    /// config travels by value inside `LoadOutcome`, whose empty `Missing`
    /// variant would then be paying for a table that is usually absent.
    pub lights: Option<Box<Lights>>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            plugins: BTreeMap::new(),
            recap: Recap::default(),
            focus_silence: Vec::new(),
            daemon_enabled: DEFAULT_DAEMON_ENABLED,
            nag_after_secs: NAG_OFF,
            lights: None,
        }
    }
}

/// See `Config::daemon_enabled`.
const DEFAULT_DAEMON_ENABLED: bool = true;

/// The schedule that means the nag is off. See `Config::nag_after_secs`.
const NAG_OFF: u64 = 0;

/// Why a config could not be used. Every variant carries the offender by
/// name, because "config invalid" without a noun is a hunt.
#[derive(Debug, PartialEq)]
pub enum ConfigError {
    /// The file exists but is not TOML.
    Malformed(String),
    /// The TOML is well-formed but violates the schema.
    Invalid(String),
    /// The file exists and could not be read.
    Unreadable(String),
}

impl ConfigError {
    /// What went wrong, already sanitized for printing. Each mode wraps it in
    /// the sentence describing what IT did about it.
    pub fn detail(&self) -> &str {
        match self {
            ConfigError::Malformed(detail)
            | ConfigError::Invalid(detail)
            | ConfigError::Unreadable(detail) => detail,
        }
    }
}

/// What loading found at the path. `Missing` is deliberately not an error:
/// an unconfigured machine is a state to report, not a fault to diagnose.
#[derive(Debug, PartialEq)]
pub enum LoadOutcome {
    Missing,
    Loaded(Config),
}

/// Where the config lives for a given home directory. Pure, so the path rule
/// is testable without an environment.
pub fn config_path(home: &str) -> PathBuf {
    Path::new(home).join(".config/pns/config.toml")
}

/// The pure half: text in, config or a named refusal out.
pub fn parse_config(text: &str) -> Result<Config, ConfigError> {
    // The parser's Display echoes the offending source line, and this file
    // carries plugin secrets into log lines, so the refusal is rebuilt from
    // the cause and the location alone.
    let document: toml::Table = text.parse().map_err(|error: toml::de::Error| {
        let line = error
            .span()
            .map(|span| text[..span.start].matches('\n').count() + 1);
        ConfigError::Malformed(match line {
            Some(line) => format!("{} at line {line}", error.message()),
            None => error.message().to_string(),
        })
    })?;

    let mut config = Config::default();
    // SIX ADMITTED KEYS AND NO MORE. The arm below is the whole schema at
    // this level, and everything that is not one of the six is still refused
    // BY NAME, so a retired table and a plural typo both say what they are.
    for (key, value) in document {
        match key.as_str() {
            "recap" => config.recap = parse_recap(value)?,
            "focus" => config.focus_silence = parse_focus(value)?,
            "daemon" => config.daemon_enabled = parse_daemon(value)?,
            "nag" => config.nag_after_secs = parse_nag(value)?,
            "lights" => config.lights = Some(Box::new(parse_lights(value)?)),
            "plugins" => {
                let toml::Value::Table(plugins) = value else {
                    return Err(ConfigError::Invalid("`plugins` is not a table".to_string()));
                };

                for (name, entry) in plugins {
                    let toml::Value::Table(mut settings) = entry else {
                        return Err(ConfigError::Invalid(format!(
                            "plugin `{name}` is not a table"
                        )));
                    };
                    // `enabled` is removed rather than read, so the flag
                    // reaches this layer and everything left over reaches the
                    // plugin untouched.
                    let enabled = match settings.remove("enabled") {
                        None => false,
                        Some(toml::Value::Boolean(flag)) => flag,
                        Some(_) => {
                            return Err(ConfigError::Invalid(format!(
                                "plugin `{name}` has a non-boolean `enabled`"
                            )));
                        }
                    };
                    // AND THE SETTINGS ARE JUDGED for a plugin that ships,
                    // because a near miss there is a destination that quietly
                    // never works. `enabled` is already out of the table and
                    // still listed, since it is a key the operator writes.
                    let table = format!("plugins.{name}");
                    for key in settings.keys() {
                        admits_flat(&table, key)?;
                    }
                    config
                        .plugins
                        .insert(name, PluginEntry { enabled, settings });
                }
            }
            _ => {
                // AND THE SIX ARE LISTED, off the roster's own top-level row.
                // This is the most operator-visible typo class there is (a
                // whole table misspelled, or a table that MOVED, which refuses
                // the file whole and takes every plugin's secret with it), and
                // it was the last refusal in this file that named no
                // alternatives.
                return Err(ConfigError::Invalid(format!(
                    "unknown top-level key `{key}`; the file serves {}",
                    keys_of(TOP_LEVEL).unwrap_or_default().join(", ")
                )));
            }
        }
    }
    Ok(config)
}

/// `[recap]`'s switches, each starting at its default and moved only by a key
/// that states it.
fn parse_recap(value: toml::Value) -> Result<Recap, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`recap` is not a table".to_string()));
    };
    let mut recap = Recap::default();
    // ONE ARM PER KEY, and the three that are not booleans read through a
    // function of their own, so each refusal sits with the shape it judges.
    for (key, setting) in table {
        // BELT AND BRACES HERE, DELIBERATELY, and this is the first of the five
        // top-level tables it reads that way. The `_` arm below refuses the
        // same key with the same sentence for as long as the roster and the
        // arms AGREE, so removing this line changes nothing observable today
        // and a mutation of it survives the suite. Its whole effect is what
        // happens when the two stop agreeing: a key added to an arm and not to
        // the roster stops working at its own feature test instead of quietly
        // working while every refusal listing omits it. Do not delete the five
        // as redundant; the plugin tables have no `_` arm at all, and there the
        // gate is the only check.
        admits_flat("recap", &key)?;
        match key.as_str() {
            "min_events" => recap.min_events = threshold(&setting)?,
            "repos" => recap.repos = repositories(&setting)?,
            "review_notes" => recap.review_notes = Some(note_glob(&setting)?),
            "summarizer" => recap.summarizer = Some(argv(&setting)?),
            "summarizer_deadline_secs" => recap.summarizer_deadline_secs = seconds(&setting)?,
            "replay_card" => recap.replay_card = flag(&key, &setting)?,
            "digest" => recap.digest = flag(&key, &setting)?,
            "digest_as_thread" => recap.digest_as_thread = flag(&key, &setting)?,
            _ => {
                return Err(unknown_key("recap", "recap", &key));
            }
        }
    }
    Ok(recap)
}

/// `[focus]`'s one key: the Focus modes that mean it.
///
/// NO `enabled` KEY, deliberately. Naming no mode and switching the feature off
/// are the same statement, so a second way to say it is a second thing that can
/// disagree with the first: a table reading `enabled = true` with an empty
/// `silence`, or `enabled = false` with three modes listed, would each need a
/// rule nobody has stated.
///
/// AN EMPTY LIST IS NOT REFUSED, unlike `recap`'s empty `summarizer` and
/// `repos`. Those name a thing pns would then try and fail to use; this names
/// the modes that silence, and none of them is a working, readable setting that
/// says exactly what it does.
fn parse_focus(value: toml::Value) -> Result<Vec<String>, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`focus` is not a table".to_string()));
    };
    let mut silence = Vec::new();
    for (key, setting) in table {
        admits_flat("focus", &key)?;
        match key.as_str() {
            "silence" => silence = modes(&setting)?,
            _ => {
                return Err(unknown_key("focus", "focus", &key));
            }
        }
    }
    Ok(silence)
}

/// `[daemon]`'s one switch, in `parse_focus`'s shape: an unknown key inside
/// the table and a value of the wrong type are each refused BY NAME, rather
/// than half-read into a clock the operator believes they turned off.
fn parse_daemon(value: toml::Value) -> Result<bool, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`daemon` is not a table".to_string()));
    };
    let mut enabled = DEFAULT_DAEMON_ENABLED;
    for (key, setting) in table {
        admits_flat("daemon", &key)?;
        match key.as_str() {
            "enabled" => {
                enabled = setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`daemon` key `enabled` has type `{}`, not boolean",
                        setting.type_str()
                    ))
                })?;
            }
            _ => {
                return Err(unknown_key("daemon", "daemon", &key));
            }
        }
    }
    Ok(enabled)
}

/// `[nag]`'s one key, in `parse_daemon`'s shape: an unknown key inside the
/// table and a value of the wrong shape are each refused BY NAME rather than
/// half-read into a schedule the operator believes they set.
fn parse_nag(value: toml::Value) -> Result<u64, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`nag` is not a table".to_string()));
    };
    let mut after_secs = NAG_OFF;
    for (key, setting) in table {
        admits_flat("nag", &key)?;
        match key.as_str() {
            "after_secs" => after_secs = nag_schedule(&setting)?,
            _ => {
                return Err(unknown_key("nag", "nag", &key));
            }
        }
    }
    Ok(after_secs)
}

/// `after_secs`, in whole seconds, BOUNDED ON BOTH SIDES with zero carved out.
///
/// ZERO IS NOT A SCHEDULE AND IS NOT AN ERROR: it is the same statement as
/// writing no table, which is what makes this key the switch as well as the
/// timing. Every other value under the floor IS an error, because it is a
/// schedule the operator meant and pns will not run.
///
/// THE FLOOR IS THIRTY SECONDS. A nudge arriving before the operator could
/// plausibly have picked up their phone is the stacking this design forbids,
/// and thirty is low enough that the feature can be drilled in half a minute.
///
/// THE CEILING IS AN HOUR, mirroring `MAX_SUMMARIZER_DEADLINE_SECS` rather than
/// any harness number. It must also sit inside the daemon's own registration
/// window (`daemon::DUE_WINDOW_SECS`, thirty days), which it does with room to
/// spare, and it is what keeps `2 * after_secs` in the staleness cap far from
/// any arithmetic edge.
///
/// REFUSED RATHER THAN CLAMPED, in `min_events`'s style: a silently corrected
/// schedule is a schedule the operator believes they set.
fn nag_schedule(setting: &toml::Value) -> Result<u64, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`nag` key `after_secs` has type `{}`, not a count of seconds",
            setting.type_str()
        )));
    };
    if count == NAG_OFF {
        return Ok(NAG_OFF);
    }
    if !(MIN_NAG_AFTER_SECS..=MAX_NAG_AFTER_SECS).contains(&count) {
        return Err(ConfigError::Invalid(format!(
            "`nag` key `after_secs` is {count}, outside the {MIN_NAG_AFTER_SECS} to \
             {MAX_NAG_AFTER_SECS} second range; 0 is the feature off"
        )));
    }
    Ok(count)
}

/// The shortest nag anyone may schedule. See `nag_schedule`.
const MIN_NAG_AFTER_SECS: u64 = 30;

/// The longest. See `nag_schedule`.
const MAX_NAG_AFTER_SECS: u64 = MAX_SUMMARIZER_DEADLINE_SECS;

/// `silence`, the Focus modes that mean it: a list of display NAMES as Control
/// Center shows them, raw `modeIdentifier` strings, or a mix.
///
/// THE LIST MAY BE EMPTY AND AN ENTRY MAY NOT, which is not two rules but one
/// applied to two different statements. An empty list says "no mode silences
/// pns", which is the feature off and exactly what it reads as. An empty
/// STRING says nothing at all: no Focus mode is named by it, so it is a policy
/// the operator wrote and pns would never act on. That is precisely the state
/// the misspelled-key refusal one function up exists to prevent, and `repos`
/// refuses its own empty entry by name for the same reason.
///
/// THE NAME ITSELF IS NOT JUDGED BEYOND THAT. A name that matches no mode is
/// an ordinary thing to write (a Focus you keep on another Mac), and `pns
/// doctor` is where an operator learns whether the mode they named is the one
/// that is on.
fn modes(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let names = strings("focus", "silence", "a list of Focus mode names", setting)?;
    if names.iter().any(String::is_empty) {
        return Err(ConfigError::Invalid(
            "`focus` key `silence` names a mode that is the empty string, which is no Focus at all"
                .to_string(),
        ));
    }
    Ok(names)
}

/// `[lights]`, the lamp policy: one interval, five behaviour shapes and three
/// levels of routing, each starting at its default and moved only by a key that
/// states it.
///
/// EVERY UNKNOWN KEY IS REFUSED BY NAME, and that refusal is the whole argument
/// for parsing this table here rather than inside the hue plugin. A plugin's
/// settings are free-form at this layer, so a mistyped key there is silently
/// ignored; the failure that costs is a lamp that never lights and a config
/// that looks right, which an operator standing in a dark room cannot falsify.
fn parse_lights(value: toml::Value) -> Result<Lights, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`lights` is not a table".to_string()));
    };
    let mut lights = Lights::default();
    for (key, setting) in table {
        admits_flat("lights", &key)?;
        match key.as_str() {
            "refresh_secs" => {
                lights.refresh_secs =
                    bounded("lights", &key, &setting, MIN_REFRESH_SECS, MAX_REFRESH_SECS)?;
            }
            "done" => lights.done = parse_pulse("lights.done", &setting, lights.done)?,
            "failed" => lights.failed = parse_pulse("lights.failed", &setting, lights.failed)?,
            "blocked" => {
                lights.blocked = parse_blocked(&setting, lights.blocked)?;
            }
            "dim" => lights.dim = parse_breath("lights.dim", &setting, lights.dim)?,
            "unread" => lights.unread = parse_unread(&setting, lights.unread)?,
            "loop" => lights.looping = parse_looping(&setting, lights.looping)?,
            "lamp" => lights.lamps = parse_targets("lamp", &setting)?,
            "room" => lights.rooms = parse_targets("room", &setting)?,
            "zone" => lights.zones = parse_targets("zone", &setting)?,
            _ => {
                return Err(unknown_key("lights", "lights", &key));
            }
        }
    }
    Ok(lights)
}

/// The keys a behaviour's own table serves, read into whichever of the shapes
/// that behaviour has.
///
/// THE DEFAULT ARRIVES AS A VALUE rather than being rebuilt here, so a table
/// that states one key moves that one and leaves the rest where the locked
/// figures put them.
fn parse_pulse(
    where_it_is: &str,
    setting: &toml::Value,
    mut pulse: Pulse,
) -> Result<Pulse, ConfigError> {
    for (key, stated) in behaviour_table(where_it_is, setting)? {
        admits_flat(where_it_is, key)?;
        match key.as_str() {
            "duration_ms" => {
                pulse.duration_ms = bounded(where_it_is, key, stated, MIN_FADE_MS, MAX_FADE_MS)?;
            }
            "brightness" => pulse.brightness = percent(where_it_is, key, stated)?,
            _ => return Err(unknown_key(where_it_is, where_it_is, key)),
        }
    }
    Ok(pulse)
}

fn parse_breath(
    where_it_is: &str,
    setting: &toml::Value,
    mut breath: Breath,
) -> Result<Breath, ConfigError> {
    for (key, stated) in behaviour_table(where_it_is, setting)? {
        admits_flat(where_it_is, key)?;
        breath_key(where_it_is, key, stated, &mut breath)?;
    }
    ends_agree(where_it_is, &breath)?;
    Ok(breath)
}

fn parse_blocked(setting: &toml::Value, mut blocked: Blocked) -> Result<Blocked, ConfigError> {
    const WHERE: &str = "lights.blocked";
    for (key, stated) in behaviour_table(WHERE, setting)? {
        admits_flat(WHERE, key)?;
        if key == "give_up_after_secs" {
            blocked.give_up_after_secs = bounded(
                WHERE,
                key,
                stated,
                MIN_LEASE_TIMEOUT_SECS,
                MAX_GIVE_UP_AFTER_SECS,
            )?;
            continue;
        }
        breath_key(WHERE, key, stated, &mut blocked.breath)?;
    }
    ends_agree(WHERE, &blocked.breath)?;
    Ok(blocked)
}

fn parse_unread(setting: &toml::Value, mut unread: Unread) -> Result<Unread, ConfigError> {
    const WHERE: &str = "lights.unread";
    for (key, stated) in behaviour_table(WHERE, setting)? {
        admits_flat(WHERE, key)?;
        if key == "after_secs" {
            // ZERO IS ALLOWED AND MEANS "AT ONCE", which is the failure
            // flavour's own behaviour spelled for the success one. It is not a
            // switch that turns anything off, so it needs no floor.
            unread.after_secs = bounded(WHERE, key, stated, 0, MAX_THRESHOLD_SECS)?;
            continue;
        }
        breath_key(WHERE, key, stated, &mut unread.breath)?;
    }
    ends_agree(WHERE, &unread.breath)?;
    Ok(unread)
}

fn parse_looping(setting: &toml::Value, mut looping: Looping) -> Result<Looping, ConfigError> {
    const WHERE: &str = "lights.loop";
    for (key, stated) in behaviour_table(WHERE, setting)? {
        admits_flat(WHERE, key)?;
        match key.as_str() {
            "threshold_secs" => {
                looping.threshold_secs =
                    bounded(WHERE, key, stated, MIN_THRESHOLD_SECS, MAX_THRESHOLD_SECS)?;
            }
            "lease_timeout_secs" => {
                looping.lease_timeout_secs = bounded(
                    WHERE,
                    key,
                    stated,
                    MIN_LEASE_TIMEOUT_SECS,
                    MAX_THRESHOLD_SECS,
                )?;
            }
            _ => breath_key(WHERE, key, stated, &mut looping.breath)?,
        }
    }
    ends_agree(WHERE, &looping.breath)?;
    Ok(looping)
}

/// The three keys every breathing shape shares, so `unread` and `loop` read
/// them through the same arm the two plain breaths do.
fn breath_key(
    where_it_is: &str,
    key: &str,
    stated: &toml::Value,
    breath: &mut Breath,
) -> Result<(), ConfigError> {
    match key {
        "duration_ms" => {
            breath.duration_ms = bounded(where_it_is, key, stated, MIN_FADE_MS, MAX_FADE_MS)?;
        }
        "high" => breath.high = percent(where_it_is, key, stated)?,
        "low" => breath.low = percent(where_it_is, key, stated)?,
        _ => return Err(unknown_key(where_it_is, where_it_is, key)),
    }
    Ok(())
}

/// A breath whose `low` is above its `high` is REFUSED rather than rendered
/// upside down, because the driver's stated invariant is that it stops at the
/// peak: with the ends swapped it would stop at the fainter of the two and the
/// next tick would resume from there, which is the opposite of what the shape
/// promises.
fn ends_agree(where_it_is: &str, breath: &Breath) -> Result<(), ConfigError> {
    if breath.low > breath.high {
        return Err(ConfigError::Invalid(format!(
            "`{where_it_is}` has low {} above high {}, so the breath would stop at \
             its faintest rather than at its peak",
            breath.low, breath.high
        )));
    }
    Ok(())
}

/// One behaviour's own table, refused by name when it is not a table at all.
fn behaviour_table<'setting>(
    where_it_is: &str,
    setting: &'setting toml::Value,
) -> Result<&'setting toml::map::Map<String, toml::Value>, ConfigError> {
    setting.as_table().ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{where_it_is}` has type `{}`, not a table of settings",
            setting.type_str()
        ))
    })
}

/// One brightness, in percent, refused by name outside the range.
fn percent(where_it_is: &str, key: &str, stated: &toml::Value) -> Result<u8, ConfigError> {
    let count = bounded(
        where_it_is,
        key,
        stated,
        MIN_BRIGHTNESS.into(),
        MAX_BRIGHTNESS.into(),
    )?;
    // THE BOUND ABOVE ALREADY HELD, so this cannot fail and a fallback here
    // would be a second, silent answer to a question `bounded` has already
    // refused by name.
    Ok(u8::try_from(count).expect("bounded at MAX_BRIGHTNESS, which is a percent and fits a u8"))
}

/// `[lights.lamp]`, `[lights.room]` and `[lights.zone]`: one table per declared
/// name, at one of the three levels.
///
/// A NAME IS NOT JUDGED against the bridge here. This layer reads a file, and
/// only the bridge's own listings can say which lamps, rooms and zones exist;
/// an unresolvable name is reported by the tick and by `pns doctor` in their
/// own words, once there is a listing to judge it against.
fn parse_targets(
    level: &str,
    setting: &toml::Value,
) -> Result<BTreeMap<String, Target>, ConfigError> {
    let Some(table) = setting.as_table() else {
        return Err(ConfigError::Invalid(format!(
            "`lights` key `{level}` has type `{}`, not a table of {level} names",
            setting.type_str()
        )));
    };
    let mut targets = BTreeMap::new();
    for (name, entry) in table {
        let where_it_is = format!("lights.{level}.{name}");
        let Some(settings) = entry.as_table() else {
            return Err(ConfigError::Invalid(format!(
                "`{where_it_is}` has type `{}`, not a table of settings",
                entry.type_str()
            )));
        };
        let mut target = Target::default();
        let mut states_behaviours = false;
        for (key, stated) in settings {
            admits(TARGET_KEYS, &where_it_is, key)?;
            match key.as_str() {
                "shows" => target.shows = Some(behaviours(&where_it_is, key, stated)?),
                "dim_window" => target.dim_window = Some(text(&where_it_is, key, stated)?),
                "dim_behaviours" => {
                    target.dim_behaviours = behaviours(&where_it_is, key, stated)?;
                    states_behaviours = true;
                }
                _ => {
                    return Err(unknown_key(TARGET_KEYS, &where_it_is, key));
                }
            }
        }
        // NO DEAD KNOBS, which is the config ruling reaching the one pair of
        // keys that can be half written. The enables RIDE the window (they are
        // resolved as one answer), so a declaration that names which behaviours
        // run dimmed and never says WHEN is a list nothing reads: the operator
        // gets a lamp that strobes all night and a file that says it should
        // not. STATED rather than non-empty, because an empty list with no
        // window is the same dead knob and the two must not disagree.
        if states_behaviours && target.dim_window.is_none() {
            return Err(ConfigError::Invalid(format!(
                "`{where_it_is}` states `dim_behaviours` with no `dim_window` for \
                 them to run in, so nothing would ever read them"
            )));
        }
        targets.insert(name.clone(), target);
    }
    Ok(targets)
}

/// A list of behaviour words off the closed set, refused BY NAME.
///
/// THE REFUSAL LISTS THE WHOLE SET, which is worth the extra words here and
/// nowhere else in this file: the failure it prevents is a lamp that stays dark
/// while the operator is sure they routed it, and their only evidence is a lamp
/// doing nothing.
fn behaviours(
    where_it_is: &str,
    key: &str,
    stated: &toml::Value,
) -> Result<Vec<Behaviour>, ConfigError> {
    let words = strings(where_it_is, key, "a list of behaviour names", stated)?;
    words
        .iter()
        .map(|word| {
            BEHAVIOUR_WORDS
                .iter()
                .find(|(spelling, _)| spelling == word)
                .map(|(_, behaviour)| *behaviour)
                .ok_or_else(|| {
                    let known: Vec<&str> = BEHAVIOUR_WORDS.iter().map(|(word, _)| *word).collect();
                    ConfigError::Invalid(format!(
                        "`{where_it_is}` key `{key}` names `{word}`, which is no behaviour; \
                         the lamps say {}",
                        known.join(", ")
                    ))
                })
        })
        .collect()
}

/// One setting that has to be a string, refused BY NAME and BY TYPE.
fn text(where_it_is: &str, key: &str, stated: &toml::Value) -> Result<String, ConfigError> {
    stated.as_str().map(str::to_string).ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{where_it_is}` key `{key}` has type `{}`, not a string",
            stated.type_str()
        ))
    })
}

/// One `[lights]` scalar, refused BY NAME outside its range.
///
/// BOTH ENDS, ALWAYS. A floor alone leaves a value that parses and cannot work;
/// a ceiling alone leaves the same at the other end. Each bound is argued at
/// the constant that holds it, and the refusal echoes both, so an operator
/// reading it learns the range rather than only that they missed it.
///
/// THE TABLE IS AN ARGUMENT because the behaviour tables are nested: an
/// operator told `lights` has no `high` would go looking in the wrong heading.
fn bounded(
    table: &str,
    key: &str,
    setting: &toml::Value,
    low: u64,
    high: u64,
) -> Result<u64, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` has type `{}`, not a count between {low} and {high}",
            setting.type_str()
        )));
    };
    if count < low || count > high {
        return Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` is {count}, outside the {low} to {high} range"
        )));
    }
    Ok(count)
}

/// One `[recap]` switch. A value of any other type is refused BY NAME rather
/// than read as its own truthiness, which is what would leave a delivery on
/// while its config said otherwise.
fn flag(key: &str, setting: &toml::Value) -> Result<bool, ConfigError> {
    setting.as_bool().ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`recap` key `{key}` has type `{}`, not boolean",
            setting.type_str()
        ))
    })
}

/// `min_events`, the volume threshold. A negative or fractional value is
/// refused BY NAME rather than clamped: the operator asked for a threshold, and
/// a silently corrected one is a threshold they believe they set.
fn threshold(setting: &toml::Value) -> Result<usize, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| usize::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `min_events` has type `{}`, not a count",
            setting.type_str()
        )));
    };
    // AND ZERO IS REFUSED BY NAME TOO, for the same reason a negative is.
    // `counted.len() >= 0` is always true, so a zero threshold recaps EVERY
    // event, including one over an empty window, which is the state
    // `recap::NOTHING_HAPPENED` says the event path never posts. An operator
    // calibrating the knob downward gets a card and a Discord recap on every
    // event, each saying nothing was recorded. One is the floor: it means "any
    // activity at all".
    if count == 0 {
        return Err(ConfigError::Invalid(
            "`recap` key `min_events` is 0, which is not a threshold; 1 is the floor".to_string(),
        ));
    }
    Ok(count)
}

/// `summarizer_deadline_secs`, in whole seconds. See `min_events` for why a
/// value that is not a count is refused rather than corrected; zero is a
/// deadline nothing can meet, which is the fallback saying so, not a shape
/// this layer has to judge.
///
/// THE TOP END IS REFUSED BY NAME TOO, and unlike zero it really is a shape
/// this layer has to judge. Two things break past the ceiling and neither is
/// visible where it happens. NOTHING SUPERVISES THE DETACHED RECAP CHILD, which
/// `spawn_recap` states outright: at four minutes that is fine, and at a day it
/// is one child plus one wedged backend held for a day, with a second pair
/// arriving at the next return moment. AND `9223372036854775807` IS A PLAIN
/// TOML INTEGER: it parses, and `Instant::now() + Duration::from_secs` of it
/// PANICS (MEASURED: "overflow when adding duration to instant") inside a
/// process whose stderr is /dev/null and whose exit code nobody reads, so the
/// recap simply vanishes after the card has said it is coming. A refusal the
/// operator reads beats a silence they cannot.
fn seconds(setting: &toml::Value) -> Result<u64, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `summarizer_deadline_secs` has type `{}`, not a count of seconds",
            setting.type_str()
        )));
    };
    if count > MAX_SUMMARIZER_DEADLINE_SECS {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `summarizer_deadline_secs` is {count}, past the \
             {MAX_SUMMARIZER_DEADLINE_SECS}-second ceiling"
        )));
    }
    Ok(count)
}

/// `summarizer`, the command the window is handed to: a list of WORDS, passed
/// to the process directly and never through a shell.
///
/// THE SHELL STRING IS THE MISTAKE THIS REFUSES. `summarizer = "ollama run
/// qwen3.5:4b"` is what a hand writes first, and reading it as a one-word
/// command would name a binary nobody has, so it is refused by name rather than
/// left to fail once a night inside a detached process.
///
/// AN EMPTY LIST IS REFUSED TOO, because it names no command at all: taken as
/// written it would leave the summarizer configured and unrunnable, which reads
/// to the operator as a summarizer that is not answering rather than as a table
/// they have to fix.
///
/// AND SO IS AN EMPTY FIRST WORD, on that same reasoning rather than a new one.
/// `[""]` parses, `Command::new("")` fails to spawn, and the operator gets
/// precisely the outcome the paragraph above exists to prevent. Only the first
/// word is judged: an empty ARGUMENT is a real thing to pass a program, and
/// nothing about it stops the command running.
fn argv(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let words = strings("recap", "summarizer", "a list of command words", setting)?;
    if words.is_empty() {
        return Err(ConfigError::Invalid(
            "`recap` key `summarizer` is empty, so it names no command to run".to_string(),
        ));
    }
    if words[0].is_empty() {
        return Err(ConfigError::Invalid(
            "`recap` key `summarizer` starts with an empty word, so it names no command to run"
                .to_string(),
        ));
    }
    Ok(words)
}

/// `repos`, the repositories the merged pull requests are read from: a list of
/// names in `gh`'s own `OWNER/REPO` spelling, passed to it as one argument
/// each.
///
/// EMPTINESS IS REFUSED AT BOTH LEVELS, for `summarizer`'s reason rather than a
/// new one. A key present with no name under it, or a name that is the empty
/// string, would leave the section reading "nothing merged in this window" over
/// a night that merged plenty, and the operator would be looking at their
/// repository rather than at their config.
///
/// THE NAME ITSELF IS NOT JUDGED BEYOND THAT, and deliberately. `gh` accepts
/// `OWNER/REPO`, `HOST/OWNER/REPO` and a full URL, it is the authority on which
/// of those exist, and a shape rule written here would refuse a spelling that
/// works. It is passed as ARGV, so nothing in it can be read as syntax by
/// anything; a name `gh` does not know costs the section one "unavailable"
/// line, which is the same rung a missing `gh` takes.
fn repositories(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let names = strings("recap", "repos", "a list of repository names", setting)?;
    if names.is_empty() || names.iter().any(String::is_empty) {
        return Err(ConfigError::Invalid(
            "`recap` key `repos` names no repository to read".to_string(),
        ));
    }
    Ok(names)
}

/// One key holding a list of plain strings, with the TABLE, the key and what
/// the list is FOR named in every refusal. The emptiness rules belong to the
/// callers, because what an empty list MEANS is theirs.
///
/// THE TABLE IS AN ARGUMENT because two tables now hold list keys, and a
/// refusal that named only the key would send an operator with both a `[recap]`
/// and a `[focus]` table looking in the wrong one.
fn strings(
    table: &str,
    key: &str,
    noun: &str,
    setting: &toml::Value,
) -> Result<Vec<String>, ConfigError> {
    let Some(values) = setting.as_array() else {
        return Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` has type `{}`, not {noun}",
            setting.type_str()
        )));
    };
    values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "`{table}` key `{key}` has a `{}` in it, not {noun}",
                    value.type_str()
                ))
            })
        })
        .collect()
}

/// `review_notes`, the one pattern deciding which files the recap may open.
///
/// THE GLOB IS THE WHOLE PERMISSION, which is why its shape is judged here
/// rather than resolved generously at the read. Two spellings are refused by
/// name and each would widen what pns opens beyond what the operator wrote:
///
/// A RELATIVE PATH resolves against the working directory, and the recap is
/// rendered by a process started from whatever directory the return event fired
/// in. The same key would then name a different set of files on every run, so
/// only an absolute path and a `~/` one are admitted.
///
/// AND A `*` IN A DIRECTORY makes the set of DIRECTORIES a search rather than a
/// statement: `~/.claude/*/checklist-*.md` asks pns to walk directories nobody
/// listed. Only the file name may hold one, which keeps the read to a single
/// directory the operator named in full.
fn note_glob(setting: &toml::Value) -> Result<String, ConfigError> {
    let Some(pattern) = setting.as_str() else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` has type `{}`, not a path with a file name in it",
            setting.type_str()
        )));
    };
    let (directory, name) = pattern.rsplit_once('/').unwrap_or(("", pattern));
    if name.is_empty() {
        return Err(ConfigError::Invalid(
            "`recap` key `review_notes` names no file to read".to_string(),
        ));
    }
    if !pattern.starts_with('/') && !pattern.starts_with("~/") {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` is `{pattern}`, which is not an absolute path or a `~/` one"
        )));
    }
    if directory.contains('*') {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` is `{pattern}`, and only its file name may hold a `*`"
        )));
    }
    // AND EXACTLY ONE OF THEM, because that is all the matcher reads. A second
    // `*` is matched LITERALLY, so a pattern carrying one silently matches
    // nothing at all, which is the outcome every refusal in this file exists to
    // turn into a sentence the operator can act on.
    if name.matches('*').count() > 1 {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `review_notes` is `{pattern}`, and its file name may hold only one `*`"
        )));
    }
    Ok(pattern.to_string())
}

/// The IO edge: read the file at `path` and hand its text to the parser.
pub fn load_config(path: &Path) -> Result<LoadOutcome, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_config(&text).map(LoadOutcome::Loaded),
        // A dangling symlink also reads NotFound, and chezmoi deploys configs
        // as symlinks: the entry is PRESENT with a wrong target, so only an
        // absent entry is Missing and the broken link is an error.
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && std::fs::symlink_metadata(path).is_err() =>
        {
            Ok(LoadOutcome::Missing)
        }
        Err(error) => Err(ConfigError::Unreadable(format!(
            "{}: {error}",
            path.display()
        ))),
    }
}

/// The `[plugins.mobile]` settings, but ONLY when the table is ARMED: switched
/// on, and naming a backend this binary answers.
///
/// THE ONE READER OF THAT TABLE. The push token, the watching-card toggle and
/// the submission deadline all come through here, so a table naming a backend
/// nothing implements contributes no settings ANYWHERE, rather than being
/// refused on the paths that remembered to ask and honoured on the ones that
/// did not. A `submit_deadline_secs` written under `type = "pushover"` is a
/// number for a backend this binary has never heard of, and reading it as
/// moshi's is exactly the misattribution `type` exists to stop.
///
/// `Ok(None)` IS THE INERT TABLE: absent, or present with the switch off. That
/// is the reading `enabled_hue_table` and `home::enabled_router_table` already
/// give their own, and it covers the table's SETTINGS too, the deadline
/// included: one switch, one answer, rather than a per-key exception nobody
/// could predict from the flag they set. Nothing complains about it either,
/// because a line about a channel the operator turned off, on every event, is
/// noise. `pns doctor` is where a switched-off table naming no backend is
/// still made visible.
///
/// `Err` CARRIES THE REASON, never a bare `None`. A caller that collapsed the
/// two reported a missing token for a fault that was the type, and sent an
/// operator whose token was already correct to go and check it.
pub fn armed_mobile(config: &Config) -> Result<Option<&toml::Table>, String> {
    let Some(mobile) = config.plugins.get("mobile").filter(|mobile| mobile.enabled) else {
        return Ok(None);
    };
    crate::channels::moshi::mobile_backend(&mobile.settings)?;
    Ok(Some(&mobile.settings))
}

/// How long pns waits for moshi to acknowledge a submission, from
/// `[plugins.mobile] submit_deadline_secs`, with the default when no key states
/// one.
///
/// IT IS READ OFF THE ARMED MOBILE TABLE and nowhere else, through
/// `armed_mobile`. Plugin settings reach this layer free-form, so every
/// plugin's table would answer a key spelled this way, and a reader that asked
/// the wrong one would take a number the operator wrote for something else.
/// The backend is part of that same question: a deadline under a table naming
/// no compiled-in backend is refused rather than read as moshi's.
///
/// THE REFUSALS ARE LOUD AND NAMED, because the caller falls back to the
/// default and a silent fallback is the operator asking for something, not
/// getting it, and being told nothing.
pub fn submit_deadline(config: &Config) -> Result<Duration, ConfigError> {
    let Some(mobile) = armed_mobile(config).map_err(ConfigError::Invalid)? else {
        return Ok(Duration::from_secs(DEFAULT_SUBMIT_DEADLINE_SECS));
    };
    let Some(stated) = mobile.get("submit_deadline_secs") else {
        return Ok(Duration::from_secs(DEFAULT_SUBMIT_DEADLINE_SECS));
    };
    let Some(count) = stated
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`mobile` key `submit_deadline_secs` has type `{}`, not a count of seconds",
            stated.type_str()
        )));
    };
    if count == 0 {
        return Err(ConfigError::Invalid(
            "`mobile` key `submit_deadline_secs` is 0, which is the bound switched off by \
             accident: a deadline that expires before the daemon can answer costs the phone \
             card on every approval"
                .to_string(),
        ));
    }
    if count > MAX_SUBMIT_DEADLINE_SECS {
        return Err(ConfigError::Invalid(format!(
            "`mobile` key `submit_deadline_secs` is {count}, past the \
             {MAX_SUBMIT_DEADLINE_SECS}-second ceiling"
        )));
    }
    Ok(Duration::from_secs(count))
}

#[cfg(test)]
mod tests {
    use super::{
        Behaviour, Blocked, Breath, ConfigError, Lights, LoadOutcome, Looping, Pulse, Target,
        Unread, config_path, load_config, parse_config, submit_deadline,
    };
    use std::time::Duration;

    // --- path resolution ----------------------------------------------------

    #[test]
    fn the_config_lives_under_the_homes_dot_config_pns() {
        assert_eq!(
            config_path("/Users/operator"),
            std::path::PathBuf::from("/Users/operator/.config/pns/config.toml")
        );
    }

    // --- parsing and the schema ---------------------------------------------

    #[test]
    fn a_plugin_table_with_enabled_true_is_selected_and_keeps_its_settings() {
        let config = parse_config("[plugins.hue]\nenabled = true\nbridge = \"office\"\n").unwrap();
        let hue = &config.plugins["hue"];
        assert!(hue.enabled);
        assert_eq!(
            hue.settings.get("bridge").and_then(|v| v.as_str()),
            Some("office")
        );
        assert!(
            !hue.settings.contains_key("enabled"),
            "the selection flag is this layer's, not a setting"
        );
    }

    #[test]
    fn an_absent_enabled_flag_reads_disabled_because_selection_is_explicit() {
        let config = parse_config("[plugins.hue]\nbridge = \"office\"\n").unwrap();
        assert!(!config.plugins["hue"].enabled);
    }

    #[test]
    fn an_empty_config_is_valid_and_selects_nothing() {
        let config = parse_config("").unwrap();
        assert!(config.plugins.is_empty());
    }

    #[test]
    fn a_non_boolean_enabled_flag_is_refused_naming_the_plugin() {
        let err = parse_config("[plugins.hue]\nenabled = \"yes\"\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(message.contains("hue"), "the offender is named: {message}")
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_top_level_key_is_refused_so_a_typo_cannot_disable_a_channel() {
        // [plugin.hue] instead of [plugins.hue] must be a loud refusal, never
        // a quietly ignored table that leaves hue disabled.
        let err = parse_config("[plugin.hue]\nenabled = true\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("plugin"),
                    "the offender is named: {message}"
                )
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_plugin_entry_that_is_not_a_table_is_refused_naming_the_plugin() {
        let err = parse_config("[plugins]\nhue = true\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(message.contains("hue"), "the offender is named: {message}")
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_stale_top_level_home_table_is_refused_by_name_rather_than_ignored() {
        // The probe's settings moved into `[plugins.router]`. A config still
        // carrying `[home]` must be refused NAMING it, so the operator is sent
        // to the one table they have to move; admitting it as a key nothing
        // reads any more would leave `pns home` reporting "not configured"
        // beside a file that plainly configures it.
        let err = parse_config("[home]\nrouter_url = \"https://192.168.1.1\"\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(message.contains("home"), "the offender is named: {message}")
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_non_table_plugins_value_is_refused_naming_the_key() {
        // `plugins = 5` at the one key the whole file hangs off must refuse,
        // never parse to an empty config with everything silently disabled.
        let err = parse_config("plugins = 5\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("plugins"),
                    "the offender is named: {message}"
                )
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_line_is_reported_without_echoing_its_value() {
        // The config carries plugin secrets, and error strings travel to
        // logs: the refusal names where and why, never the line's contents.
        let err = parse_config("[plugins.mobile]\ntoken = \"SUPERSECRET\" trailing\n").unwrap_err();
        match err {
            ConfigError::Malformed(message) => {
                assert!(!message.is_empty(), "the cause is still named");
                assert!(
                    !message.contains("SUPERSECRET"),
                    "the offending line's value must not be echoed: {message}"
                );
            }
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn malformed_toml_is_a_loud_error_never_a_silent_empty_config() {
        // A config that fails to parse and quietly becomes "nothing enabled"
        // would turn every notification off with no trace.
        assert!(matches!(
            parse_config("not [ toml"),
            Err(ConfigError::Malformed(_))
        ));
    }

    // --- the recap's switches -----------------------------------------------

    #[test]
    fn a_recap_table_is_read_rather_than_refused_and_each_switch_stands_alone() {
        // ONE KEY STATED, THE OTHER TWO UNTOUCHED. The three deliveries are
        // independent, so an operator who silenced the recap must not find
        // they also silenced the catch-up card, or the other way round.
        let config = parse_config("[recap]\ndigest = false\n").unwrap();
        assert!(!config.recap.digest, "the stated switch was read");
        assert!(config.recap.replay_card, "the card kept its default");
        assert!(config.recap.digest_as_thread, "the thread kept its default");
    }

    #[test]
    fn a_config_with_no_recap_table_leaves_every_switch_on() {
        // ABSENT IS ALL ON, which is what makes the table optional: a machine
        // that never writes one behaves exactly as it did before the table
        // existed. The direction is STATED rather than derived, because a
        // derived default is all-off, and that would silently take the
        // catch-up card away from every machine whose config predates this.
        let config = parse_config("[plugins.hue]\nenabled = true\n").unwrap();
        assert!(config.recap.replay_card, "the catch-up card");
        assert!(config.recap.digest, "the recap");
        assert!(config.recap.digest_as_thread, "the recap's own thread");
    }

    #[test]
    fn a_misspelled_recap_key_is_refused_by_name_rather_than_left_at_its_default() {
        // UNKNOWN KEYS REFUSE HERE, unlike a plugin's free-form settings, and
        // the difference is who reads them: a plugin table is handed to a
        // plugin this layer cannot judge, while this table is read here and
        // nowhere else. An unjudged key is a typo that leaves the switch ON
        // while the operator believes they turned it off.
        let err = parse_config("[recap]\nreplaycard = false\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("replaycard"),
                "the offender is named: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_non_boolean_recap_switch_is_refused_naming_the_key() {
        // `digest = "yes"` read as a switch is the same defect one level down
        // from a non-boolean `enabled`: the operator asked for something, did
        // not get it, and was told nothing.
        let err = parse_config("[recap]\ndigest = \"yes\"\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("digest"),
                "the offender is named: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_top_level_key_that_merely_looks_like_recap_is_still_refused_by_name() {
        // GUARD. Admitting `[recap]` admits ONE more key and nothing else:
        // the plural typo, newly plausible now that the singular parses, has
        // to name itself rather than sit there as a table nothing reads. The
        // retired `[home]` table's test guards the same arm from the other
        // side, and both must stay green as the arm grows.
        let err = parse_config("[recaps]\ndigest = false\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("recaps"),
                "the offender is named: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_non_table_recap_value_is_refused_naming_the_key() {
        // `recap = 5` is `plugins = 5` one table over: a value at the key the
        // switches hang off must refuse, never fall through to the all-on
        // default and leave the operator believing their file was read.
        //
        // THE ARM IS NAMED, not just the key. "unknown top-level key `recap`"
        // is what comes back when the admitting arm is gone entirely, and it
        // carries the word `recap` too: an assertion that asked only for the
        // name would pass for the refusal that says the table is not a
        // setting at all, which is a different fault with a different fix.
        let err = parse_config("recap = 5\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("recap"),
                    "the offender is named: {message}"
                );
                assert!(
                    message.contains("is not a table"),
                    "and it is the non-table arm rather than the unknown-key one: {message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn the_recaps_volume_threshold_is_a_count_the_operator_can_state() {
        // THE CALIBRATION KNOB. The locked threshold carries a tilde and has no
        // live measurement behind it, so it ships as a key defaulted to the
        // operator's own guess and the recap's header prints the real count
        // every time. One week of real recaps settles it without a rebuild.
        let config = parse_config("[recap]\nmin_events = 3\n").unwrap();
        assert_eq!(config.recap.min_events, 3, "the stated count was read");
        assert!(config.recap.digest, "and the switches kept their defaults");
        assert_eq!(
            parse_config("[plugins.hue]\nenabled = true\n")
                .unwrap()
                .recap
                .min_events,
            8,
            "an absent key is the operator's stated eight"
        );
    }

    #[test]
    fn a_volume_threshold_of_zero_is_refused_by_name_rather_than_read_as_every_event() {
        // ZERO IS NOT A THRESHOLD. `counted.len() >= 0` is always true, so it
        // recaps every single event, including one over an EMPTY window, which
        // is the one state the recap body says the event path never posts. An
        // operator calibrating the knob downward would get a card and a Discord
        // recap on every event, each saying nothing was recorded.
        let err = parse_config("[recap]\nmin_events = 0\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(message.contains("min_events"), "{message}");
                assert!(
                    message.contains('1'),
                    "the refusal names the floor rather than only the offence: {message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        assert_eq!(
            parse_config("[recap]\nmin_events = 1\n")
                .unwrap()
                .recap
                .min_events,
            1,
            "and one is accepted: it means any activity at all"
        );
    }

    #[test]
    fn a_volume_threshold_that_is_not_a_count_is_refused_naming_the_key() {
        // A STRING, A FRACTION AND A NEGATIVE are each a threshold the operator
        // asked for and would not get, and each has to say so rather than leave
        // the count silently at its default.
        for stated in ["\"eight\"", "8.5", "-1", "true"] {
            let err = parse_config(&format!("[recap]\nmin_events = {stated}\n")).unwrap_err();
            match err {
                ConfigError::Invalid(message) => assert!(
                    message.contains("min_events"),
                    "the offender is named for {stated}: {message}"
                ),
                other => panic!("expected Invalid for {stated}, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_summarizer_is_an_argument_list_the_operator_states_word_by_word() {
        // ARGV, NEVER A SHELL STRING. Nothing here is interpreted, so there is
        // no quoting rule to get wrong and no injection surface at all; a
        // different backend is a different array and a Linux machine writes its
        // own, which is the whole of the configurable-backend mandate.
        let config = parse_config(
            "[recap]\nsummarizer = [\"ollama\", \"run\", \"qwen3.5:4b\", \"--think=false\"]\n",
        )
        .unwrap();
        assert_eq!(
            config.recap.summarizer.as_deref(),
            Some(
                ["ollama", "run", "qwen3.5:4b", "--think=false"]
                    .map(String::from)
                    .as_slice()
            ),
            "the words the operator wrote, in order"
        );
        assert_eq!(
            parse_config("[recap]\ndigest = true\n")
                .unwrap()
                .recap
                .summarizer,
            None,
            "UNSET IS THE WORKING SETTING: no summarizer is the plain lists"
        );
    }

    #[test]
    fn a_summarizer_that_is_not_a_list_of_words_is_refused_naming_the_key() {
        // THE FOUR SHAPES A HAND WRITES BY MISTAKE: the shell string this key
        // deliberately is not, an array with something that is not a word in
        // it, an empty array that names no command at all, and an array whose
        // FIRST WORD is empty, which names no command either and used to parse.
        // `Command::new("")` fails to spawn, and the operator then reads a
        // summarizer that is not answering rather than the table they have to
        // fix, which is the exact outcome the empty-array refusal exists to
        // prevent. Each is refused by name rather than leaving the summarizer
        // silently unset, which is the difference between a recap that says it
        // fell back and one the operator believes is summarized.
        for (stated, expected) in [
            ("\"ollama run qwen3.5:4b\"", "not a list"),
            ("[\"ollama\", 3]", "not a list"),
            ("[]", "names no command"),
            ("[\"\"]", "names no command"),
            ("[\"\", \"run\"]", "names no command"),
        ] {
            let err = parse_config(&format!("[recap]\nsummarizer = {stated}\n")).unwrap_err();
            match err {
                ConfigError::Invalid(message) => {
                    assert!(
                        message.contains("summarizer"),
                        "the offender is named for {stated}: {message}"
                    );
                    assert!(
                        message.contains(expected),
                        "the refusal says what is wrong for {stated}: {message}"
                    );
                }
                other => panic!("expected Invalid for {stated}, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_summarizers_deadline_is_a_count_of_seconds_with_a_generous_default() {
        // FOUR MINUTES, and what it covers is GENERATION. MEASURED on one
        // machine: a whole three-call episode took about 114.6 seconds, nearly
        // all of it tokens arriving at about eleven a second, while the cold
        // model load cost about 5.5 seconds and was paid once. Nobody is
        // waiting: the caller is a process the event path never joined.
        assert_eq!(
            parse_config("[recap]\ndigest = true\n")
                .unwrap()
                .recap
                .summarizer_deadline_secs,
            240,
            "the default is generous against a measured episode"
        );
        assert_eq!(
            parse_config("[recap]\nsummarizer_deadline_secs = 5\n")
                .unwrap()
                .recap
                .summarizer_deadline_secs,
            5
        );
        // AND IT HAS A TOP END, refused by name for `min_events`'s own reason.
        // An hour is already far past the default, and past it the two failures
        // are real: nothing supervises the detached recap child, so a wedged
        // backend holds one child and one backend process for as long as the
        // number says, and `9223372036854775807` is a plain TOML integer that
        // PANICS the child at `Instant::now() + deadline` (MEASURED: "overflow
        // when adding duration to instant"). That panic lands in a process
        // whose stderr is /dev/null and whose exit code nobody reads, so the
        // recap vanishes with no rung of the ladder taken, after the card has
        // already said it is coming.
        assert_eq!(
            parse_config("[recap]\nsummarizer_deadline_secs = 3600\n")
                .unwrap()
                .recap
                .summarizer_deadline_secs,
            3600,
            "an hour is inside the ceiling"
        );
        for stated in ["\"soon\"", "9.5", "-1", "3601", "9223372036854775807"] {
            let err = parse_config(&format!("[recap]\nsummarizer_deadline_secs = {stated}\n"))
                .unwrap_err();
            match err {
                ConfigError::Invalid(message) => assert!(
                    message.contains("summarizer_deadline_secs"),
                    "the offender is named for {stated}: {message}"
                ),
                other => panic!("expected Invalid for {stated}, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_two_external_sources_are_named_by_the_operator_or_not_read_at_all() {
        // NEITHER SECTION HAS A SOURCE pns can find on its own: merged pull
        // requests live in a repository nothing here knows the name of, and the
        // review notes live wherever this operator's own pipeline puts them.
        // Both are therefore keys, and an absent key is the working setting.
        let config = parse_config(
            "[recap]\nrepos = [\"webdavis/dotfiles\"]\n\
             review_notes = \"~/.claude/pipeline/slices/checklist-*.md\"\n",
        )
        .unwrap();
        assert_eq!(config.recap.repos, ["webdavis/dotfiles".to_string()]);
        assert_eq!(
            config.recap.review_notes.as_deref(),
            Some("~/.claude/pipeline/slices/checklist-*.md")
        );
        let unconfigured = parse_config("[recap]\ndigest = true\n").unwrap().recap;
        assert!(
            unconfigured.repos.is_empty(),
            "UNSET IS THE WORKING SETTING: no repo is no `gh` at all"
        );
        assert_eq!(
            unconfigured.review_notes, None,
            "and no glob is no directory read at all"
        );
    }

    #[test]
    fn a_repos_value_that_is_not_repository_names_is_refused_naming_the_key() {
        // THE SAME FOUR SHAPES `summarizer` REFUSES, for the same reason: a
        // list this layer reads itself is a list it can judge, and a repo name
        // it silently dropped would read to the operator as a night with no
        // merges in it rather than as a table they have to fix.
        for (stated, expected) in [
            ("\"webdavis/dotfiles\"", "not a list"),
            ("[\"webdavis/dotfiles\", 3]", "not a list"),
            ("[]", "names no repository"),
            ("[\"\"]", "names no repository"),
        ] {
            let err = parse_config(&format!("[recap]\nrepos = {stated}\n")).unwrap_err();
            match err {
                ConfigError::Invalid(message) => {
                    assert!(
                        message.contains("repos"),
                        "the offender is named for {stated}: {message}"
                    );
                    assert!(
                        message.contains(expected),
                        "the refusal says what is wrong for {stated}: {message}"
                    );
                }
                other => panic!("expected Invalid for {stated}, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_review_notes_glob_that_names_no_readable_file_is_refused_naming_the_key() {
        // THE GLOB IS THE WHOLE PERMISSION. It is the only thing that decides
        // which files pns opens, so a shape it cannot resolve exactly is
        // refused rather than resolved generously: a RELATIVE path would
        // resolve against whatever directory the return event happened to be
        // in, and a `*` in a DIRECTORY would make the set of directories pns
        // reads a search rather than a statement.
        for (stated, expected) in [
            ("3", "not a path"),
            ("\"\"", "names no file"),
            ("\"slices/checklist-*.md\"", "absolute"),
            ("\"~/.claude/*/checklist-*.md\"", "file name may hold a"),
            ("\"~/.claude/checklist-*-*.md\"", "only one"),
        ] {
            let err = parse_config(&format!("[recap]\nreview_notes = {stated}\n")).unwrap_err();
            match err {
                ConfigError::Invalid(message) => {
                    assert!(
                        message.contains("review_notes"),
                        "the offender is named for {stated}: {message}"
                    );
                    assert!(
                        message.contains(expected),
                        "the refusal says what is wrong for {stated}: {message}"
                    );
                }
                other => panic!("expected Invalid for {stated}, got {other:?}"),
            }
        }
    }

    // --- the Focus modes that mean it ---------------------------------------

    #[test]
    fn a_focus_table_names_the_modes_that_silence_pns() {
        let config = parse_config("[focus]\nsilence = [\"Sleep\", \"Coding\"]\n").unwrap();
        assert_eq!(config.focus_silence, ["Sleep", "Coding"]);
    }

    #[test]
    fn a_config_with_no_focus_table_names_no_mode_at_all() {
        // OFF IS THE DEFAULT, and it is the whole reason there is no `enabled`
        // key: a machine that never wrote the table behaves exactly as it did
        // before the table existed. MEASURED on this operator's own machine, a
        // Focus was asserted for 95% of one day, so a feature that shipped on
        // would have silenced almost everything pns raised that day.
        let config = parse_config("[plugins.hue]\nenabled = true\n").unwrap();
        assert!(config.focus_silence.is_empty());
    }

    #[test]
    fn a_silence_list_that_is_not_a_list_is_refused_naming_the_key() {
        // `silence = "Sleep"` is what a hand writes first. Read as one name it
        // would work by accident; read as anything else it silences nothing
        // and says nothing, which is the state the operator cannot discover.
        let err = parse_config("[focus]\nsilence = \"Sleep\"\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("silence"),
                    "the offender is named: {message}"
                );
                assert!(
                    message.contains("focus"),
                    "and so is the table it is in: {message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_mode_name_that_is_not_a_string_is_refused_naming_the_key() {
        let err = parse_config("[focus]\nsilence = [\"Sleep\", 5]\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("silence"),
                "the offender is named: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_mode_name_that_is_the_empty_string_is_refused_by_name() {
        // AN ENTRY THAT NAMES NO MODE is a policy the operator believes they
        // wrote and pns can never act on, which is the misspelled key's own
        // failure one level down. `[recap] repos` refuses its empty entry for
        // this reason and this refusal is that rule, not a new one.
        let err = parse_config("[focus]\nsilence = [\"Sleep\", \"\"]\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("silence"),
                    "the offender is named: {message}"
                );
                assert!(
                    message.contains("focus"),
                    "and so is the table it is in: {message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_silence_list_is_admitted_because_it_is_the_feature_switched_off() {
        // THE BOUNDARY OF THE REFUSAL ABOVE. An empty LIST is a working,
        // readable setting that says exactly what it does, so a refusal that
        // reached it would refuse the one config the template's own commented
        // block turns into when a mode is deleted from it.
        let config = parse_config("[focus]\nsilence = []\n").unwrap();
        assert!(config.focus_silence.is_empty());
    }

    #[test]
    fn a_misspelled_focus_key_is_refused_by_name_rather_than_ignored() {
        // An unjudged key here is a Focus policy the operator believes they
        // wrote and pns never reads.
        let err = parse_config("[focus]\nsilenced = [\"Sleep\"]\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("silenced"),
                "the offender is named: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn a_non_table_focus_value_is_refused_naming_the_arm_rather_than_the_key() {
        // `recap = 5`'s sibling one table over, and asserted the same way: the
        // unknown-top-level-key refusal carries the word `focus` too, so an
        // assertion that asked only for the name would pass for the day the
        // admitting arm went missing entirely.
        let err = parse_config("focus = 5\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => {
                assert!(
                    message.contains("focus"),
                    "the offender is named: {message}"
                );
                assert!(
                    message.contains("is not a table"),
                    "and it is the non-table arm rather than the unknown-key one: {message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    // --- [daemon] ------------------------------------------------------------

    /// The clock's one switch, in the four states the table has.
    ///
    /// DEFAULT ON, unlike `[focus]` and unlike a plugin, and the reason is that
    /// this table gates nothing an operator can see. An idle daemon reads one
    /// empty directory a second; default OFF would put both rider features
    /// behind two switches, so enabling a light and seeing nothing would send
    /// the operator hunting for a second, invisible one.
    #[test]
    fn the_daemon_table_reads_one_switch_defaults_on_and_refuses_the_rest_by_name() {
        assert!(
            parse_config("").unwrap().daemon_enabled,
            "no table at all is the default, which is on"
        );
        assert!(
            parse_config("[daemon]\nenabled = true\n")
                .unwrap()
                .daemon_enabled
        );
        assert!(
            !parse_config("[daemon]\nenabled = false\n")
                .unwrap()
                .daemon_enabled
        );

        let err = parse_config("[daemon]\nenabled = \"yes\"\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("daemon") && message.contains("enabled"),
                "the offender is named: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }

        let err = parse_config("[daemon]\nenable = true\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("daemon") && message.contains("enable"),
                "the offender is named: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }

        let err = parse_config("daemon = 5\n").unwrap_err();
        match err {
            ConfigError::Invalid(message) => assert!(
                message.contains("daemon") && message.contains("is not a table"),
                "and it is the non-table arm rather than the unknown-key one: {message}"
            ),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    // --- [nag] ---------------------------------------------------------------

    /// The nag's one key, which is the switch AND the schedule.
    ///
    /// `[focus] silence`'S OWN PRECEDENT: naming nothing and switching off are
    /// one statement, so there is no second `enabled` key that can disagree
    /// with the first. DEFAULT OFF, unlike `[daemon]` beside it, because this
    /// table gates something that INTERRUPTS and because the feature needs a
    /// `chezmoi apply` and a running daemon before it works at all.
    #[test]
    fn the_nag_table_reads_one_schedule_defaults_off_and_zero_is_off_rather_than_an_error() {
        assert_eq!(
            parse_config("").unwrap().nag_after_secs,
            0,
            "no table at all is the feature off"
        );
        assert_eq!(
            parse_config("[nag]\nafter_secs = 300\n")
                .unwrap()
                .nag_after_secs,
            300
        );
        assert_eq!(
            parse_config("[nag]\nafter_secs = 0\n")
                .unwrap()
                .nag_after_secs,
            0,
            "zero is the same statement as no table, and it is not an error"
        );
        // The floor and the ceiling are admitted at their own edges.
        assert_eq!(
            parse_config("[nag]\nafter_secs = 30\n")
                .unwrap()
                .nag_after_secs,
            30
        );
        assert_eq!(
            parse_config("[nag]\nafter_secs = 3600\n")
                .unwrap()
                .nag_after_secs,
            3600
        );
    }

    /// Every way a schedule can fail to be one, each naming the offender.
    ///
    /// THE FLOOR EXISTS because a nudge arriving before the operator could
    /// plausibly have reached their phone is exactly the stacking the design
    /// forbids; THE CEILING mirrors `summarizer_deadline_secs` and must sit
    /// inside the daemon's own registration window, which it does by three
    /// orders of magnitude.
    #[test]
    fn a_schedule_that_is_not_a_count_of_seconds_is_refused_by_name() {
        for (case, text, named) in [
            ("negative", "[nag]\nafter_secs = -1\n", "after_secs"),
            (
                "a duration string",
                "[nag]\nafter_secs = \"5m\"\n",
                "after_secs",
            ),
            ("fractional", "[nag]\nafter_secs = 300.5\n", "after_secs"),
            ("a list", "[nag]\nafter_secs = [300]\n", "after_secs"),
            ("under the floor", "[nag]\nafter_secs = 29\n", "after_secs"),
            (
                "over the ceiling",
                "[nag]\nafter_secs = 3601\n",
                "after_secs",
            ),
            (
                "a misspelled key",
                "[nag]\nafter_seconds = 300\n",
                "after_seconds",
            ),
            ("a non-table nag", "nag = 300\n", "is not a table"),
        ] {
            match parse_config(text).unwrap_err() {
                ConfigError::Invalid(message) => assert!(
                    message.contains("nag") && message.contains(named),
                    "{case}: the offender is named: {message}"
                ),
                other => panic!("{case}: expected Invalid, got {other:?}"),
            }
        }
    }

    // --- the mobile submission deadline --------------------------------------

    #[test]
    fn the_mobile_submission_deadline_is_a_count_of_seconds_defaulted_to_five() {
        // FIVE SECONDS, the crate's own house number for a local pipe that
        // should have been instant: it is `PNS_PAYLOAD_DEADLINE_MS`'s default,
        // bounding the same kind of thing on the same hook. The submission is
        // a registration with a daemon, measured at roughly a tenth of a
        // second, so five is about thirty times the observed round trip.
        assert_eq!(
            submit_deadline(&parse_config("").unwrap()).unwrap(),
            Duration::from_secs(5),
            "no config at all is still bounded"
        );
        assert_eq!(
            submit_deadline(
                &parse_config("[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n").unwrap()
            )
            .unwrap(),
            Duration::from_secs(5),
            "a mobile table that does not state one is the default"
        );
        // THE TABLE IS ARMED IN EVERY CASE BELOW, switch and backend both,
        // because that is what `armed_mobile` reads and this key is read
        // through it: a number under a table nobody switched on, or under one
        // naming a backend nothing implements, is not a bound this binary owns.
        assert_eq!(
            submit_deadline(
                &parse_config(
                    "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
                     submit_deadline_secs = 30\n"
                )
                .unwrap()
            )
            .unwrap(),
            Duration::from_secs(30),
            "the operator's own number is the bound"
        );
        // OFF THE MOBILE TABLE. Every plugin's settings reach this layer in
        // the same shape, so a reader spelled against the wrong table would
        // take a number the operator wrote for something else, or miss the one
        // they wrote for this. The misplacement is now refused a whole layer
        // earlier, at the load that judges each table's own vocabulary, so
        // this states both halves: the key on another table never parses, and
        // a config carrying no mobile table at all is still the default.
        assert!(
            parse_config("[plugins.hue]\nsubmit_deadline_secs = 30\n").is_err(),
            "the mobile table's key is not part of hue's vocabulary"
        );
        assert_eq!(
            submit_deadline(&parse_config("[plugins.hue]\nenabled = true\n").unwrap()).unwrap(),
            Duration::from_secs(5),
            "another plugin's table is not where the mobile bound is read"
        );
    }

    #[test]
    fn a_mobile_table_naming_no_backend_contributes_no_settings_at_all() {
        // THE DEADLINE INCLUDED, which is the half a reader spelled against
        // the table directly used to miss. `type` says which backend every
        // setting under the table belongs to, so a table naming one nothing
        // implements has no settings this binary may read as moshi's: a
        // `submit_deadline_secs = 1` written for some other backend must not
        // shorten the window pns waits for a moshi submission in.
        let refused = parse_config(
            "[plugins.mobile]\nenabled = true\ntype = \"pushover\"\nsubmit_deadline_secs = 1\n",
        )
        .unwrap();
        match submit_deadline(&refused) {
            Err(ConfigError::Invalid(message)) => {
                assert!(message.contains("\"pushover\""), "quoting it: {message}");
                assert!(message.contains("type"), "and naming the key: {message}");
            }
            other => panic!("expected the type refusal, got {other:?}"),
        }
        // THE POSITIVE CONTROL: a refusal that fired on every table would pass
        // the assertion above and take every operator's deadline away.
        let armed = parse_config(
            "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\nsubmit_deadline_secs = 30\n",
        )
        .unwrap();
        assert_eq!(submit_deadline(&armed).unwrap(), Duration::from_secs(30));
        // AND A TABLE THE OPERATOR SWITCHED OFF IS INERT, its settings with
        // it: one switch, one answer, rather than a per-key exception nobody
        // could predict from the flag they set.
        let off = parse_config(
            "[plugins.mobile]\nenabled = false\ntype = \"moshi\"\nsubmit_deadline_secs = 30\n",
        )
        .unwrap();
        assert_eq!(submit_deadline(&off).unwrap(), Duration::from_secs(5));
    }

    #[test]
    fn a_submission_deadline_that_is_not_a_count_of_seconds_is_refused_by_name() {
        // REFUSED IN BOTH DIRECTIONS, and each refusal names the key, because
        // "config invalid" without a noun is a hunt.
        //
        // ZERO IS A TRAP HERE, unlike `summarizer_deadline_secs`'s zero. A
        // deadline that fires before the daemon can possibly answer is this
        // feature switched off by accident: every approval would lose its
        // phone card while the operator believed they had merely tightened a
        // bound. The ceiling mirrors `MAX_SUMMARIZER_DEADLINE_SECS` rather
        // than the harness's own ten minutes, because another tool's number is
        // not ours to hard-code and Codex's differs.
        for stated in [
            "0",
            "-1",
            "\"5s\"",
            "9.5",
            "[5]",
            "3601",
            "9223372036854775807",
        ] {
            let config = parse_config(&format!(
                "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
                 submit_deadline_secs = {stated}\n"
            ))
            .unwrap();
            match submit_deadline(&config) {
                Err(ConfigError::Invalid(message)) => assert!(
                    message.contains("submit_deadline_secs"),
                    "the offender is named for {stated}: {message}"
                ),
                other => panic!("expected a named refusal for {stated}, got {other:?}"),
            }
        }
    }

    // --- the IO edge --------------------------------------------------------

    #[test]
    fn a_missing_file_is_its_own_outcome_not_an_error_and_not_empty() {
        let outcome = load_config(std::path::Path::new("/nonexistent/pns-config-test.toml"));
        assert_eq!(outcome, Ok(LoadOutcome::Missing));
    }

    #[test]
    fn a_present_file_loads_through_the_parser() {
        let path = std::env::temp_dir().join(format!("pns-config-test-{}", std::process::id()));
        std::fs::write(&path, "[plugins.hue]\nenabled = true\n").unwrap();
        let outcome = load_config(&path);
        std::fs::remove_file(&path).ok();
        match outcome {
            Ok(LoadOutcome::Loaded(config)) => assert!(config.plugins["hue"].enabled),
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    #[test]
    fn a_dangling_config_symlink_is_an_error_never_missing() {
        // chezmoi deploys configs as symlinks: a broken link is a PRESENT
        // entry whose target is wrong, and reading it as "unconfigured"
        // would silently disable everything. Only a truly absent entry is
        // Missing.
        let link = std::env::temp_dir().join(format!("pns-config-dangling-{}", std::process::id()));
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink("pns-absent-target", &link).unwrap();
        let outcome = load_config(&link);
        std::fs::remove_file(&link).ok();
        match outcome {
            Err(ConfigError::Unreadable(message)) => {
                assert!(!message.is_empty(), "the path and cause are named")
            }
            other => panic!("expected Unreadable, got {other:?}"),
        }
    }

    #[test]
    fn an_unreadable_path_is_an_error_never_a_silent_unconfigured() {
        // A directory at the config path is the deterministic unreadable
        // case: it exists, so reporting Missing here would make a broken
        // path read as "unconfigured" and silently disable everything.
        let outcome = load_config(std::env::temp_dir().as_path());
        match outcome {
            Err(ConfigError::Unreadable(message)) => {
                assert!(!message.is_empty(), "the path and cause are named")
            }
            other => panic!("expected Unreadable, got {other:?}"),
        }
    }

    // --- [lights] -----------------------------------------------------------

    /// The Invalid refusal's own sentence, or a panic naming what came instead.
    fn refusal(text: &str) -> String {
        match parse_config(text) {
            Err(ConfigError::Invalid(message)) => message,
            other => panic!("expected a named refusal, got {other:?}"),
        }
    }

    /// The `[lights]` table one config parses to.
    fn lights(text: &str) -> Lights {
        *parse_config(text)
            .expect("this config parses")
            .lights
            .expect("and carries a lights table")
    }

    #[test]
    fn no_lights_table_is_none_and_an_empty_one_is_every_locked_default() {
        // ABSENT AND EMPTY ARE DIFFERENT CONFIGS, which is what the Option
        // spells: a machine with no table keeps the room-based pulse it has
        // always had, and a machine with an empty one has asked for the lamps
        // and routed nothing yet.
        assert_eq!(parse_config("").expect("empty parses").lights, None);
        let shipped = lights("[lights]\n");
        assert_eq!(shipped, Lights::default());
        // THE LOCKED FIGURES, each one set on a real lamp. A change to any of
        // them is a change to something that was looked at.
        assert_eq!(shipped.refresh_secs, 12);
        assert_eq!(
            shipped.done,
            Pulse {
                duration_ms: 4000,
                brightness: 100
            }
        );
        assert_eq!(shipped.failed, shipped.done);
        assert_eq!(
            shipped.blocked,
            Blocked {
                breath: Breath {
                    duration_ms: 2000,
                    high: 100,
                    low: 30
                },
                give_up_after_secs: 57_600,
            }
        );
        assert_eq!(
            shipped.unread,
            Unread {
                breath: Breath {
                    duration_ms: 4000,
                    high: 60,
                    low: 10
                },
                after_secs: 300,
            }
        );
        assert_eq!(
            shipped.looping,
            Looping {
                breath: Breath {
                    duration_ms: 4000,
                    high: 60,
                    low: 10
                },
                threshold_secs: 300,
                lease_timeout_secs: 3900,
            }
        );
        assert_eq!(
            shipped.dim,
            Breath {
                duration_ms: 3000,
                high: 7,
                low: 1
            }
        );
        assert!(shipped.lamps.is_empty() && shipped.rooms.is_empty() && shipped.zones.is_empty());
    }

    #[test]
    fn a_behaviour_table_moves_the_keys_it_states_and_leaves_the_rest_at_their_locked_values() {
        let stated = lights(
            "[lights]\nrefresh_secs = 25\n\
             [lights.done]\nduration_ms = 1500\n\
             [lights.blocked]\nlow = 45\n\
             [lights.unread]\nafter_secs = 60\n\
             [lights.loop]\nthreshold_secs = 360\nlease_timeout_secs = 600\n\
             [lights.dim]\nhigh = 9\n",
        );
        assert_eq!(stated.refresh_secs, 25);
        assert_eq!(
            stated.done,
            Pulse {
                duration_ms: 1500,
                brightness: 100
            },
            "the duration moved and the brightness stayed at its locked value"
        );
        assert_eq!(
            stated.failed,
            Lights::default().failed,
            "and its sibling is untouched"
        );
        assert_eq!(stated.blocked.breath.low, 45);
        assert_eq!(stated.blocked.breath.duration_ms, 2000);
        assert_eq!(
            stated.blocked.give_up_after_secs,
            Lights::default().blocked.give_up_after_secs,
            "the breath moved and the backstop stayed at its locked default"
        );
        assert_eq!(stated.unread.after_secs, 60);
        assert_eq!(stated.unread.breath, Lights::default().unread.breath);
        assert_eq!(stated.looping.threshold_secs, 360);
        assert_eq!(stated.looping.lease_timeout_secs, 600);
        assert_eq!(stated.dim.high, 9);
        assert_eq!(stated.dim.low, 1);
    }

    #[test]
    fn a_knob_that_does_not_apply_to_a_behaviour_does_not_exist_on_it() {
        // NO DEAD KNOBS (operator ruling), enforced by the roster rather than by
        // a comment: a blink has no low end to fade to, and a breath has no
        // single brightness. A reader who sets one and watches nothing happen is
        // exactly what this refuses.
        for (written, key) in [
            ("[lights.done]\nlow = 10\n", "low"),
            ("[lights.done]\nhigh = 90\n", "high"),
            ("[lights.failed]\nlow = 10\n", "low"),
            ("[lights.blocked]\nbrightness = 90\n", "brightness"),
            ("[lights.unread]\nbrightness = 90\n", "brightness"),
            ("[lights.loop]\nbrightness = 90\n", "brightness"),
            ("[lights.dim]\nbrightness = 90\n", "brightness"),
            ("[lights.dim]\nthreshold_secs = 90\n", "threshold_secs"),
            ("[lights.done]\nthreshold_secs = 90\n", "threshold_secs"),
            ("[lights.blocked]\nafter_secs = 90\n", "after_secs"),
            (
                "[lights.unread]\nlease_timeout_secs = 90\n",
                "lease_timeout_secs",
            ),
        ] {
            let said = refusal(written);
            assert!(
                said.contains(key) && said.contains("the table serves"),
                "{written:?} must refuse `{key}` by name and list what the table does \
                 serve: {said}"
            );
        }
    }

    #[test]
    fn every_lights_number_is_bounded_on_both_sides_and_refused_by_name_outside_them() {
        // BOTH ENDS, ALWAYS. A floor alone leaves a value that parses and cannot
        // work; a ceiling alone leaves the same at the other end.
        for (written, key) in [
            ("[lights]\nrefresh_secs = 9\n", "refresh_secs"),
            ("[lights]\nrefresh_secs = 31\n", "refresh_secs"),
            ("[lights.done]\nduration_ms = 199\n", "duration_ms"),
            ("[lights.done]\nduration_ms = 5001\n", "duration_ms"),
            ("[lights.done]\nbrightness = 0\n", "brightness"),
            ("[lights.done]\nbrightness = 101\n", "brightness"),
            ("[lights.blocked]\nlow = 0\n", "low"),
            ("[lights.blocked]\nhigh = 101\n", "high"),
            (
                "[lights.blocked]\ngive_up_after_secs = 59\n",
                "give_up_after_secs",
            ),
            (
                "[lights.blocked]\ngive_up_after_secs = 604801\n",
                "give_up_after_secs",
            ),
            ("[lights.loop]\nthreshold_secs = 0\n", "threshold_secs"),
            ("[lights.loop]\nthreshold_secs = 86401\n", "threshold_secs"),
            (
                "[lights.loop]\nlease_timeout_secs = 59\n",
                "lease_timeout_secs",
            ),
            (
                "[lights.loop]\nlease_timeout_secs = 86401\n",
                "lease_timeout_secs",
            ),
            ("[lights.unread]\nafter_secs = 86401\n", "after_secs"),
        ] {
            let said = refusal(written);
            assert!(
                said.contains(key) && said.contains("range"),
                "{written:?} must be refused by name with the range echoed: {said}"
            );
        }
        // THE ENDS THEMSELVES ARE ACCEPTED, which is what makes the bound a
        // bound rather than an off-by-one.
        for written in [
            "[lights]\nrefresh_secs = 10\n",
            "[lights]\nrefresh_secs = 30\n",
            "[lights.done]\nduration_ms = 200\nbrightness = 1\n",
            "[lights.done]\nduration_ms = 5000\nbrightness = 100\n",
            "[lights.loop]\nthreshold_secs = 1\nlease_timeout_secs = 60\n",
            "[lights.unread]\nafter_secs = 0\n",
            "[lights.blocked]\ngive_up_after_secs = 60\n",
            "[lights.blocked]\ngive_up_after_secs = 604800\n",
        ] {
            assert!(
                parse_config(written).is_ok(),
                "{written:?} sits on a bound and must be accepted"
            );
        }
    }

    #[test]
    fn the_blocked_backstop_reads_the_configured_number_rather_than_a_hardcoded_default() {
        // A KNOB WORTH NOTHING IF THE PARSER READS THE TABLE AND KEEPS THE
        // DEFAULT ANYWAY, so this proves the stated value lands rather than
        // merely that a valid table parses.
        assert_eq!(
            lights("[lights.blocked]\ngive_up_after_secs = 57600\n")
                .blocked
                .give_up_after_secs,
            57_600,
            "the shipped default, stated explicitly"
        );
        assert_eq!(
            lights("[lights.blocked]\ngive_up_after_secs = 3600\n")
                .blocked
                .give_up_after_secs,
            3_600,
            "a number that is NOT the default, so a parser that silently kept the \
             default instead of reading the table would still be caught"
        );
    }

    #[test]
    fn a_breath_whose_low_is_above_its_high_is_refused_rather_than_rendered_upside_down() {
        // THE DRIVER'S STATED INVARIANT IS THAT IT STOPS AT THE PEAK, and with
        // the ends swapped it would stop at the fainter of the two and the next
        // tick would resume from there.
        for written in [
            "[lights.blocked]\nhigh = 20\nlow = 40\n",
            "[lights.unread]\nhigh = 20\nlow = 40\n",
            "[lights.loop]\nhigh = 20\nlow = 40\n",
            "[lights.dim]\nhigh = 2\nlow = 4\n",
        ] {
            let said = refusal(written);
            assert!(
                said.contains("low 40") || said.contains("low 4"),
                "{written:?} must name both ends: {said}"
            );
            assert!(said.contains("peak"), "and say what it costs: {said}");
        }
        assert!(
            parse_config("[lights.blocked]\nhigh = 40\nlow = 40\n").is_ok(),
            "equal ends are a lamp that holds steady, which is a shape rather than \
             a mistake"
        );
    }

    #[test]
    fn a_lights_value_of_the_wrong_type_is_refused_by_name_and_by_type() {
        for (written, key) in [
            ("[lights]\nrefresh_secs = \"20\"\n", "refresh_secs"),
            ("[lights.done]\nduration_ms = true\n", "duration_ms"),
            ("[lights.dim]\nlow = 10.5\n", "low"),
            ("[lights]\ndone = 3\n", "lights.done"),
            ("[lights]\nlamp = 3\n", "lamp"),
        ] {
            let said = refusal(written);
            assert!(said.contains(key), "{written:?} must name `{key}`: {said}");
        }
    }

    // --- the routing grammar -------------------------------------------------

    #[test]
    fn a_declaration_at_any_of_the_three_levels_reads_the_same_three_keys() {
        // ONE VOCABULARY FOR THREE LEVELS, because a lamp, a room and a zone
        // answer the same questions and differ only in how specific they are.
        for level in ["lamp", "room", "zone"] {
            let held = lights(&format!(
                "[lights.{level}.\"3F - Studio\"]\n\
                 shows = [\"done\", \"failed\"]\n\
                 dim_window = \"22:00-07:00\"\n\
                 dim_behaviours = [\"blocked\", \"unread\", \"loop\"]\n"
            ));
            let table = match level {
                "lamp" => &held.lamps,
                "room" => &held.rooms,
                _ => &held.zones,
            };
            assert_eq!(
                table.get("3F - Studio"),
                Some(&Target {
                    shows: Some(vec![Behaviour::Done, Behaviour::Failed]),
                    dim_window: Some("22:00-07:00".to_string()),
                    dim_behaviours: vec![Behaviour::Blocked, Behaviour::Unread, Behaviour::Looping],
                }),
                "at the {level} level"
            );
        }
    }

    #[test]
    fn a_declaration_that_states_nothing_states_nothing_rather_than_defaulting() {
        // `None` IS "SAID NOTHING", which a plain `Vec` could not spell: it is
        // what lets a lamp state which behaviours it carries and inherit its
        // room's window, and what tells a deliberate empty list from silence.
        let silent = lights("[lights.lamp.\"HCL1\"]\n");
        assert_eq!(silent.lamps["HCL1"], Target::default());
        assert_eq!(silent.lamps["HCL1"].shows, None);
        let emptied = lights("[lights.lamp.\"HCL1\"]\nshows = []\n");
        assert_eq!(
            emptied.lamps["HCL1"].shows,
            Some(Vec::new()),
            "an empty list is an OVERRIDE, which is how one lamp is taken out of a \
             routed room"
        );
    }

    #[test]
    fn a_behaviour_word_the_lamps_do_not_speak_is_refused_with_the_closed_set_named() {
        // THE REFUSAL LISTS THE WHOLE SET, which is worth the extra words here:
        // the failure it prevents is a lamp that stays dark while the operator
        // is sure they routed it, and their only evidence is a lamp doing
        // nothing.
        for key in ["shows", "dim_behaviours"] {
            let said = refusal(&format!(
                "[lights.room.\"3F - Studio\"]\n{key} = [\"breathing\"]\n"
            ));
            assert_eq!(
                said,
                format!(
                    "`lights.room.3F - Studio` key `{key}` names `breathing`, which is \
                     no behaviour; the lamps say done, failed, blocked, unread, loop"
                ),
            );
        }
    }

    #[test]
    fn dim_behaviours_with_no_window_to_run_them_in_is_refused_rather_than_read_and_dropped() {
        // NO DEAD KNOBS, which is the config ruling applied to the one pair of
        // keys that can be half written. The enables RIDE the window, so a
        // declaration naming which behaviours run dimmed and never saying when
        // is a list nothing will ever read: the operator gets a lamp that
        // strobes all night and a file that says it should not.
        for stated in ["[\"blocked\"]", "[]"] {
            assert_eq!(
                refusal(&format!(
                    "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
                     dim_behaviours = {stated}\n"
                )),
                "`lights.room.3F - Studio` states `dim_behaviours` with no \
                 `dim_window` for them to run in, so nothing would ever read them",
                "dim_behaviours = {stated}"
            );
        }
        // AN EMPTY LIST BESIDE A WINDOW IS THE BEDROOM RULE and stays legal:
        // the refusal is about a missing window, never about an empty list.
        assert!(
            parse_config(
                "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
                 dim_window = \"22:00-07:00\"\ndim_behaviours = []\n"
            )
            .is_ok()
        );
    }

    #[test]
    fn an_unknown_declaration_key_is_refused_by_name_with_the_path_the_operator_wrote() {
        // THE PATH THEY WROTE, not the roster's own row: an operator told
        // `lights.<level>` has no `dim_hours` would go looking for a table they
        // never typed.
        let said = refusal("[lights.room.\"3F - Studio\"]\ndim_hours = \"22:00-07:00\"\n");
        assert!(
            said.contains("`lights.room.3F - Studio` key `dim_hours`"),
            "{said}"
        );
        assert!(
            said.contains("dim_behaviours, dim_window, shows"),
            "and it lists what the level does serve: {said}"
        );
    }

    #[test]
    fn a_declaration_that_is_not_a_table_of_settings_is_refused_by_name() {
        for written in [
            "[lights]\nlamp = { \"HCL1\" = 3 }\n",
            "[lights]\nroom = 3\n",
            "[lights]\nzone = \"Upstairs\"\n",
        ] {
            let said = refusal(written);
            assert!(!said.is_empty(), "{written:?} must be refused: {said}");
        }
    }

    // --- every table's own vocabulary ---------------------------------------

    /// The header a config writes to reach one roster table. Two of the rows
    /// name a NESTED table whose own name is the operator's, so the roster
    /// holds the prefix and this picks a name to write under it.
    fn header_for(table: &str) -> String {
        match table {
            // THE ONE ROSTER ROW WITH NO HEADING OF ITS OWN: three levels share
            // it, so a sample writes whichever of them, and the refusal names
            // the path the operator wrote rather than this row.
            super::TARGET_KEYS => "lights.room.\"3F - Studio\"".to_string(),
            other => other.to_string(),
        }
    }

    /// A config that writes one key under one roster row. THE TOP-LEVEL ROW HAS
    /// NO HEADING: its keys are the six table names, written bare at the start
    /// of the file, which is the shape of every misspelled or moved table.
    fn config_writing(table: &str, key: &str, value: &str) -> String {
        match table {
            super::TOP_LEVEL => format!("{key} = {value}\n"),
            other => format!(
                "[{}]\n{key} = {value}\n{}",
                header_for(other),
                companion(table, key)
            ),
        }
    }

    /// The one key that cannot stand alone, written beside its sample.
    ///
    /// `dim_behaviours` NAMES WHAT RUNS DIMMED INSIDE A WINDOW, and a
    /// declaration that states it without one is refused by name. This walk
    /// asks whether the arm READS the key, so its sample writes the window the
    /// key depends on rather than the walk reading a refusal as a key nothing
    /// serves.
    fn companion(table: &str, key: &str) -> &'static str {
        match (table, key) {
            (super::TARGET_KEYS, "dim_behaviours") => "dim_window = \"22:00-07:00\"\n",
            _ => "",
        }
    }

    /// How a refusal from one roster row names the level it refused: every
    /// bracketed table by its own header, and the top level as what it is.
    fn shown_as(table: &str) -> String {
        match table {
            super::TOP_LEVEL => "top-level".to_string(),
            other => format!("`{}`", header_for(other)),
        }
    }

    /// The header text a refusal from one roster row carries, which for the
    /// three levels sharing a row is the PATH THE OPERATOR WROTE rather than the
    /// row's own name: an operator told `lights.<level>` has no `dim_hours`
    /// would go looking for a table they never typed.
    fn refusal_names(table: &str) -> String {
        match table {
            super::TARGET_KEYS => "`lights.room.3F - Studio`".to_string(),
            other => shown_as(other),
        }
    }

    #[test]
    fn a_mistyped_key_inside_a_plugin_table_is_refused_naming_the_table_and_the_key() {
        // A plugin's settings used to reach the plugin free-form, so a near
        // miss was a destination that quietly never worked: `room` for `rooms`
        // is a pulse into a room the bridge does not have, and `tokens` for
        // `token` is a phone card that silently never leaves the machine.
        for (table, mistyped, near) in [
            ("plugins.hermes", "keys", "key"),
            ("plugins.hue", "room", "rooms"),
            ("plugins.macos-banner", "sound", "enabled"),
            ("plugins.mobile", "tokens", "token"),
            ("plugins.router", "phone", "device_hostname"),
        ] {
            let said = refusal(&format!("[{table}]\nenabled = true\n{mistyped} = \"x\"\n"));
            assert!(
                said.contains(&format!("`{table}`")),
                "the TABLE is named: {said}"
            );
            assert!(
                said.contains(&format!("`{mistyped}`")),
                "and so is the key: {said}"
            );
            assert!(
                said.contains(near),
                "and the keys it does serve are listed: {said}"
            );
        }
    }

    #[test]
    fn every_key_a_shipped_plugin_table_serves_is_still_admitted() {
        // The positive control under the refusal above: a sweep that refused
        // the whole vocabulary would pass every assertion up there.
        let shipped = "[plugins.hermes]\nenabled = true\nkey = \"k\"\n             [plugins.hue]\nenabled = true\nbridge = \"b\"\nkey = \"k\"\n             rooms = [\"3F - Studio\"]\nquiet_hours = \"22:00-07:00\"\n             [plugins.macos-banner]\nenabled = true\n             [plugins.mobile]\nenabled = true\ntype = \"moshi\"\ntoken = \"t\"\n             mobile_watch_card = false\nsubmit_deadline_secs = 5\n             [plugins.router]\nenabled = true\ntype = \"unifi\"\n             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\n             device_mac = \"2e:11:ab:6d:b0:4f\"\ndevice_ipv4 = \"192.168.1.9\"\n             api_key = \"k\"\nstale_alert_channel = \"priority\"\n";
        let config = parse_config(shipped).expect("every shipped key parses");
        assert_eq!(config.plugins.len(), 5);
    }

    #[test]
    fn an_unregistered_plugin_tables_settings_stay_free_form_because_selection_is_by_name() {
        // TODAY'S BEHAVIOUR, pinned rather than changed. This layer knows the
        // vocabulary of the plugins that ship and has none for a name nothing
        // registered, so judging its keys would mean inventing a schema for a
        // plugin that does not exist. The NAME is the defect and the registry
        // is where it is refused, which is one layer later and still loud.
        let config = parse_config("[plugins.nosuch]\nenabled = true\nwhatever = 1\n")
            .expect("the settings of an unknown plugin are not this layer's to judge");
        assert!(config.plugins["nosuch"].enabled);
        assert!(config.plugins["nosuch"].settings.contains_key("whatever"));
        assert!(
            crate::registry::roster().enabled(&config).is_err(),
            "and the name itself is still refused, one layer on"
        );
    }

    #[test]
    fn a_table_the_file_does_not_serve_is_refused_listing_the_tables_it_does() {
        // THE MOST OPERATOR-VISIBLE TYPO CLASS: a whole table misspelled, or a
        // table that moved. `[home]` is the real one; the router probe's
        // settings moved under `[plugins.router]`, and a config written before
        // that move is refused WHOLE, which takes every plugin's secret with
        // it. Told only that `home` is unknown, an operator has nowhere to go.
        let said = refusal("[home]\nrouter_url = \"https://192.168.1.1\"\n");
        assert!(said.contains("`home`"), "the table is named: {said}");
        for serves in ["daemon", "focus", "lights", "nag", "plugins", "recap"] {
            assert!(
                said.contains(serves),
                "and `{serves}` is among the tables it says the file serves: {said}"
            );
        }
    }

    #[test]
    fn type_is_the_word_that_selects_a_backend_and_the_old_brand_is_refused() {
        // ONE WORD FOR ONE QUESTION, under every table that has a backend to
        // pick. `brand` was the router's alone, so an operator who had learnt
        // it on one table had to learn a second word on the next; there is now
        // one, and the retired spelling is refused by name with the vocabulary
        // spelled out rather than reaching the probe as a setting it ignores.
        let said = refusal("[plugins.router]\nenabled = true\nbrand = \"unifi\"\n");
        assert!(said.contains("`brand`"), "the retired key is named: {said}");
        assert!(
            said.contains("type"),
            "and `type` is listed instead: {said}"
        );
        assert!(
            parse_config("[plugins.router]\nenabled = true\ntype = \"unifi\"\n").is_ok(),
            "the router table serves `type`"
        );
        assert!(
            parse_config("[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n").is_ok(),
            "and so does the mobile table"
        );
    }

    #[test]
    fn every_table_refuses_an_unknown_key_by_name_and_lists_what_it_serves() {
        // ONE TEST PER TABLE, driven by the roster rather than written out, so
        // a table added to the schema without this treatment is a red test.
        // THE TOP LEVEL IS ONE OF THE ROWS, so the outermost refusal is held to
        // the same standard as the innermost.
        for (table, serves) in super::TABLE_KEYS.iter().copied() {
            let said = refusal(&config_writing(table, "zzz_not_a_key", "\"x\""));
            assert!(
                said.contains(&refusal_names(table)) && said.contains("`zzz_not_a_key`"),
                "`{table}` names the table and the key: {said}"
            );
            for key in serves {
                assert!(
                    said.contains(key),
                    "`{table}` lists `{key}` among what it serves: {said}"
                );
            }
        }
    }

    // --- the roster, the template and the doctor's wording ------------------

    /// One valid value for every key the roster declares, which is what makes
    /// the walk below a real parse rather than a name check.
    ///
    /// ITS KEY SET IS ASSERTED EQUAL TO THE ROSTER'S, so a key added to the
    /// roster with no sample here is a red test rather than a key nobody ever
    /// proved the parser reads. Written out rather than generated, `enabled`
    /// five times included: a generator over the roster would derive this list
    /// from the very thing it is here to check.
    ///
    /// THE TOP-LEVEL SAMPLES ARE INLINE TABLES, which is the same statement in
    /// TOML as the heading each of them would otherwise be written as, and it
    /// is what lets one walk cover a level with no heading of its own.
    const SAMPLE_VALUES: &[(&str, &str, &str)] = &[
        (super::TOP_LEVEL, "daemon", "{ enabled = true }"),
        (super::TOP_LEVEL, "focus", "{ silence = [\"Sleep\"] }"),
        (super::TOP_LEVEL, "lights", "{ refresh_secs = 12 }"),
        (super::TOP_LEVEL, "nag", "{ after_secs = 300 }"),
        (
            super::TOP_LEVEL,
            "plugins",
            "{ hermes = { enabled = true } }",
        ),
        (super::TOP_LEVEL, "recap", "{ digest = true }"),
        ("recap", "digest", "true"),
        ("recap", "digest_as_thread", "true"),
        ("recap", "min_events", "8"),
        ("recap", "replay_card", "true"),
        ("recap", "repos", "[\"webdavis/dotfiles\"]"),
        ("recap", "review_notes", "\"~/.claude/checklist-*.md\""),
        (
            "recap",
            "summarizer",
            "[\"ollama\", \"run\", \"qwen3.5:4b\"]",
        ),
        ("recap", "summarizer_deadline_secs", "240"),
        ("focus", "silence", "[\"Sleep\"]"),
        ("daemon", "enabled", "true"),
        ("nag", "after_secs", "300"),
        ("lights", "blocked", "{ duration_ms = 2000 }"),
        ("lights", "dim", "{ duration_ms = 3000 }"),
        ("lights", "done", "{ duration_ms = 4000 }"),
        ("lights", "failed", "{ duration_ms = 4000 }"),
        ("lights", "lamp", "{ HCL1 = { shows = [\"done\"] } }"),
        ("lights", "loop", "{ threshold_secs = 300 }"),
        ("lights", "refresh_secs", "12"),
        ("lights", "room", "{ Study = { shows = [\"done\"] } }"),
        ("lights", "unread", "{ after_secs = 300 }"),
        ("lights", "zone", "{ Upstairs = { shows = [\"done\"] } }"),
        ("lights.blocked", "duration_ms", "2000"),
        ("lights.blocked", "give_up_after_secs", "57600"),
        ("lights.blocked", "high", "100"),
        ("lights.blocked", "low", "30"),
        ("lights.dim", "duration_ms", "3000"),
        ("lights.dim", "high", "7"),
        ("lights.dim", "low", "1"),
        ("lights.done", "brightness", "100"),
        ("lights.done", "duration_ms", "4000"),
        ("lights.failed", "brightness", "100"),
        ("lights.failed", "duration_ms", "4000"),
        ("lights.loop", "duration_ms", "4000"),
        ("lights.loop", "high", "60"),
        ("lights.loop", "lease_timeout_secs", "3900"),
        ("lights.loop", "low", "10"),
        ("lights.loop", "threshold_secs", "300"),
        ("lights.unread", "after_secs", "300"),
        ("lights.unread", "duration_ms", "4000"),
        ("lights.unread", "high", "60"),
        ("lights.unread", "low", "10"),
        (super::TARGET_KEYS, "dim_behaviours", "[\"blocked\"]"),
        (super::TARGET_KEYS, "dim_window", "\"22:00-07:00\""),
        (super::TARGET_KEYS, "shows", "[\"done\"]"),
        ("plugins.hermes", "enabled", "true"),
        ("plugins.hermes", "key", "\"secret\""),
        ("plugins.hue", "bridge", "\"192.168.1.10\""),
        ("plugins.hue", "enabled", "true"),
        ("plugins.hue", "key", "\"secret\""),
        ("plugins.hue", "quiet_hours", "\"22:00-07:00\""),
        ("plugins.hue", "rooms", "[\"3F - Studio\"]"),
        ("plugins.macos-banner", "enabled", "true"),
        ("plugins.mobile", "enabled", "true"),
        ("plugins.mobile", "mobile_watch_card", "false"),
        ("plugins.mobile", "submit_deadline_secs", "5"),
        ("plugins.mobile", "token", "\"secret\""),
        ("plugins.mobile", "type", "\"moshi\""),
        ("plugins.router", "api_key", "\"secret\""),
        ("plugins.router", "device_hostname", "\"mister\""),
        ("plugins.router", "device_ipv4", "\"192.168.1.9\""),
        ("plugins.router", "device_mac", "\"2e:11:ab:6d:b0:4f\""),
        ("plugins.router", "enabled", "true"),
        ("plugins.router", "router_url", "\"https://192.168.1.1\""),
        ("plugins.router", "stale_alert_channel", "\"priority\""),
        ("plugins.router", "type", "\"unifi\""),
    ];

    #[test]
    fn every_key_the_roster_declares_is_read_by_the_table_that_declares_it() {
        // THE ROSTER IS THE SCHEMA'S ONE STATEMENT and this is what stops it
        // becoming a second, drifting one. A key declared with no arm to read
        // it is refused by that arm, which is a table whose refusal names a
        // key it will not accept; a key an arm reads that the roster does not
        // declare stops working, because the roster is checked first. Both are
        // red, and this walk is the half that catches the first.
        let mut walked: Vec<(&str, &str)> = Vec::new();
        for (table, key, value) in SAMPLE_VALUES.iter().copied() {
            let text = config_writing(table, key, value);
            assert!(
                parse_config(&text).is_ok(),
                "{} declares `{key}` and will not parse it: {:?}",
                shown_as(table),
                parse_config(&text)
            );
            walked.push((table, key));
        }

        // AND THE TWO SETS ARE THE SAME SET, or the walk above proves only
        // whatever half of the roster someone remembered to sample.
        let mut declared: Vec<(&str, &str)> = super::TABLE_KEYS
            .iter()
            .flat_map(|(table, keys)| keys.iter().map(move |key| (*table, *key)))
            .collect();
        walked.sort_unstable();
        declared.sort_unstable();
        assert_eq!(
            walked, declared,
            "every declared key is walked, and no more"
        );
    }

    /// The shipped config, which is the config OF RECORD: this repo's template
    /// is the only pns config anyone has.
    ///
    /// INCLUDED AT COMPILE TIME AND ONLY UNDER `cfg(test)`, so the binary the
    /// apply builds out of the deployed crate (which is the crate alone, no
    /// repo around it) never asks for a file that is not there. Measured both
    /// ways with the path pointed at a file that does not exist: `cargo build
    /// --bin pns` exits 0 because `cfg(test)` is stripped before the macro
    /// expands, and `cargo test --no-run` fails with "couldn't read".
    ///
    /// THE COST IS THAT THE TEST BUILD REACHES FOUR LEVELS OUT OF THE CRATE,
    /// into the repo checkout around it. `cargo test` and `cargo clippy
    /// --all-targets` therefore only work from inside this repo: run either in
    /// the deployed `~/.local/share/pns` and the error is a "couldn't read"
    /// naming a path, which says nothing about why. THE DAY pns MOVES TO ITS
    /// OWN REPO, as it is planned to, this test stops compiling and the
    /// template it reads has to arrive by another road (a copy vendored into
    /// the crate, or a path handed in by the build). No mechanism is built for
    /// that day here; it is written down so it is found by reading rather than
    /// by a build breaking.
    const SHIPPED_TEMPLATE: &str =
        include_str!("../../../../dot_config/pns/private_config.toml.tmpl");

    /// The template with its chezmoi actions taken out: a directive standing on
    /// its own line goes with the line, and an action inside a value becomes
    /// the string the vault would have put there.
    ///
    /// NOT A CHEZMOI, and it does not need to be. What this test reads is which
    /// KEYS the file names and under which tables, and no action in it is a key
    /// or a table; they are one conditional wrapper and five secrets.
    fn rendered_template() -> String {
        super::strip_chezmoi_actions(SHIPPED_TEMPLATE, "\"from-the-vault\"")
            .expect("the shipped template's own actions are well-formed")
    }

    #[test]
    fn the_stub_refuses_a_secret_action_that_forgot_totoml() {
        // THE MUTANT THIS PINS: a template secret line with `| toToml`
        // dropped. Chezmoi would then splice the raw vault bytes in unquoted
        // and the deployed file would not parse, but a stub that swaps ANY
        // action for a quoted placeholder would keep every template test
        // green. So the stub only stands in for the one action grammar the
        // renderer writes, and refuses the rest out loud.
        let error = super::strip_chezmoi_actions(
            "token = {{ (keepassxc \"Moshi :: Webhook Secret\").Password }}",
            "\"from-the-vault\"",
        )
        .expect_err("a bare action with no `| toToml` is not a secret action");
        assert!(error.contains("not a `| toToml` secret action"), "{error}");
    }

    /// THE STUB ONLY READS THE GRAMMAR of a secret action, which is what
    /// keeps a dropped `| toToml` or an unknown field from passing itself off
    /// as vault output. It reads neither WHICH entry a line names nor WHICH
    /// field it takes off that entry, so pointing hue's `bridge` at
    /// `.Password` or a line at another vault entry leaves every other
    /// template test green while the deployed file quietly carries the wrong
    /// credential, and both are one character.
    ///
    /// NOTHING RENDERS THIS TEMPLATE IN A TEST, so its own text is the only
    /// place that agreement can sit until PR S2 generates the file from
    /// `config_text::render` and compares the two byte for byte. The list is
    /// exact rather than a `contains` per line, so a sixth secret appearing,
    /// or one of these five going away, is the same red.
    #[test]
    fn the_shipped_template_names_the_entry_and_field_of_every_secret() {
        let secrets: Vec<&str> = SHIPPED_TEMPLATE
            .lines()
            .filter(|line| line.contains("keepassxc"))
            .collect();
        assert_eq!(
            secrets,
            [
                r#"token = {{ (keepassxc "Moshi :: Webhook Secret").Password | toToml }}"#,
                r#"key = {{ (keepassxc "Hermes :: Webhook Secret :: #pns").Password | toToml }}"#,
                r#"bridge = {{ (keepassxc "OpenHue :: API Key (hue-bridge-pro)").UserName | toToml }}"#,
                r#"key = {{ (keepassxc "OpenHue :: API Key (hue-bridge-pro)").Password | toToml }}"#,
                r#"api_key = {{ (keepassxc "UniFi :: API Key (dresden-udr)").Password | toToml }}"#,
            ]
        );
    }

    #[test]
    fn the_shipped_config_template_still_parses_through_this_schema() {
        // THE FENCE UNDER THE SWEEP. Judging every plugin table's keys can
        // refuse a config that worked yesterday, and the only config that
        // matters is the one this repo ships. If it stops loading, the
        // machine falls back to the CORE with a warning nobody is standing in
        // front of: the phone and the banner keep working, and the durable
        // paper trail, the lights and the home probe all stop.
        let rendered = rendered_template();
        let config = parse_config(&rendered)
            .unwrap_or_else(|error| panic!("the shipped template must load: {error:?}"));
        assert_eq!(
            config.plugins.keys().collect::<Vec<_>>(),
            vec!["hermes", "hue", "macos-banner", "mobile", "router"],
            "and it must still select what it selects"
        );
        // And every one of those names is a plugin that exists, which is the
        // refusal one layer on.
        crate::registry::roster()
            .enabled(&config)
            .expect("the template names only registered plugins");
    }

    #[test]
    fn the_shipped_template_states_the_blocked_backstop_at_its_default_uncommented() {
        // DEFAULTS VISIBLE IN CONFIG (operator ruling): the key fence counts a
        // commented line too, and the parser reads the same number whether the
        // line is there or not, so only the line itself pins the ruling.
        assert!(
            rendered_template()
                .lines()
                .any(|line| line == "give_up_after_secs = 57600"),
            "the template must state the blocked backstop, uncommented, at 57600"
        );
    }

    #[test]
    fn every_key_the_template_documents_is_a_key_the_roster_serves() {
        // THE SCANNER IS THE SHARED ONE, so the template and what `pns setup`
        // composes are held to the roster by the same reader rather than by
        // two that can drift apart. The count is pinned HERE and only here,
        // because the template is the text whose key list is a fixed document.
        assert_eq!(
            super::documented_keys_the_roster_serves(&rendered_template()),
            TEMPLATE_KEY_PAIRS,
            "the scan read a different number of keys than the template documents"
        );
    }

    /// How many `key = value` pairs the scan above finds in the shipped
    /// template, commented lines included.
    ///
    /// EXACT, NOT A FLOOR. The number is here to catch a SCANNER that quietly
    /// stopped reading, and a floor with room under it is a scanner allowed to
    /// lose a quarter of the file and still pass: the scan is whitespace-exact
    /// in two places (`# ` and ` = `), so a template edit writing `key= value`
    /// on a run of lines drops exactly that run and nothing says so.
    ///
    /// THE ONE EDIT THAT MOVES IT is the template documenting a key more or a
    /// key fewer, in which case this number moves with it. A change here for
    /// any other reason is the scan breaking rather than the template changing.
    const TEMPLATE_KEY_PAIRS: usize = 66;

    #[test]
    fn the_doctors_own_wording_names_only_keys_the_router_table_serves() {
        // THE THIRD DOCUMENT. The template says what to write, the refusals say
        // what is wrong with what was written, and the doctor's setup report
        // says which key to go and set; a key renamed in two of the three is an
        // operator sent to a spelling nothing reads.
        //
        // READ OFF THE REPORT ITSELF, never restated here. A test that asserts
        // string literals against the roster's string literals agrees with
        // itself whatever the doctor actually says, which is the exact drift it
        // is named for: rename `router_url` to `url` in the sentence below and
        // nothing else, and a test written that way stays green.
        use crate::home::{DeviceKey, SetupFailure, setup_report};
        let serves = super::keys_of("plugins.router").expect("the router table is in the roster");
        let quoted = "\"x\"".to_string();
        for (failure, sends_the_operator_to_a_key) in [
            (SetupFailure::NoConfigFile, false),
            (SetupFailure::ConfigError("refused".to_string()), false),
            (SetupFailure::NoRouterPlugin, false),
            (SetupFailure::RouterDisabled, true),
            (SetupFailure::NoType, true),
            (SetupFailure::UnknownType("asus".to_string()), true),
            (SetupFailure::InvalidRouterTable, true),
            (SetupFailure::NoDeviceIdentifier, true),
            (
                SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Mac,
                    found: quoted.clone(),
                },
                true,
            ),
            (
                SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Hostname,
                    found: quoted.clone(),
                },
                true,
            ),
            (
                SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Ipv4,
                    found: quoted.clone(),
                },
                true,
            ),
            (SetupFailure::NoApiKey, true),
        ] {
            let said = setup_report(&failure);
            let words: Vec<&str> = said
                .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
                .collect();
            // A WORD CARRYING AN UNDERSCORE IS A CONFIG KEY and nothing else in
            // this vocabulary: the prose around it is English. So one that the
            // table does not serve is a key renamed in the code and left
            // standing here.
            for word in words.iter().filter(|word| word.contains('_')) {
                assert!(
                    serves.contains(word),
                    "the report says `{word}`, which the router table does not serve: {said}"
                );
            }
            // AND THE OTHER DIRECTION, which is the half a spelling check
            // cannot see: a line that is supposed to send the operator to a key
            // has to still name one, or `router_url` became `url` and the
            // sentence now points at nothing.
            assert_eq!(
                words.iter().any(|word| serves.contains(word)),
                sends_the_operator_to_a_key,
                "whether this line names a key the table serves changed: {said}"
            );
        }
    }
}
