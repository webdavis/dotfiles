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

/// The lamps' policy: how often a state is re-armed, how long a loop must have
/// been working before its lamp breathes, and how faint a dimmed signal runs.
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
#[derive(Debug, Clone, PartialEq)]
pub struct Lights {
    pub refresh_secs: u64,
    pub breathe_after_secs: u64,
    pub dim_brightness: u8,
    /// WHICH activities put the loop lamp on breathing. Every source not named
    /// here contributes nothing to the condition; the ones named still do.
    ///
    /// AN ABSENT KEY IS RESOLVED TO THE FULL SET AT PARSE TIME, which is what
    /// lets this be a plain `Vec`. Absent means every source and an empty list
    /// means breathing off, and a `Vec` cannot tell those apart by itself; the
    /// alternative is an `Option` every reader has to remember to unwrap the
    /// right way round.
    pub breathe_on: Vec<BreatheSource>,
    pub families: BTreeMap<String, Family>,
    pub places: BTreeMap<String, Place>,
}

/// One place's own policy: the behaviours it refuses, the hours it wants
/// quiet, what quiet means there, and whether a state suppressed by those hours
/// is shown afterwards.
///
/// A PLACE IS A ROOM NAME OR A LIGHT NAME, the same vocabulary a family claims
/// in, so an operator writes one spelling of "which lamps" rather than two.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Place {
    pub skip: Vec<Behaviour>,
    pub quiet_hours: Option<String>,
    /// What quiet MEANS here, and `None` IS "SAID NOTHING" for `catch_up`'s
    /// reason: the chain is walked specific first per setting, so a lamp that
    /// wrote `off` has to be able to turn its room's `dim` back off. With a
    /// plain `QuietMode`, "not written" and "written off" were the same value
    /// and the room won either way. Absent at every rung is off, which is the
    /// shipped meaning of quiet hours.
    pub quiet_mode: Option<QuietMode>,
    /// CATCH-UP DEFAULTS OFF (operator ruling): a state that was suppressed
    /// through the night is news nobody wants at 07:00.
    ///
    /// `None` IS "SAID NOTHING", which is what a plain `bool` could not spell.
    /// The chain is walked specific first per setting, so a lamp that wrote
    /// `false` has to be able to turn its room's `true` back off; with a bool,
    /// "not written" and "written false" were the same value and the room won
    /// either way. Absent at every rung is off, which is the ruling above.
    pub catch_up: Option<bool>,
}

/// What a lamp can say. A CLOSED SET, which is the whole reason `[lights]` is
/// judged here instead of passed through as a plugin's free-form settings: a
/// `skip` list holding a word nothing matches is a lamp that keeps signalling
/// something the operator switched off, with no message anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Behaviour {
    Done,
    Failed,
    NeedsYou,
    Breathing,
    Glow,
}

/// The five words, in the spelling a config uses, and the order the refusal
/// lists them in.
pub const BEHAVIOUR_WORDS: [(&str, Behaviour); 5] = [
    ("done", Behaviour::Done),
    ("failed", Behaviour::Failed),
    ("needs-you", Behaviour::NeedsYou),
    ("breathing", Behaviour::Breathing),
    ("glow", Behaviour::Glow),
];

/// One kind of work that can put the loop lamp on breathing.
///
/// A CLOSED SET, judged at load for `Behaviour`'s reason: a `breathe_on`
/// holding a word nothing matches is a lamp that stays dark while the operator
/// is sure they switched it on, with no message anywhere.
///
/// THE TWO AGENT SOURCES DIFFER ONLY IN PATIENCE. `AgentWork` breathes the
/// moment herdr says a workspace is working; `AgentLoops` waits for that to
/// have been true continuously for `breathe_after_secs`. Naming both is
/// harmless and the eager one simply wins.
///
/// THE TWO COMMAND SOURCES DIFFER THE SAME WAY, over the shell's own marker:
/// `Commands` is any tracked command, `LongCommands` only one that has been
/// running past the notifier's long tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BreatheSource {
    AgentWork,
    AgentLoops,
    Commands,
    LongCommands,
}

