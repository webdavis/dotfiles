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

/// Which plugins run, given what loading the config found. The composition
/// policy in one place, and it turns on ONE question: did the file parse?
///
/// A LOADED config is authoritative, and that holds even when it names a
/// plugin nothing registered. The typo is LOUD, the returned warning, and the
/// selection is the WHOLE ROSTER: the file parsed, so every credential in it
/// is in hand and the composition root has already read hue's table, hermes's
/// key and the recap off it. Narrowing here would let one mistyped table name
/// cost a fully configured machine its durable paper trail and its lights,
/// which is not what a spelling mistake asked for.
///
/// A MISSING config selects the CORE, which is what a machine nobody has
/// configured gets. A config that could not be READ (unreadable, malformed,
/// invalid) is loud and selects the core too: on an always-exit-0 notification
/// path, a config error that silently turned every notification off would be
/// the exact failure the config layer exists to refuse, and the three it
/// leaves out could not have delivered anything anyway, since their
/// credentials are in the file nobody could read.
///
/// SELECTING ONLY THE KNOWN NAMES out of a config with one typo in it is a
/// third answer, narrower than either of these. It is not built: it would
/// have to decide what a half-honoured config means for every reader that
/// already took a value off the same file, and nothing has asked for it yet.
pub fn select_plugins(
    registry: &Registry,
    loaded: Result<crate::config::LoadOutcome, crate::config::ConfigError>,
) -> (Selection, Option<String>) {
    use crate::config::LoadOutcome;
    use RegistryError;

    match loaded {
        Ok(LoadOutcome::Loaded(config)) => match registry.enabled(&config.plugin_switches()) {
            Ok(selection) => (selection, None),
            Err(error) => {
                let detail = match error {
                    RegistryError::UnknownPlugin(name) => format!("unknown plugin `{name}`"),
                    RegistryError::Duplicate(name) => format!("duplicate plugin `{name}`"),
                    RegistryError::Unsatisfied { plugin, needs } => {
                        format!("`{plugin}` is enabled and needs `{needs}`, which is not")
                    }
                };
                (registry.all(), Some(every_plugin_warning(&detail)))
            }
        },
        Ok(LoadOutcome::Missing) => (registry.core(), None),
        Err(error) => (registry.core(), Some(core_warning(error.detail()))),
    }
}

/// The line a config that PARSED prints when it names a plugin nothing
/// registered: what was wrong, and that nothing was turned off because of it.
fn every_plugin_warning(detail: &str) -> String {
    format!("pns: config error ({detail}); running every built-in plugin")
}

/// The line a config nobody could read prints: what was wrong, and which two
/// plugins carry the machine until it is fixed.
fn core_warning(detail: &str) -> String {
    format!(
        "pns: config error ({detail}); running the core plugins ({})",
        CORE.join(", ")
    )
}

#[cfg(test)]
mod tests;
