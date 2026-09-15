/// The one `[plugins.discord] type` a compiled-in backend answers. VALIDATED
/// AND THEN DISCARDED, the way `[plugins.mobile]`'s is: the enum that
/// dispatches between two backends is worth writing the day there are two.
pub const BOT_TYPE: &str = "bot";

/// Whether the discord table names a backend this binary answers, and the
/// REASON when it does not.
///
/// THE SAME QUESTION `mobile_backend` ASKS OF ITS OWN TABLE, worded to match:
/// name the table, quote what was written, name the one type that answers.
/// `discord` is the plugin and `type` is what carries the post, so a table
/// naming none must not be read as this one the day a second transport
/// compiles in.
pub fn discord_backend(settings: &toml::Table) -> Result<(), String> {
    let named = settings
        .get("type")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("no type in [plugins.discord]; the only type is {BOT_TYPE:?}"))?;
    if named != BOT_TYPE {
        return Err(format!(
            "[plugins.discord] has type {named:?}, which no compiled-in backend answers; \
             the only type is {BOT_TYPE:?}"
        ));
    }
    Ok(())
}

/// What `[plugins.discord]` provides the bot: its token and the channel it
/// posts to.
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
    channel: Option<String>,
    /// Why no post can be made: the table is switched on and names a transport
    /// nothing compiled in answers. It travels WITH the reading rather than
    /// collapsing into an absent token, for `Mobile`'s own reason: a backend
    /// nobody answers and a token nobody wrote are two different edits, and
    /// folding both into `None` names `token` for a fault that was `type`.
    refusal: Option<String>,
}

impl DiscordSettings {
    /// The bot token, or None for the not-set-up case.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// The channel id every post goes to. ONE ENTRY TODAY, `channels.default`:
    /// the per-project lookup over the rest of that table is its own slice, and
    /// the catch-all is what a destination with no map needs to post at all.
    pub fn channel(&self) -> Option<&str> {
        self.channel.as_deref()
    }

    /// Why the leg is refused before either seam, or None.
    pub fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
    }

    /// The reading for a table whose `type` no compiled-in transport answers:
    /// no credentials at all, and the reason.
    pub fn refused(reason: String) -> Self {
        Self {
            refusal: Some(reason),
            ..Self::default()
        }
    }
}

/// The bot's settings out of the `[plugins.discord]` table. Silent, like every
/// not-set-up reading.
pub fn discord_settings(settings: &toml::Table) -> DiscordSettings {
    DiscordSettings {
        token: stated(settings.get("token")),
        channel: stated(
            settings
                .get("channels")
                .and_then(toml::Value::as_table)
                .and_then(|channels| channels.get("default")),
        ),
        refusal: None,
    }
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
    fn the_table_has_to_name_a_backend_and_the_refusal_names_the_key() {
        for settings in ["", "token = \"t\"\n", "type = 5\n", "type = \"\"\n"] {
            let complaint = discord_backend(&settings.parse().unwrap())
                .expect_err(&format!("case: {settings:?}"));
            assert!(complaint.contains("type"), "names the key: {complaint}");
            assert!(
                complaint.contains("[plugins.discord]"),
                "and the table: {complaint}"
            );
            assert!(
                complaint.contains(BOT_TYPE),
                "and the one type that answers: {complaint}"
            );
        }
        let complaint = discord_backend(&"type = \"webhook\"\n".parse().unwrap())
            .expect_err("no backend answers `webhook`");
        assert!(complaint.contains("\"webhook\""), "got: {complaint}");
    }

    #[test]
    fn every_way_the_table_can_fail_to_state_a_value_reads_not_set_up() {
        for settings in [
            "",
            "other = \"x\"\n",
            "token = \"\"\n",
            "token = 42\n",
            "channels = \"c\"\n",
            "[channels]\n",
            "[channels]\ndefault = \"\"\n",
        ] {
            let read = discord_settings(&settings.parse().unwrap());
            assert!(read.token().is_none() || read.channel().is_none());
        }
        let nothing = discord_settings(&"".parse().unwrap());
        assert_eq!(nothing.token(), None);
        assert_eq!(nothing.channel(), None);
    }

    #[test]
    fn an_armed_table_states_both_the_token_and_the_catch_all_channel() {
        let read = discord_settings(
            &"type = \"bot\"\ntoken = \"tok\"\n[channels]\ndefault = \"1234\"\n"
                .parse()
                .unwrap(),
        );
        assert_eq!(read.token(), Some("tok"));
        assert_eq!(read.channel(), Some("1234"));
    }
}