/// The four words, in the spelling a config uses, and the order the refusal
/// lists them in.
pub const BREATHE_SOURCE_WORDS: [(&str, BreatheSource); 4] = [
    ("agent-work", BreatheSource::AgentWork),
    ("agent-loops", BreatheSource::AgentLoops),
    ("commands", BreatheSource::Commands),
    ("long-commands", BreatheSource::LongCommands),
];

/// What quiet hours DO to a place: take the signal away, or show it faintly.
///
/// `Off` IS THE SIGNAL BEING OFF, not the quiet hours being off. There is no
/// third value meaning "no quiet hours": a place that wants none simply states
/// no `quiet_hours`, so the two cannot disagree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum QuietMode {
    #[default]
    Off,
    Dim,
}

/// One source family's claim on the house: whole rooms, individual lights, and
/// the lights inside a claimed room that it does not want.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Family {
    pub rooms: Vec<String>,
    pub lights: Vec<String>,
    pub except: Vec<String>,
}

impl Default for Lights {
    fn default() -> Self {
        Lights {
            refresh_secs: DEFAULT_REFRESH_SECS,
            breathe_after_secs: DEFAULT_BREATHE_AFTER_SECS,
            dim_brightness: DEFAULT_DIM_BRIGHTNESS,
            // EVERY SOURCE THAT WAITS, which is the breathing the design
            // describes: an agent loop past `breathe_after_secs`, and either
            // command tier. `AgentWork` is left OUT because it breathes on any
            // working workspace with no duration test, and an eager source in
            // the default set wins every time, which would leave
            // `breathe_after_secs` governing nothing at all. It stays available
            // by name, so this narrows the default rather than the vocabulary.
            breathe_on: BREATHE_SOURCE_WORDS
                .iter()
                .map(|(_, source)| *source)
                .filter(|source| *source != BreatheSource::AgentWork)
                .collect(),
            families: BTreeMap::new(),
            places: BTreeMap::new(),
        }
    }
}

/// How often a lamp holding a state is re-armed.
///
/// TWELVE, WHICH IS THE TEMPLATE'S OWN ADVICE and not a round number: breathing
/// asks the bridge for its own breathe, a swell the BRIDGE ends by itself after
/// about fifteen seconds, so a default above that would have the lamp finish its
/// swell and sit dark until the next tick. An operator who writes `[lights]` and
/// nothing else gets breathing rather than a slow blink.
const DEFAULT_REFRESH_SECS: u64 = 12;

/// The floor under it, and it is the TRANSPORT DEADLINE rather than a round
/// number: a tick makes bounded bridge calls whose own limit is ten seconds
/// (`BRIDGE_DEADLINE`), so an interval shorter than one call can start a tick
/// while the last one is still dialling. Below this the knob is asking for a
/// pile of children rather than a faster lamp.
const MIN_REFRESH_SECS: u64 = 10;

/// And the ceiling: the ORDINARY LEASE, which is the longest interval that can
/// still re-arm a lamp before the lease behind it runs out.
///
/// IT IS A LEASE BOUND RATHER THAN A ROUND NUMBER. The tick is registered with
/// `until` at least as far as its own first due second, so a refresh longer than
/// the ordinary lease used to EXTEND that lease to the refresh: an allowed 600
/// seconds bought a ten-minute lease and an allowed day bought a sticky glow
/// nothing was left to clear. Refusing the interval at load is what keeps the
/// two lease lengths the fixed numbers they are documented as.
///
/// A day is not a refresh in any case: past a few minutes the signal has expired
/// and gone dark long before the next arm, so the lamp would be off for
/// virtually the whole interval while the config claimed a state.
const MAX_REFRESH_SECS: u64 = 300;

/// How long a loop must have been working before its lamp breathes. The
/// operator's own figure: fifteen minutes.
const DEFAULT_BREATHE_AFTER_SECS: u64 = 900;

/// The floor. Zero would breathe for every momentary reading of "working",
/// which is what the delay exists to prevent, so one second is the floor and
/// it means "as soon as anything is seen working".
const MIN_BREATHE_AFTER_SECS: u64 = 1;

/// The ceiling. A loop that has been working for a whole day has stalled, and
/// a threshold past that describes a lamp that never breathes at all.
const MAX_BREATHE_AFTER_SECS: u64 = 86_400;

