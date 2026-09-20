use super::*;

/// One plugin's slice of the config: the selection flag, and its settings
/// with the flag itself removed, because `enabled` belongs to this layer and
/// everything else belongs to the plugin.
#[derive(Debug, PartialEq)]
pub struct PluginEntry {
    pub enabled: bool,
    pub settings: toml::Table,
}

/// `[paths]`: where this install keeps its state and looks for channel
/// executables, or `None` for each key the file does not name.
///
/// `None` IS NOT A DEFAULT PATH. `channels_dir` NAMED forces every channel
/// onto its executable, so "the file says nothing" and "the file says the
/// usual place" are two different instructions and cannot share a value.
#[derive(Debug, Default, PartialEq)]
pub struct Paths {
    pub state_dir: Option<String>,
    pub channels_dir: Option<String>,
}

/// The whole parsed file. Ordered, so listings and errors are deterministic.
///
/// THE DEFAULT IS WRITTEN OUT rather than derived, for `Recap`'s own reason
/// one type up: `daemon_enabled` is true when nothing says otherwise, and a
/// derived bool would read false and take the clock away from every machine
/// whose config was written before the table existed.
#[derive(Debug, PartialEq)]
pub struct Config {
    pub phone_marker_file: Option<String>,
    /// `[paths]`: the two install-wide directories.
    pub paths: Paths,
    pub plugins: BTreeMap<String, PluginEntry>,
    pub recap: Recap,
    /// `[focus] enabled`: whether the named Focus modes are read at all.
    ///
    /// DEFAULT ON, because the roster beside it is what decides whether
    /// anything is silenced: a machine that never wrote the table names no
    /// mode and is silenced by none. The switch is here for the operator who
    /// wants their list kept and Focus awareness off for a week.
    pub focus_enabled: bool,
    /// `[focus] modes`: the Focus MODE NAMES that silence, each written
    /// either as the name Control Center shows or as a raw `modeIdentifier`.
    ///
    /// EMPTY IS NOTHING SILENCED, which is what every machine that never
    /// wrote the table gets. It is the ROSTER and never the switch; read them
    /// together through `focus_silence`.
    pub focus_modes: Vec<String>,
    /// `[delivery_class.<name>]`: what each delivery class a producer may send
    /// DOES, keyed by the class name.
    ///
    /// THE WHOLE STATEMENT OF WHICH CLASSES EXIST. pns compiles in none, so a
    /// class no table here defines is refused rather than delivered as a
    /// guess, and a message naming no class takes `DEFAULT_DELIVERY_CLASS`'s
    /// table. Empty is a file that defined none, which is every message on
    /// the default route with the mute respected.
    pub delivery_classes: BTreeMap<String, DeliveryClass>,
    /// `[delivery] remote_deadline`: how long ONE remote call may take, in
    /// seconds, before the caller stops waiting on it. Zero is no deadline at
    /// all, which is the caller's own instruction rather than a default.
    pub remote_deadline_secs: u64,
    /// `[gateway] enabled`: whether `pns gateway run` stays up and ticks.
    ///
    /// DEFAULT ON, which is the opposite of every plugin, and the difference
    /// is that this switch delivers nothing. An idle daemon
    /// reads one empty directory a second. Default OFF would put every feature
    /// that rides the clock behind TWO switches, so an operator who enabled the
    /// feature and saw nothing would have to discover a second, invisible one.
    pub gateway_enabled: bool,
    /// `[gateway] service`: the launchd label `pns gateway` starts, stops,
    /// restarts and reports on. NO DEFAULT: pns compiles in no label of its
    /// own, since it does not know what a given installation's plist is
    /// named. `None` is what every gateway verb refuses on.
    pub gateway_service: Option<String>,
    /// `[routes]`: what the two routes pns selects for itself are called.
    ///
    /// NOT AN OPTION and not a list: both names are defaulted, so a file with
    /// no table and a file writing the defaults are the same statement, and
    /// every OTHER route this machine posts to is named by the producer that
    /// raised the event and granted a key in `[plugins.log.keys]`.
    pub routes: pns_domain::routes::Routes,
    pub retry_limits: pns_domain::retry::RetryLimits,
    pub retry_backoff: pns_domain::retry::RetryBackoff,
    /// `[producer.<name>] remind`: which producers asked for the reminder,
    /// keyed by the name the producer sends.
    ///
    /// A NAME THE OPERATOR WROTE ON PURPOSE, which is the whole point: the
    /// reminder used to be switched on by pns matching the sender's name
    /// against a compiled-in one, so a producer could not ask for it and could
    /// not turn it off. A name with no entry here asks for nothing, and a
    /// per-call `--remind` beats whatever this says.
    pub producer_remind: BTreeMap<String, bool>,
    /// `[remind] delay`: how long an unanswered approval waits before it is
    /// carded a second time, in whole seconds off the key's duration.
    ///
    /// AN UNSET KEY IS THE FEATURE OFF, and `"0s"` is refused by name rather
    /// than read as the same statement: a key that doubles as its own switch
    /// is a value the operator has to decode, and the absent key already says
    /// it.
    ///
    /// DEFAULT OFF, unlike `[gateway]` beside it, and the difference is that
    /// this one INTERRUPTS. It also needs three separate operator steps before
    /// it works (an apply for the hook declaration, the daemon running, and
    /// this key), and a default-on feature that silently does nothing until all
    /// three are done is a mystery rather than a default.
    pub remind_delay_secs: u64,
    /// `[stale] enabled`: whether the page about a blocked session is raised
    /// at all. DEFAULT ON, for `DEFAULT_ESCALATE_AFTER_SECS`'s reason.
    ///
    /// A SWITCH OF ITS OWN, because the window beside it cannot be the
    /// switch: unset is an hour rather than off, so there would be no value
    /// left to mean off with `"0s"` refused.
    pub stale_enabled: bool,
    /// `[stale] escalate_after`: how long a session stays blocked before ONE
    /// page about it is raised, in whole seconds off the key's duration.
    /// Read it with the switch through `stale_window_secs`.
    ///
    /// DEFAULT ON AT AN HOUR, unlike `remind_delay_secs` above it: see
    /// `DEFAULT_ESCALATE_AFTER_SECS`.
    pub stale_escalate_after_secs: u64,
    /// `[stale] route`: the route that page takes, or None for the one the
    /// health kind resolves to against `[routes]`.
    pub stale_route: Option<String>,
    /// `[storage] busy_deadline`: how long a writer waits for the state
    /// database's write lock before the operation is refused. ZERO IS NO WAIT
    /// AT ALL, which is what SQLite's own busy handler does when it is off.
    pub storage_busy_deadline: Duration,
    /// `[lights]`: the lamp policy, or None when no table was written.
    ///
    /// Boxed because this is the largest optional policy. Configurations
    /// without lamps do not reserve space for all its fields.
    pub lights: Option<Box<Lights>>,
    /// `[quiet.calendar]`: the calendar that switches the mute on and off.
    ///
    /// NOT AN OPTION, like `failures` below it: every key is defaulted, so a
    /// file with no table and a file writing the defaults are one statement.
    pub quiet_calendar: QuietCalendar,
    /// `[failures]`: whether the failure record is served as a page, and where.
    ///
    /// NOT AN OPTION, unlike `lights` above it: both its keys are defaulted, so
    /// a file with no table and a file writing the defaults are the same
    /// statement and there is nothing for `None` to mean.
    pub failures: Failures,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            phone_marker_file: None,
            paths: Paths::default(),
            plugins: BTreeMap::new(),
            recap: Recap::default(),
            focus_enabled: DEFAULT_FOCUS_ENABLED,
            focus_modes: Vec::new(),
            delivery_classes: BTreeMap::new(),
            remote_deadline_secs: DEFAULT_REMOTE_DEADLINE_SECS,
            routes: pns_domain::routes::Routes::default(),
            gateway_enabled: DEFAULT_GATEWAY_ENABLED,
            gateway_service: None,
            retry_limits: Default::default(),
            retry_backoff: Default::default(),
            producer_remind: BTreeMap::new(),
            remind_delay_secs: REMIND_OFF,
            stale_enabled: DEFAULT_STALE_ENABLED,
            stale_escalate_after_secs: DEFAULT_ESCALATE_AFTER_SECS,
            stale_route: None,
            storage_busy_deadline: DEFAULT_BUSY_DEADLINE,
            lights: None,
            quiet_calendar: QuietCalendar::default(),
            failures: Failures::default(),
        }
    }
}

