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

    /// DOCUMENTS THE RETIRED TABLE NAME, not a new code path.
    /// `[plugins.github.channels]` was the GitHub source design's own map and
    /// is deleted in favour of the one `[plugins.discord.channels]`, so an
    /// operator whose local file still holds it has to be told it moved
    /// rather than left with a map that resolves nothing and a source that
    /// posts to the catch-all.
    ///
    /// THE REFUSAL MOVED WITH THE PLUGIN. `[plugins.github]` is a registered
    /// sensor now, so the unknown-PLUGIN arm no longer answers for this; the
    /// schema's unknown-KEY arm does, which is the louder of the two because
    /// it fails the whole file rather than warning. This test exists to keep
    /// that specific name legible in the suite, not to guard a mutant the
    /// generic case does not already catch.
    #[test]
    fn a_config_still_holding_the_deleted_github_channel_map_is_refused_naming_it() {
        let text = "[plugins.github]\n\
                    enabled = true\n\
                    token = \"ghp-not-a-real-token\"\n\
                    [plugins.github.channels]\n\
                    \"webdavis/dotfiles\" = \"9001\"\n";
        let Err(super::super::ConfigError::Invalid(said)) = super::super::parse_config(text) else {
            panic!("the retired map was accepted");
        };
        assert!(said.contains("channels"), "{said}");
        assert!(said.contains("github"), "{said}");
    }
}