/// The brightness a dimmed signal runs at, in percent.
///
/// ONE, WHICH IS THE OPERATOR'S OWN FIGURE (2026-08-30: one to five percent,
/// ideally one). Drill D4 measured a lamp asked for one percent reporting 1.19,
/// which is its own floor rather than a rounding: the bulb cannot go lower, so
/// this asks for the faintest thing the hardware has.
const DEFAULT_DIM_BRIGHTNESS: u8 = 1;

/// Percent, so the two ends are the two ends. ZERO IS REFUSED rather than read
/// as off: a dark signal is a lamp that says nothing, and the way to say
/// nothing is the place's own `skip` list.
const MIN_DIM_BRIGHTNESS: u8 = 1;
const MAX_DIM_BRIGHTNESS: u8 = 100;

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
                    config
                        .plugins
                        .insert(name, PluginEntry { enabled, settings });
                }
            }
            _ => {
                return Err(ConfigError::Invalid(format!(
                    "unknown top-level key `{key}`"
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
                return Err(ConfigError::Invalid(format!("unknown `recap` key `{key}`")));
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
        match key.as_str() {
            "silence" => silence = modes(&setting)?,
            _ => {
                return Err(ConfigError::Invalid(format!("unknown `focus` key `{key}`")));
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
                return Err(ConfigError::Invalid(format!(
                    "unknown `daemon` key `{key}`"
                )));
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
        match key.as_str() {
            "after_secs" => after_secs = nag_schedule(&setting)?,
            _ => {
                return Err(ConfigError::Invalid(format!("unknown `nag` key `{key}`")));
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

/// `[lights]`, the lamp policy: three scalars, each starting at its default and
/// moved only by a key that states it.
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
        match key.as_str() {
            "refresh_secs" => {
                lights.refresh_secs = bounded(&key, &setting, MIN_REFRESH_SECS, MAX_REFRESH_SECS)?;
            }
            "breathe_after_secs" => {
                lights.breathe_after_secs = bounded(
                    &key,
                    &setting,
                    MIN_BREATHE_AFTER_SECS,
                    MAX_BREATHE_AFTER_SECS,
                )?;
            }
            "breathe_on" => lights.breathe_on = breathe_sources(&setting)?,
            "families" => lights.families = parse_families(&setting)?,
            "places" => lights.places = parse_places(&setting)?,
            "dim_brightness" => {
                let percent = bounded(
                    &key,
                    &setting,
                    MIN_DIM_BRIGHTNESS.into(),
                    MAX_DIM_BRIGHTNESS.into(),
                )?;
                // THE BOUND ABOVE ALREADY HELD, so this cannot fail and a
                // fallback here would be a second, silent answer to a question
                // `bounded` has already refused by name.
                lights.dim_brightness = u8::try_from(percent)
                    .expect("bounded at MAX_DIM_BRIGHTNESS, which is a percent and fits a u8");
            }
            _ => {
                return Err(ConfigError::Invalid(format!(
                    "unknown `lights` key `{key}`"
                )));
            }
        }
    }
    Ok(lights)
}

/// `breathe_on`, refused BY NAME for anything outside the closed set.
fn breathe_sources(stated: &toml::Value) -> Result<Vec<BreatheSource>, ConfigError> {
    let words = strings("lights", "breathe_on", "a list of activity names", stated)?;
    words
        .iter()
        .map(|word| {
            BREATHE_SOURCE_WORDS
                .iter()
                .find(|(spelling, _)| spelling == word)
                .map(|(_, source)| *source)
                .ok_or_else(|| {
                    let known: Vec<&str> =
                        BREATHE_SOURCE_WORDS.iter().map(|(word, _)| *word).collect();
                    ConfigError::Invalid(format!(
                        "`lights` key `breathe_on` names `{word}`, which is nothing the lamps \
                         watch; they watch {}",
                        known.join(", ")
                    ))
                })
        })
        .collect()
}

/// `[lights.families]`, one table per source family.
///
/// THE FAMILY NAMES ARE NOT JUDGED HERE, deliberately. `local`, `github` and
/// `loop` are the three the crate produces today, and only the crate knows
/// that, so a name outside the set is named by `pns doctor` against
/// `hue::KNOWN_FAMILIES` instead: a map is often half written, and a config
/// that refuses to load is a worse answer than a line saying which family
/// nothing routes to. What IS judged is the shape inside it, because a mistyped
/// `room` for `rooms` is a lamp that never lights and says nothing.
fn parse_families(setting: &toml::Value) -> Result<BTreeMap<String, Family>, ConfigError> {
    let Some(table) = setting.as_table() else {
        return Err(ConfigError::Invalid(format!(
            "`lights` key `families` has type `{}`, not a table of families",
            setting.type_str()
        )));
    };
    let mut families = BTreeMap::new();
    for (name, entry) in table {
        let where_it_is = format!("lights.families.{name}");
        let Some(claims) = entry.as_table() else {
            return Err(ConfigError::Invalid(format!(
                "`{where_it_is}` has type `{}`, not a table of claims",
                entry.type_str()
            )));
        };
        let mut family = Family::default();
        for (key, claim) in claims {
            let named = match key.as_str() {
                "rooms" => &mut family.rooms,
                "lights" => &mut family.lights,
                "except" => &mut family.except,
                _ => {
                    return Err(ConfigError::Invalid(format!(
                        "unknown `{where_it_is}` key `{key}`"
                    )));
                }
            };
            *named = places_claimed(&where_it_is, key, claim)?;
        }
        families.insert(name.clone(), family);
    }
    Ok(families)
}

/// `[lights.places]`, one table per room or light with a policy of its own.
///
/// A PLACE NAME IS NOT JUDGED against the bridge here, for `families`' reason:
/// this layer reads a file, and only a bridge listing can say which names
/// exist. The doctor is where an unresolved one is named out loud.
fn parse_places(setting: &toml::Value) -> Result<BTreeMap<String, Place>, ConfigError> {
    let Some(table) = setting.as_table() else {
        return Err(ConfigError::Invalid(format!(
            "`lights` key `places` has type `{}`, not a table of places",
            setting.type_str()
        )));
    };
    let mut places = BTreeMap::new();
    for (name, entry) in table {
        let where_it_is = format!("lights.places.{name}");
        let Some(settings) = entry.as_table() else {
            return Err(ConfigError::Invalid(format!(
                "`{where_it_is}` has type `{}`, not a table of settings",
                entry.type_str()
            )));
        };
        let mut place = Place::default();
        for (key, stated) in settings {
            match key.as_str() {
                "skip" => place.skip = behaviours(&where_it_is, stated)?,
                "quiet_hours" => place.quiet_hours = Some(text(&where_it_is, key, stated)?),
                "quiet_mode" => place.quiet_mode = Some(quiet_mode(&where_it_is, stated)?),
                "catch_up" => {
                    place.catch_up = Some(stated.as_bool().ok_or_else(|| {
                        ConfigError::Invalid(format!(
                            "`{where_it_is}` key `catch_up` has type `{}`, not boolean",
                            stated.type_str()
                        ))
                    })?);
                }
                _ => {
                    return Err(ConfigError::Invalid(format!(
                        "unknown `{where_it_is}` key `{key}`"
                    )));
                }
            }
        }
        places.insert(name.clone(), place);
    }
    Ok(places)
}

/// `skip`, the behaviours a place refuses, each one a word off the closed set.
///
/// THE REFUSAL LISTS THE WHOLE SET, which is worth the extra words here and
/// nowhere else in this file: the failure it prevents is a lamp that goes on
/// signalling something the operator switched off, and the operator's only
/// evidence would be a lamp doing what they told it not to.
fn behaviours(where_it_is: &str, stated: &toml::Value) -> Result<Vec<Behaviour>, ConfigError> {
    let words = strings(where_it_is, "skip", "a list of behaviour names", stated)?;
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
                        "`{where_it_is}` key `skip` names `{word}`, which is no behaviour; \
                         the lamps say {}",
                        known.join(", ")
                    ))
                })
        })
        .collect()
}

