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

#[cfg(test)]
mod tests {
    use super::*;

    /// THE MUTANT THIS PINS: a table for a plugin nothing registered read as
    /// no table at all. `[plugins.github.channels]` was the GitHub source
    /// design's own map and is deleted in favour of the one
    /// `[plugins.discord.channels]`, so an operator whose local file still
    /// holds it has to be told it moved rather than left with a map that
    /// resolves nothing and a source that posts to the catch-all.
    #[test]
    fn a_config_still_holding_the_deleted_github_table_is_refused_naming_it() {
        let text = "[plugins.github.channels]\n\
                    \"webdavis/dotfiles\" = \"9001\"\n";
        let (_, notice) = select_plugins(
            &pns_domain::registry::roster(),
            super::super::parse_config(text)
                .map(|config| super::super::LoadOutcome::Loaded(Box::new(config))),
        );
        let notice = notice.expect("a config naming an unregistered plugin says so");
        assert!(notice.contains("unknown plugin `github`"), "{notice}");
    }
}
