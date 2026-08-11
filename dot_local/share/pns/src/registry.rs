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
    /// The config enabled a name nothing registered.
    UnknownPlugin(String),
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
        let _ = (name, routing);
        todo!("R2c: refuse duplicates, keep registration order")
    }

    /// Every registered name, in registration order.
    pub fn names(&self) -> Vec<&'static str> {
        todo!("R2c: the names in registration order")
    }

    /// The registrations the config enables, in REGISTRATION order whatever
    /// order the config listed them in. A config naming an unregistered
    /// plugin is refused; a registered plugin the config omits or disables
    /// is simply not selected.
    pub fn enabled(&self, config: &Config) -> Result<Vec<Registration>, RegistryError> {
        let _ = config;
        todo!("R2c: select by config, refuse unknown names, keep registration order")
    }
}

#[cfg(test)]
mod tests {
    use super::{Registration, Registry, RegistryError, Routing};
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
    fn an_empty_config_selects_nothing_which_is_a_verdict_not_an_error() {
        let config = parse_config("").unwrap();
        let enabled: Vec<Registration> = three_plugin_registry().enabled(&config).unwrap();
        assert!(enabled.is_empty());
    }
}
