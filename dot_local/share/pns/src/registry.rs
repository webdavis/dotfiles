//! The plugin registry: compiled-in channels declare themselves here, and the
//! config selects among them.
//!
//! This is what closes routing's KNOWN LIMIT. A channel no longer appears in
//! core policy by NAME; it registers a name plus a routing DECLARATION (local
//! surface, presence-gated, durable log), and the plan is computed over
//! whatever is registered and enabled. Adding a destination is a registration
//! at the composition root, never an edit to policy.
//!
//! Fail directions: registering the same name twice is refused (two plugins
//! answering one config table is a wiring bug, not a preference), and a config
//! that enables a name nothing registered is refused naming it, because a
//! typo'd plugin name that silently no-ops is a notification quietly turned
//! off, the same failure the config layer refuses everywhere else.

use crate::config::Config;

/// What a plugin declares about WHERE it delivers. The plan is computed from
/// these three properties and nothing else, which is what keeps policy closed
/// to new names while open to new destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Routing {
    /// Delivers to this machine's own surfaces (a banner, a light).
    pub local: bool,
    /// The presence verdict may suppress it (the phone leg today).
    pub presence_gated: bool,
    /// The durable log: what remote-only selects, and synchronously, because
    /// an undelivered log entry is invisible in a way an undelivered alert
    /// is not.
    pub durable: bool,
    /// Whether an EVENT dispatches it at all. False for a plugin the binary
    /// serves in its own mode rather than as a leg (hue's pulse today): it
    /// registers so the config can select it and a typo in its name is still
    /// caught, but no notification ever routes to it.
    pub event_dispatched: bool,
}

/// One registered plugin: its config-table name and its routing declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registration {
    pub name: &'static str,
    pub routing: Routing,
}

/// Why registration or selection was refused, always naming the offender.
#[derive(Debug, PartialEq)]
pub enum RegistryError {
    /// Two plugins claimed the same name.
    Duplicate(String),
    /// The config names a plugin nothing registered, enabled or not: the
    /// typo is the defect either way.
    UnknownPlugin(String),
}

/// A vetted selection, and the only value a plan can be computed over. The
/// inner list is private and no constructor is public, so a Selection can
/// only come out of [`Registry::enabled`]: fabricated registrations cannot
/// reach routing without passing the duplicate and unknown-name refusals.
#[derive(Debug, PartialEq)]
pub struct Selection(Vec<Registration>);

impl Selection {
    pub fn iter(&self) -> std::slice::Iter<'_, Registration> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The ordered set of compiled-in plugins. Registration order is delivery
/// order, so the composition root states the order once and the config cannot
/// scramble it.
#[derive(Debug, Default)]
pub struct Registry {
    registrations: Vec<Registration>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a plugin. A name already taken is refused.
    pub fn register(&mut self, name: &'static str, routing: Routing) -> Result<(), RegistryError> {
        if self.registrations.iter().any(|entry| entry.name == name) {
            return Err(RegistryError::Duplicate(name.to_string()));
        }
        self.registrations.push(Registration { name, routing });
        Ok(())
    }

    /// Every registered name, in registration order.
    pub fn names(&self) -> Vec<&'static str> {
        self.registrations.iter().map(|entry| entry.name).collect()
    }

    /// Every registration, for the machine with NO config file: the built-in
    /// roster is the default, so the cutover from the bash engine changes
    /// nothing until an operator writes a config, and a config that EXISTS
    /// is authoritative precisely because writing one is the opt-in.
    pub fn all(&self) -> Selection {
        Selection(self.registrations.clone())
    }

    /// The registrations the config enables, in REGISTRATION order whatever
    /// order the config listed them in. A config naming an unregistered
    /// plugin is refused; a registered plugin the config omits or disables
    /// is simply not selected.
    pub fn enabled(&self, config: &Config) -> Result<Selection, RegistryError> {
        // The CONFIG's names are walked first, and the enabled flag is not
        // consulted: an unregistered name is a typo whether or not it is
        // switched on, and the next edit turns it into a silent no-op.
        for name in config.plugins.keys() {
            if !self.registrations.iter().any(|entry| entry.name == name) {
                return Err(RegistryError::UnknownPlugin(name.clone()));
            }
        }
        Ok(Selection(
            self.registrations
                .iter()
                .filter(|entry| {
                    config
                        .plugins
                        .get(entry.name)
                        .is_some_and(|selected| selected.enabled)
                })
                .copied()
                .collect(),
        ))
    }
}

/// The real roster, for tests that need the production declarations rather
/// than a made-up pair. Four fixtures used to reproduce this independently,
/// so a declaration could change in the roster and stay green in three of
/// them; there is one now, and the composition root registers the same set.
#[cfg(test)]
pub fn test_roster() -> Registry {
    let mut registry = Registry::new();
    for (name, routing) in ROSTER {
        registry.register(name, routing).unwrap();
    }
    registry
}