/// `quiet_mode`, and only the two words that mean something.
fn quiet_mode(where_it_is: &str, stated: &toml::Value) -> Result<QuietMode, ConfigError> {
    match text(where_it_is, "quiet_mode", stated)?.as_str() {
        "off" => Ok(QuietMode::Off),
        "dim" => Ok(QuietMode::Dim),
        other => Err(ConfigError::Invalid(format!(
            "`{where_it_is}` key `quiet_mode` is `{other}`, which is neither `off` nor `dim`"
        ))),
    }
}

/// One place setting that has to be a string, refused BY NAME and BY TYPE.
fn text(where_it_is: &str, key: &str, stated: &toml::Value) -> Result<String, ConfigError> {
    stated.as_str().map(str::to_string).ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{where_it_is}` key `{key}` has type `{}`, not a string",
            stated.type_str()
        ))
    })
}

/// One claim list: names of rooms or lights as the bridge spells them.
///
/// AN EMPTY NAME IS REFUSED, on `focus`'s own rule rather than a new one: it
/// names no room and no lamp, so it is a claim the operator wrote and pns can
/// never act on. The names themselves are NOT judged here; the bridge is the
/// authority on which exist, and `pns doctor` is where an operator learns which
/// of theirs it does not have.
fn places_claimed(
    where_it_is: &str,
    key: &str,
    claim: &toml::Value,
) -> Result<Vec<String>, ConfigError> {
    let names = strings(where_it_is, key, "a list of room or light names", claim)?;
    if names.iter().any(String::is_empty) {
        return Err(ConfigError::Invalid(format!(
            "`{where_it_is}` key `{key}` names a place that is the empty string, \
             which is no room and no lamp"
        )));
    }
    Ok(names)
}

/// One `[lights]` scalar, refused BY NAME outside its range.
///
/// BOTH ENDS, ALWAYS. A floor alone leaves a value that parses and cannot work;
/// a ceiling alone leaves the same at the other end. Each bound is argued at
/// the constant that holds it, and the refusal echoes both, so an operator
/// reading it learns the range rather than only that they missed it.
fn bounded(key: &str, setting: &toml::Value, low: u64, high: u64) -> Result<u64, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`lights` key `{key}` has type `{}`, not a count between {low} and {high}",
            setting.type_str()
        )));
    };
    if count < low || count > high {
        return Err(ConfigError::Invalid(format!(
            "`lights` key `{key}` is {count}, outside the {low} to {high} range"
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

/// How long pns waits for moshi to acknowledge a submission, from
/// `[plugins.moshi] submit_deadline_secs`, with the default when no key states
/// one.
///
/// IT IS READ OFF MOSHI'S OWN TABLE and nowhere else. Plugin settings reach
/// this layer free-form, so every plugin's table would answer a key spelled
/// this way, and a reader that asked the wrong one would take a number the
/// operator wrote for something else.
///
/// THE REFUSALS ARE LOUD AND NAMED, because the caller falls back to the
/// default and a silent fallback is the operator asking for something, not
/// getting it, and being told nothing.
pub fn submit_deadline(config: &Config) -> Result<Duration, ConfigError> {
    let Some(stated) = config
        .plugins
        .get("moshi")
        .and_then(|moshi| moshi.settings.get("submit_deadline_secs"))
    else {
        return Ok(Duration::from_secs(DEFAULT_SUBMIT_DEADLINE_SECS));
    };
    let Some(count) = stated
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`moshi` key `submit_deadline_secs` has type `{}`, not a count of seconds",
            stated.type_str()
        )));
    };
    if count == 0 {
        return Err(ConfigError::Invalid(
            "`moshi` key `submit_deadline_secs` is 0, which is the bound switched off by \
             accident: a deadline that expires before the daemon can answer costs the phone \
             card on every approval"
                .to_string(),
        ));
    }
    if count > MAX_SUBMIT_DEADLINE_SECS {
        return Err(ConfigError::Invalid(format!(
            "`moshi` key `submit_deadline_secs` is {count}, past the \
             {MAX_SUBMIT_DEADLINE_SECS}-second ceiling"
        )));
    }
    Ok(Duration::from_secs(count))
}

#[cfg(test)]
mod tests {
    use super::{
        BREATHE_SOURCE_WORDS, Behaviour, BreatheSource, ConfigError, Family, Lights, LoadOutcome,
        Place, QuietMode, config_path, load_config, parse_config, submit_deadline,
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
        let config = parse_config("[plugins.hue]\nenabled = true\nroom = \"office\"\n").unwrap();
        let hue = &config.plugins["hue"];
        assert!(hue.enabled);
        assert_eq!(
            hue.settings.get("room").and_then(|v| v.as_str()),
            Some("office")
        );
        assert!(
            !hue.settings.contains_key("enabled"),
            "the selection flag is this layer's, not a setting"
        );
    }

    #[test]
    fn an_absent_enabled_flag_reads_disabled_because_selection_is_explicit() {
        let config = parse_config("[plugins.hue]\nroom = \"office\"\n").unwrap();
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
        let err = parse_config("[plugins.moshi]\ntoken = \"SUPERSECRET\" trailing\n").unwrap_err();
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

    // --- the moshi submission deadline ---------------------------------------

    #[test]
    fn the_moshi_submission_deadline_is_a_count_of_seconds_defaulted_to_five() {
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
            submit_deadline(&parse_config("[plugins.moshi]\nenabled = true\n").unwrap()).unwrap(),
            Duration::from_secs(5),
            "a moshi table that does not state one is the default"
        );
        assert_eq!(
            submit_deadline(&parse_config("[plugins.moshi]\nsubmit_deadline_secs = 30\n").unwrap())
                .unwrap(),
            Duration::from_secs(30),
            "the operator's own number is the bound"
        );
        // OFF MOSHI'S OWN TABLE. Every plugin's settings reach this layer in
        // the same shape, so a reader spelled against the wrong table would
        // take a number the operator wrote for something else, or miss the one
        // they wrote for this.
        assert_eq!(
            submit_deadline(&parse_config("[plugins.hue]\nsubmit_deadline_secs = 30\n").unwrap())
                .unwrap(),
            Duration::from_secs(5),
            "another plugin's key is not moshi's bound"
        );
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
                "[plugins.moshi]\nsubmit_deadline_secs = {stated}\n"
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

    #[test]
    fn a_lights_table_reads_its_three_scalars() {
        let lights = parse_config(
            "[lights]\nrefresh_secs = 25\nbreathe_after_secs = 600\ndim_brightness = 15\n",
        )
        .unwrap()
        .lights
        .expect("a table that was written is a table that was read");
        assert_eq!(lights.refresh_secs, 25);
        assert_eq!(lights.breathe_after_secs, 600);
        assert_eq!(lights.dim_brightness, 15);
    }

    #[test]
    fn no_lights_table_is_none_and_an_empty_one_is_every_default() {
        assert_eq!(
            parse_config("[plugins.hue]\nenabled = true\n")
                .unwrap()
                .lights,
            None,
            "a machine that never wrote the table is DISTINGUISHABLE from one \
             that wrote an empty one: the doctor says different things about them"
        );
        assert_eq!(
            parse_config("[lights]\n").unwrap().lights,
            Some(Box::new(Lights::default())),
            "and an empty table is every default, written out"
        );
        assert_eq!(
            Lights::default().refresh_secs,
            12,
            "and the refresh an operator gets by writing `[lights]` alone is \
             inside the ceiling the template states for smooth breathing, not \
             above it: the bridge ends its own swell at about fifteen seconds"
        );
    }

    #[test]
    fn an_unknown_lights_key_is_refused_by_name() {
        let said = refusal("[lights]\nrefrsh_secs = 20\n");
        assert!(
            said.contains("lights") && said.contains("refrsh_secs"),
            "the table and the misspelling are both named: {said}"
        );
    }

    #[test]
    fn a_lights_scalar_of_the_wrong_type_is_refused_by_name_and_by_type() {
        for (written, key) in [
            ("refresh_secs = \"20\"", "refresh_secs"),
            ("breathe_after_secs = true", "breathe_after_secs"),
            ("dim_brightness = 10.5", "dim_brightness"),
        ] {
            let said = refusal(&format!("[lights]\n{written}\n"));
            assert!(
                said.contains(key) && said.contains("has type"),
                "the key and what was written instead are both named: {said}"
            );
        }
    }

    #[test]
    fn every_lights_scalar_is_bounded_on_both_sides_and_refused_by_name_outside_them() {
        for (key, written) in [
            ("refresh_secs", "0"),
            ("refresh_secs", "1"),
            ("refresh_secs", "301"),
            ("breathe_after_secs", "0"),
            ("breathe_after_secs", "604800"),
            ("dim_brightness", "0"),
            ("dim_brightness", "101"),
        ] {
            let said = refusal(&format!("[lights]\n{key} = {written}\n"));
            assert!(
                said.contains(key) && said.contains(written) && said.contains("range"),
                "the refusal names the key, echoes what was written and states the \
                 range: {said}"
            );
        }
    }

    #[test]
    fn each_bound_is_inclusive_so_the_edge_itself_is_a_working_setting() {
        let edges = parse_config(
            "[lights]\nrefresh_secs = 10\nbreathe_after_secs = 1\ndim_brightness = 1\n",
        )
        .unwrap()
        .lights
        .expect("the floor of every range is inside it");
        assert_eq!(
            (
                edges.refresh_secs,
                edges.breathe_after_secs,
                edges.dim_brightness
            ),
            (10, 1, 1)
        );
        let edges = parse_config(
            "[lights]\nrefresh_secs = 300\nbreathe_after_secs = 86400\ndim_brightness = 100\n",
        )
        .unwrap()
        .lights
        .expect("and so is the ceiling");
        assert_eq!(
            (
                edges.refresh_secs,
                edges.breathe_after_secs,
                edges.dim_brightness
            ),
            (300, 86400, 100)
        );
    }

    #[test]
    fn a_family_claims_rooms_lights_and_exceptions() {
        let lights = parse_config(
            "[lights.families.local]\nrooms = [\"3F - Studio\"]\nexcept = [\"3F - Studio - HCL3\"]\n\
             [lights.families.github]\nlights = [\"3F - Studio - HCL3\"]\n",
        )
        .unwrap()
        .lights
        .expect("a families table is a lights table");
        assert_eq!(
            lights.families["local"],
            Family {
                rooms: vec!["3F - Studio".to_string()],
                lights: Vec::new(),
                except: vec!["3F - Studio - HCL3".to_string()],
            }
        );
        assert_eq!(
            lights.families["github"],
            Family {
                rooms: Vec::new(),
                lights: vec!["3F - Studio - HCL3".to_string()],
                except: Vec::new(),
            }
        );
    }

    #[test]
    fn a_family_claim_that_is_not_a_list_of_names_is_refused_by_name() {
        for written in [
            "rooms = \"3F - Studio\"",
            "lights = 3",
            "except = [true]",
            "rooms = [\"\"]",
        ] {
            let said = refusal(&format!("[lights.families.local]\n{written}\n"));
            assert!(
                said.contains("local"),
                "the refusal names the family that carries it: {said}"
            );
        }
    }

    #[test]
    fn an_unknown_family_key_is_refused_by_name() {
        let said = refusal("[lights.families.local]\nroom = [\"3F - Studio\"]\n");
        assert!(
            said.contains("local") && said.contains("room"),
            "the family and the misspelled key are both named: {said}"
        );
    }

    #[test]
    fn a_place_parses_its_four_keys_and_defaults_the_ones_it_does_not_state() {
        let lights = parse_config(
            "[lights.places.\"3F - Master Bedroom\"]\nskip = [\"breathing\", \"glow\"]\n\
             quiet_hours = \"22:00-07:00\"\nquiet_mode = \"dim\"\ncatch_up = true\n\
             [lights.places.\"3F - Studio\"]\nskip = []\n",
        )
        .unwrap()
        .lights
        .expect("a places table is a lights table");
        assert_eq!(
            lights.places["3F - Master Bedroom"],
            Place {
                skip: vec![Behaviour::Breathing, Behaviour::Glow],
                quiet_hours: Some("22:00-07:00".to_string()),
                quiet_mode: Some(QuietMode::Dim),
                catch_up: Some(true),
            }
        );
        assert_eq!(
            lights.places["3F - Studio"],
            Place::default(),
            "a place that states nothing beyond an empty skip list keeps every \
             default, and catch-up is off among them"
        );
    }

    #[test]
    fn a_skip_word_the_lamps_do_not_speak_is_refused_with_the_closed_set_named() {
        let said = refusal("[lights.places.\"3F - Studio\"]\nskip = [\"breething\"]\n");
        assert!(
            said.contains("3F - Studio") && said.contains("breething"),
            "the place and the misspelling are both named: {said}"
        );
        for word in ["done", "failed", "needs-you", "breathing", "glow"] {
            assert!(
                said.contains(word),
                "and the refusal lists the whole closed set, so the operator does \
                 not have to find it: `{word}` is missing from {said}"
            );
        }
    }

    #[test]
    fn a_place_setting_of_the_wrong_type_or_value_is_refused_by_name() {
        for (written, offender) in [
            ("quiet_mode = \"quiet\"", "quiet"),
            ("quiet_mode = 3", "quiet_mode"),
            ("catch_up = \"yes\"", "catch_up"),
            ("quiet_hours = 2200", "quiet_hours"),
            ("skip = \"breathing\"", "skip"),
            ("catch_ups = true", "catch_ups"),
        ] {
            let said = refusal(&format!("[lights.places.\"3F - Studio\"]\n{written}\n"));
            assert!(
                said.contains("3F - Studio") && said.contains(offender),
                "the place and the offender are both named: {said}"
            );
        }
    }

    #[test]
    fn a_present_malformed_file_is_a_loud_error() {
        let path =
            std::env::temp_dir().join(format!("pns-config-malformed-{}", std::process::id()));
        std::fs::write(&path, "corrupt [").unwrap();
        let outcome = load_config(&path);
        std::fs::remove_file(&path).ok();
        assert!(matches!(outcome, Err(ConfigError::Malformed(_))));
    }

    #[test]
    fn breathe_on_names_which_activities_breathe_and_absent_is_the_thresholded_default() {
        assert_eq!(
            parse_config("[lights]\n")
                .unwrap()
                .lights
                .unwrap()
                .breathe_on,
            vec![
                BreatheSource::AgentLoops,
                BreatheSource::Commands,
                BreatheSource::LongCommands
            ],
            "an operator who said nothing gets breathing that WAITS. `agent-work` \
             breathes on any working workspace with no duration test at all, so \
             defaulting it on would leave `breathe_after_secs` governing nothing \
             while the template beside it says that key is what calls a run a loop"
        );
        assert_eq!(
            parse_config("[lights]\nbreathe_on = [\"agent-work\"]\n")
                .unwrap()
                .lights
                .unwrap()
                .breathe_on,
            vec![BreatheSource::AgentWork],
            "and it is still there for an operator who names it: the default \
             narrows the set, it does not take a source out of the vocabulary"
        );
        assert_eq!(
            parse_config("[lights]\nbreathe_on = [\"agent-loops\", \"long-commands\"]\n")
                .unwrap()
                .lights
                .unwrap()
                .breathe_on,
            vec![BreatheSource::AgentLoops, BreatheSource::LongCommands],
        );
        // AN EMPTY LIST IS BREATHING OFF, and it is a different config from an
        // absent key. A `Vec` cannot tell those apart on its own, which is why
        // the default is resolved to the full set at PARSE time rather than
        // left for a reader to guess at.
        assert!(
            parse_config("[lights]\nbreathe_on = []\n")
                .unwrap()
                .lights
                .unwrap()
                .breathe_on
                .is_empty(),
            "an operator who named no source asked for no breathing"
        );
    }

    #[test]
    fn a_breathe_on_value_outside_the_closed_set_is_refused_at_load_by_name() {
        let said = refusal("[lights]\nbreathe_on = [\"agent-work\", \"agent-loop\"]\n");
        assert!(
            said.contains("breathe_on") && said.contains("agent-loop"),
            "the key and the value are both named: {said}"
        );
        for known in BREATHE_SOURCE_WORDS.iter().map(|(word, _)| *word) {
            assert!(
                said.contains(known),
                "and the closed set is spelled out, or an operator cannot fix it: {said}"
            );
        }
        let said = refusal("[lights]\nbreathe_on = \"agent-work\"\n");
        assert!(
            said.contains("breathe_on"),
            "a bare string is not a list of sources: {said}"
        );
    }
}
