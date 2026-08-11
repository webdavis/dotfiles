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

#[cfg(test)]
mod tests {
    use super::{Registry, RegistryError, Routing};
    use crate::config::parse_config;

    const REMOTE_GATED: Routing = Routing {
        local: false,
        presence_gated: true,
        durable: false,
    };
    const DURABLE: Routing = Routing {
        local: false,
        presence_gated: false,
        durable: true,
    };
    const LOCAL: Routing = Routing {
        local: true,
        presence_gated: false,
        durable: false,
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
}
