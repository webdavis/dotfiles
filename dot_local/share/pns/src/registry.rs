//! What the config SELECTS out of the registry, and the warnings a config
//! nobody could read prints.
//!
//! The registry itself moved to `pns-domain`. `select_plugins` stays because
//! it takes `LoadOutcome`, a config-edge type, and so do the tests below,
//! which build a real parsed config on purpose. The routing fixtures and the
//! three-plugin registry are spelled in both places until the config edge
//! moves and the two halves rejoin.

pub use pns_domain::registry::{
    CORE, PRESENCE, PluginKind, ROSTER, Registration, Registry, RegistryError, Routing, Selection,
    roster,
};
/// Which plugins run, given what loading the config found.
///
/// THE POLICY MOVED to `pns-application`, over a `ConfigOutcome` it declares
/// itself. What is left here is the mapping from this package's own config
/// result onto those three arms, which is the config edge's job and belongs
/// with the config edge.
pub fn select_plugins(
    registry: &Registry,
    loaded: Result<crate::config::LoadOutcome, crate::config::ConfigError>,
) -> (Selection, Option<String>) {
    use crate::config::LoadOutcome;
    use pns_application::selection::ConfigOutcome;

    let outcome = match loaded {
        Ok(LoadOutcome::Loaded(config)) => ConfigOutcome::Loaded(config.plugin_switches()),
        Ok(LoadOutcome::Missing) => ConfigOutcome::Missing,
        Err(error) => ConfigOutcome::Unreadable(error.detail().to_string()),
    };
    pns_application::selection::select_plugins(registry, outcome)
}

#[cfg(test)]
mod tests;