/// The declarations the composition root registers, named once so a test can
/// run against the real thing.
pub const ROSTER: [(&str, Routing); 4] = [
    (
        "moshi",
        Routing {
            local: false,
            presence_gated: true,
            durable: false,
            event_dispatched: true,
        },
    ),
    (
        // AHEAD OF THE DURABLE LOG, because this one is presence-sensitive
        // and that one is not. The plan is computed from a reading of where
        // the operator is at dispatch, and hermes can post synchronously
        // against a deadline; delivering the banner after it would show the
        // operator a decision taken about a moment that had passed.
        "macos-banner",
        Routing {
            local: true,
            presence_gated: false,
            durable: false,
            event_dispatched: true,
        },
    ),
    (
        "hermes",
        Routing {
            local: false,
            presence_gated: false,
            durable: true,
            event_dispatched: true,
        },
    ),
    (
        // A local surface the binary drives in its own `pulse` mode. It
        // registers so the config can select it and so a typo in its name is
        // still refused, but no event ever routes to it.
        "hue",
        Routing {
            local: true,
            presence_gated: false,
            durable: false,
            event_dispatched: false,
        },
    ),
];

/// Which plugins run, given what loading the config found. The composition
/// policy in one place:
///
/// A LOADED config is authoritative. A MISSING config selects every built-in,
/// so the cutover from the bash engine changes nothing until an operator
/// opts in by writing one. A BROKEN config (unreadable, malformed, invalid,
/// or naming an unknown plugin) is LOUD, the returned warning, but still
/// selects every built-in: on an always-exit-0 notification path, a config
/// error that silently turned every notification off would be the exact
/// failure the config layer exists to refuse.
pub fn select_plugins(
    registry: &Registry,
    loaded: Result<crate::config::LoadOutcome, crate::config::ConfigError>,
) -> (Selection, Option<String>) {
    use crate::config::LoadOutcome;
    use RegistryError;

    match loaded {
        Ok(LoadOutcome::Loaded(config)) => match registry.enabled(&config) {
            Ok(selection) => (selection, None),
            Err(error) => {
                let detail = match error {
                    RegistryError::UnknownPlugin(name) => format!("unknown plugin `{name}`"),
                    RegistryError::Duplicate(name) => format!("duplicate plugin `{name}`"),
                };
                (registry.all(), Some(roster_warning(&detail)))
            }
        },
        Ok(LoadOutcome::Missing) => (registry.all(), None),
        Err(error) => (registry.all(), Some(roster_warning(error.detail()))),
    }
}

/// The one line a broken config prints: what was wrong, and that nothing was
/// turned off because of it.
fn roster_warning(detail: &str) -> String {
    format!("pns: config error ({detail}); running every built-in plugin")
}

#[cfg(test)]
mod tests {
    use super::Selection;
    use super::{Registry, RegistryError, Routing};
    use crate::config::parse_config;

    const REMOTE_GATED: Routing = Routing {
        local: false,
        presence_gated: true,
        durable: false,
        event_dispatched: true,
    };
    const DURABLE: Routing = Routing {
        local: false,
        presence_gated: false,
        durable: true,
        event_dispatched: true,
    };
    const LOCAL: Routing = Routing {
        local: true,
        presence_gated: false,
        durable: false,
        event_dispatched: true,
    };

    fn three_plugin_registry() -> Registry {
        let mut registry = Registry::new();
        registry.register("moshi", REMOTE_GATED).unwrap();
        registry.register("hermes", DURABLE).unwrap();
        registry.register("macos-banner", LOCAL).unwrap();
        registry
    }

    // --- registration -------------------------------------------------------

    #[test]
    fn registration_order_is_kept_because_it_is_delivery_order() {
        let registry = three_plugin_registry();
        assert_eq!(registry.names(), vec!["moshi", "hermes", "macos-banner"]);
    }

    #[test]
    fn a_name_already_taken_is_refused_naming_it() {
        let mut registry = Registry::new();
        registry.register("moshi", REMOTE_GATED).unwrap();
        assert_eq!(
            registry.register("moshi", LOCAL),
            Err(RegistryError::Duplicate("moshi".to_string()))
        );
    }

    // --- selection by config ------------------------------------------------

