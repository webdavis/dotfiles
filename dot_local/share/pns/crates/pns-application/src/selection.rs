//! Which plugins run, given what loading the config found.
//!
//! IT TURNS ON ONE QUESTION, and the type below is that question: did the file
//! parse? Everything the config layer knows beyond that answer is the config
//! edge's business, so this crate declares the three outcomes it acts on and
//! the adapter maps its own richer result onto them. Statements: S124.

use pns_domain::registry::{CORE, Registry, RegistryError, Selection};
use std::collections::BTreeMap;

/// What loading the config found, as the selection policy needs it.
///
/// THREE ARMS AND NOT A `Result`, because a config that is MISSING and a
/// config that could not be READ lead to the same selection by different
/// reasoning and only one of them says anything out loud. Collapsing them
/// would put the distinction back at the call site, where two callers would
/// spell it two ways.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigOutcome {
    /// The file parsed, and these are the switches it set. The map is the
    /// whole of what the selection reads off a config, which is what lets this
    /// crate name no config type at all.
    Loaded(BTreeMap<String, bool>),
    /// There is no file, which is what a machine nobody has configured has.
    Missing,
    /// There is a file and nobody could read it, with what was wrong.
    Unreadable(String),
}

/// Which plugins run. The composition policy in one place.
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
/// configured gets. A config that could not be READ is loud and selects the
/// core too: on an always-exit-0 notification path, a config error that
/// silently turned every notification off would be the exact failure the
/// config layer exists to refuse, and the three it leaves out could not have
/// delivered anything anyway, since their credentials are in the file nobody
/// could read.
///
/// SELECTING ONLY THE KNOWN NAMES out of a config with one typo in it is a
/// third answer, narrower than either of these. It is not built: it would
/// have to decide what a half-honoured config means for every reader that
/// already took a value off the same file, and nothing has asked for it yet.
pub fn select_plugins(registry: &Registry, loaded: ConfigOutcome) -> (Selection, Option<String>) {
    match loaded {
        ConfigOutcome::Loaded(switches) => match registry.enabled(&switches) {
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
        ConfigOutcome::Missing => (registry.core(), None),
        ConfigOutcome::Unreadable(detail) => (registry.core(), Some(core_warning(&detail))),
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
