/// The hermes signing key out of the `[plugins.hermes]` settings: `key`,
/// non-empty, else None. Silent, like every not-set-up reading.
pub fn hermes_secret(settings: &toml::Table) -> Option<String> {
    let key = settings.get("key")?.as_str()?;
    (!key.is_empty()).then(|| key.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_way_the_settings_can_fail_to_provide_a_key_reads_not_set_up() {
        for settings in ["", "other = \"x\"\n", "key = \"\"\n", "key = 42\n"] {
            assert_eq!(
                hermes_secret(&settings.parse().unwrap()),
                None,
                "case: {settings:?}"
            );
        }
    }
}