    #[test]
    fn the_config_selects_and_registration_order_beats_config_order() {
        // The config lists banner before moshi; the plan order is still the
        // registered one, because delivery order is policy, not preference.
        let config = parse_config(
            "[plugins.macos-banner]\nenabled = true\n[plugins.moshi]\nenabled = true\n",
        )
        .unwrap();
        let enabled = three_plugin_registry().enabled(&config).unwrap();
        let names: Vec<&str> = enabled.iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["moshi", "macos-banner"]);
    }

    #[test]
    fn a_disabled_or_omitted_plugin_is_simply_not_selected() {
        let config =
            parse_config("[plugins.moshi]\nenabled = true\n[plugins.hermes]\nenabled = false\n")
                .unwrap();
        let enabled = three_plugin_registry().enabled(&config).unwrap();
        let names: Vec<&str> = enabled.iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["moshi"]);
    }

    #[test]
    fn an_enabled_name_nothing_registered_is_refused_naming_it() {
        // A typo'd plugin name that silently no-ops is a notification quietly
        // turned off; the registry refuses it the way the config layer
        // refuses unknown keys.
        let config = parse_config("[plugins.mosih]\nenabled = true\n").unwrap();
        assert_eq!(
            three_plugin_registry().enabled(&config),
            Err(RegistryError::UnknownPlugin("mosih".to_string()))
        );
    }

    #[test]
    fn a_disabled_unknown_name_is_still_refused_because_the_typo_is_the_defect() {
        // `enabled = false` on an unknown table is the same typo one edit
        // away from silently disabling a real plugin; refuse it now, while
        // the operator is looking at the file they just edited.
        let config = parse_config("[plugins.mosih]\nenabled = false\n").unwrap();
        assert_eq!(
            three_plugin_registry().enabled(&config),
            Err(RegistryError::UnknownPlugin("mosih".to_string()))
        );
    }

    #[test]
    fn all_selects_every_registration_for_the_unconfigured_machine() {
        let selection = three_plugin_registry().all();
        let names: Vec<&str> = selection.iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["moshi", "hermes", "macos-banner"]);
    }

    #[test]
    fn an_empty_config_selects_nothing_which_is_a_verdict_not_an_error() {
        let config = parse_config("").unwrap();
        let enabled = three_plugin_registry().enabled(&config).unwrap();
        assert!(enabled.is_empty());
    }

    // --- plugin selection at the composition root ---------------------------

    fn selection_names(selection: &Selection) -> Vec<&str> {
        selection.iter().map(|r| r.name).collect()
    }

    #[test]
    fn a_missing_config_selects_every_builtin_so_the_cutover_changes_nothing() {
        use crate::config::LoadOutcome;
        let (selection, warning) =
            super::select_plugins(&super::test_roster(), Ok(LoadOutcome::Missing));
        assert_eq!(
            selection_names(&selection),
            vec!["moshi", "macos-banner", "hermes", "hue"]
        );
        assert_eq!(warning, None);
    }

    #[test]
    fn a_loaded_config_is_authoritative() {
        use crate::config::LoadOutcome;
        let config = parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
        let (selection, warning) =
            super::select_plugins(&super::test_roster(), Ok(LoadOutcome::Loaded(config)));
        assert_eq!(selection_names(&selection), vec!["hermes"]);
        assert_eq!(warning, None);
    }

    #[test]
    fn a_broken_config_is_loud_but_never_turns_notifications_off() {
        use crate::config::ConfigError;
        let (selection, warning) = super::select_plugins(
            &super::test_roster(),
            Err(ConfigError::Malformed(
                "key with no value at line 1".to_string(),
            )),
        );
        assert_eq!(
            selection_names(&selection),
            vec!["moshi", "macos-banner", "hermes", "hue"]
        );
        let warning = warning.expect("a broken config must be said aloud");
        assert!(warning.contains("key with no value"));
    }

    #[test]
    fn a_config_naming_an_unknown_plugin_is_loud_and_falls_back_to_the_roster() {
        use crate::config::LoadOutcome;
        let config = parse_config("[plugins.mosih]\nenabled = true\n").unwrap();
        let (selection, warning) =
            super::select_plugins(&super::test_roster(), Ok(LoadOutcome::Loaded(config)));
        assert_eq!(
            selection_names(&selection),
            vec!["moshi", "macos-banner", "hermes", "hue"]
        );
        let warning = warning.expect("the typo'd name must be said aloud");
        assert!(warning.contains("mosih"));
    }

    #[test]
    fn a_hue_table_selects_hue_like_any_other_plugin_and_warns_about_nothing() {
        // It used to be a string exception stripped before the unknown-name
        // refusal. It is a registration now, so configuring the pulse is
        // ordinary and costs the operator no part of their event selection.
        use crate::config::LoadOutcome;
        let config =
            parse_config("[plugins.hermes]\nenabled = true\n[plugins.hue]\nenabled = true\n")
                .unwrap();
        let (selection, warning) =
            super::select_plugins(&super::test_roster(), Ok(LoadOutcome::Loaded(config)));
        assert_eq!(selection_names(&selection), vec!["hermes", "hue"]);
        assert_eq!(warning, None);
    }

    #[test]
    fn a_true_typo_is_still_refused() {
        use crate::config::LoadOutcome;
        let config = parse_config("[plugins.mosih]\nenabled = true\n").unwrap();
        let (_, warning) =
            super::select_plugins(&super::test_roster(), Ok(LoadOutcome::Loaded(config)));
        assert!(
            warning
                .expect("the typo is still the defect")
                .contains("mosih")
        );
    }
}
