use super::*;

/// One plugin's slice of the config: the selection flag, and its settings
/// with the flag itself removed, because `enabled` belongs to this layer and
/// everything else belongs to the plugin.
#[derive(Debug, PartialEq)]
pub struct PluginEntry {
    pub enabled: bool,
    pub settings: toml::Table,
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
    /// Exact request classes allowed through mute and named Focus for banner and phone.
    pub bypass_silence_classes: Vec<String>,
    /// `[daemon] enabled`: whether `pns daemon run` stays up and ticks.
    ///
    /// DEFAULT ON, which is the opposite of `[focus]` and of every plugin, and
    /// the difference is that this switch delivers nothing. An idle daemon
    /// reads one empty directory a second. Default OFF would put every feature
    /// that rides the clock behind TWO switches, so an operator who enabled the
    /// feature and saw nothing would have to discover a second, invisible one.
    pub daemon_enabled: bool,
    pub retry_limits: pns_domain::retry::RetryLimits,
    pub retry_backoff: pns_domain::retry::RetryBackoff,
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
    /// Boxed because this is the largest optional policy. Configurations
    /// without lamps do not reserve space for all its fields.
    pub lights: Option<Box<Lights>>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            plugins: BTreeMap::new(),
            recap: Recap::default(),
            focus_silence: Vec::new(),
            bypass_silence_classes: vec!["security".into()],
            daemon_enabled: DEFAULT_DAEMON_ENABLED,
            retry_limits: Default::default(),
            retry_backoff: Default::default(),
            nag_after_secs: NAG_OFF,
            lights: None,
        }
    }
}

impl Config {
    pub fn silence_policy(&self, class: Option<&str>) -> pns_domain::SilencePolicy {
        if class.is_some_and(|class| self.bypass_silence_classes.iter().any(|name| name == class)) {
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
