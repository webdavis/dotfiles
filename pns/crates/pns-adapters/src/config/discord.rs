use pns_domain::channel_map::{ChannelMap, DEFAULT_KEY};

/// What the durable log provides the bot: its token and the map of channels
/// it posts to.
///
/// NO `Debug`, and it stays that way, for `HermesKeys`'s stated reason: the
/// token never enters a type that derives one, so it cannot ride a formatted
/// dump into a log line. The channel id keeps it company because it is a
/// keepassxc secret in the shipped config too.
///
/// EVERY WAY THE TABLE CAN FAIL TO STATE ONE READS AS NOT SET UP for that
/// value alone (absent, the wrong type, an empty string), which is
/// `hermes_keys`'s own reading. The destination is what says so out loud,
/// naming the key, so an empty channel does not look like the jobs stopped.
#[derive(Default, Clone)]
pub struct DiscordSettings {
    token: Option<String>,
    channels: ChannelMap,
}

impl DiscordSettings {
    /// The bot token, or None for the not-set-up case.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Every channel the table states, keyed as the operator wrote it. The
    /// ORDER a lookup consults them in is `pns_domain::channel_map`'s, never
    /// this reader's.
    pub fn channels(&self) -> &ChannelMap {
        &self.channels
    }
}

/// The bot's settings out of the `[plugins.log]` table. Silent, like every
/// not-set-up reading.
pub fn discord_settings(settings: &toml::Table) -> DiscordSettings {
    DiscordSettings {
        token: stated(settings.get("bot_token")),
        channels: channel_map(settings),
    }
}

/// `[plugins.log.channels]` as the map the lookup reads: every entry that
/// STATES a channel, and nothing else.
///
/// AN ENTRY THAT STATES NOTHING IS AN ENTRY THAT IS NOT THERE (absent, the
/// wrong type, an empty string), which is `stated`'s reading one level down.
/// A key left blank therefore falls through to the next step of the lookup
/// rather than posting to `""` and earning a 404 per event.
fn channel_map(settings: &toml::Table) -> ChannelMap {
    let Some(channels) = settings.get("channels").and_then(toml::Value::as_table) else {
        return ChannelMap::new();
    };
    channels
        .iter()
        .filter_map(|(key, value)| Some((key.clone(), stated(Some(value))?)))
        .collect()
}

/// Whether an armed table states the catch-all, which is the one key the
/// lookup cannot end without.
pub fn states_default_channel(settings: &toml::Table) -> bool {
    states_channel(settings, DEFAULT_KEY)
}

/// Whether an armed table states a channel under `key`.
///
/// THE KEY IS AN ARGUMENT because the urgent route's name is the operator's
/// (`[routes] urgent`) and a copy compiled in here would check the wrong one
/// the day they rename it.
pub fn states_channel(settings: &toml::Table, key: &str) -> bool {
    channel_map(settings).contains_key(key)
}

/// A non-empty string setting, or None for every way it can fail to be one.
fn stated(value: Option<&toml::Value>) -> Option<String> {
    value
        .and_then(toml::Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_way_the_table_can_fail_to_state_a_value_reads_not_set_up() {
        for settings in [
            "",
            "other = \"x\"\n",
            "bot_token = \"\"\n",
            "bot_token = 42\n",
            "channels = \"c\"\n",
            "[channels]\n",
            "[channels]\ndefault = \"\"\n",
            "[channels]\ndefault = 42\n",
        ] {
            let read = discord_settings(&settings.parse().unwrap());
            assert!(read.token().is_none() || read.channels().is_empty());
            assert!(
                !states_default_channel(&settings.parse().unwrap()),
                "case: {settings:?}"
            );
        }
        let nothing = discord_settings(&"".parse().unwrap());
        assert_eq!(nothing.token(), None);
        assert!(nothing.channels().is_empty());
    }

    #[test]
    fn an_armed_table_states_both_the_token_and_every_channel_it_maps() {
        let read = discord_settings(
            &"type = \"discord\"\nbot_token = \"tok\"\n[channels]\ndefault = \"1234\"\ndotfiles = \"9001\"\nblank = \"\"\n"
                .parse()
                .unwrap(),
        );
        assert_eq!(read.token(), Some("tok"));
        assert_eq!(
            read.channels().get("default").map(String::as_str),
            Some("1234")
        );
        assert_eq!(
            read.channels().get("dotfiles").map(String::as_str),
            Some("9001")
        );
        assert_eq!(
            read.channels().get("blank"),
            None,
            "an entry stating nothing is an entry that is not there"
        );
    }
}