impl Config {
    /// The Focus modes that silence right now: the roster while the switch is
    /// on, and nothing at all while it is off.
    ///
    /// ONE READING FOR EVERY CALLER, so a reader that forgot the switch
    /// cannot exist: an empty list is already "nothing silences", which is
    /// what every reader of the roster does with an off switch anyway.
    pub fn focus_silence(&self) -> &[String] {
        if self.focus_enabled {
            &self.focus_modes
        } else {
            &[]
        }
    }

    /// How long a block stands before it is paged about, and zero while the
    /// switch is off, which is `WINDOW_OFF`'s own reading.
    pub fn stale_window_secs(&self) -> u64 {
        if self.stale_enabled {
            self.stale_escalate_after_secs
        } else {
            0
        }
    }

    /// What the class a message carries says, with a message naming none
    /// reading `[delivery_class.default]`.
    ///
    /// `None` IS TWO DIFFERENT FACTS AND ONE ANSWER: a class this file never
    /// defined, and a file that defines no default. `refuses_delivery_class`
    /// is what tells the first apart, because only that one is the operator's
    /// mistake.
    pub fn delivery_class(&self, named: &str) -> Option<&DeliveryClass> {
        self.delivery_classes.get(if named.is_empty() {
            DEFAULT_DELIVERY_CLASS
        } else {
            named
        })
    }

    /// Whether this file refuses the class a message named.
    ///
    /// A MESSAGE NAMING NO CLASS IS NEVER REFUSED: naming none is a complete
    /// statement, and a file with no `[delivery_class.default]` has simply
    /// written no rule for it.
    pub fn refuses_delivery_class(&self, named: &str) -> bool {
        !named.is_empty() && !self.delivery_classes.contains_key(named)
    }

    /// Whether the class a message carries lets it through the mute and the
    /// named Focus modes for the banner and the phone card.
    pub fn silence_policy(&self, named: &str) -> pns_domain::SilencePolicy {
        if self
            .delivery_class(named)
            .is_some_and(|class| class.bypass_mute)
        {
            pns_domain::SilencePolicy::BypassBannerAndPhone
        } else {
            pns_domain::SilencePolicy::Respect
        }
    }

    /// Which plugin names the file mentions, and whether each is switched on.
    ///
    /// THE ONLY THING SELECTION READS off a config, handed over as itself so
    /// the registry states what it needs rather than taking the whole parsed
    /// file. Every name is carried, enabled or not, because an unregistered
    /// name is a typo either way and that refusal is what this feeds.
    pub fn plugin_switches(&self) -> BTreeMap<String, bool> {
        self.plugins
            .iter()
            .map(|(name, entry)| (name.clone(), entry.enabled))
            .collect()
    }
}

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
    Loaded(Box<Config>),
}
