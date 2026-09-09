use pns_domain::registry::{Registry, Selection};
/// Which plugins run, given what loading the config found.
///
/// THE POLICY MOVED to `pns-application`, over a `ConfigOutcome` it declares
/// itself. What is left here is the mapping from this package's own config
/// result onto those three arms, which is the config edge's job and belongs
/// with the config edge.
pub fn select_plugins(
    registry: &Registry,
    loaded: Result<super::LoadOutcome, super::ConfigError>,
) -> (Selection, Option<String>) {
    use super::LoadOutcome;
    use pns_application::ConfigOutcome;

    let outcome = match loaded {
        Ok(LoadOutcome::Loaded(config)) => ConfigOutcome::Loaded(config.plugin_switches()),
        Ok(LoadOutcome::Missing) => ConfigOutcome::Missing,
        Err(error) => ConfigOutcome::Unreadable(error.detail().to_string()),
    };
    pns_application::select_plugins(registry, outcome)
}
