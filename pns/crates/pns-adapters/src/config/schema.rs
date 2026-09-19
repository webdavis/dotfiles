use super::*;

/// EVERY KEY EVERY TABLE SERVES, table by table: the one statement of this
/// schema's vocabulary, and the source of both the refusal that names a
/// mistyped key and the list of what to write instead.
///
/// ONE ROSTER RATHER THAN A LISTING PER REFUSAL, because the failure it exists
/// to prevent is a listing that drifted: a key added to a parse arm and not to
/// the sentence that names the alternatives leaves an operator reading a
/// refusal that omits the key they wanted. Every table checks this before it
/// dispatches, so a key that is not declared here does not work at all, and a
/// key declared here with no arm to read it is refused by that arm and caught
/// by the walk in this module's own tests. Both drifts are red rather than
/// quiet.
///
/// THE PLUGIN TABLES ARE IN IT and their settings are no longer free-form,
/// which is the one behaviour change: a plugin's near miss (`room` for `rooms`,
/// `tokens` for `token`) used to reach the plugin as a setting it did not
/// recognize and cost a destination silently. A table for a plugin nothing
/// registered is NOT here and stays free-form, because this layer has no
/// vocabulary to judge a plugin that does not exist; the registry refuses the
/// NAME, which is the defect in that case.
///
/// THE NESTED ROW IS A PREFIX. `[lights.lamp.<name>]`, `[lights.room.<name>]`
/// and `[lights.zone.<name>]` carry the operator's own names, so the roster
/// holds the part that is the schema's (one row for all three levels) and the
/// refusal names the whole path.
///
/// THE FIRST ROW IS THE FILE'S OWN TOP LEVEL, whose vocabulary is the TABLE
/// names. It is a row like any other so that the refusal an operator gets for a
/// misspelled or a MOVED table prints from the same source every other refusal
/// prints from, and so the walks in this module's tests reach the outermost
/// level too. It is looked up by `TOP_LEVEL` rather than by a name, because it
/// is the one level with no heading to write.
///
/// NO LENGTH IS DECLARED. A row added to a fixed-size array is a two-place
/// edit, and the count says nothing a reader needs.
pub const TABLE_KEYS: &[(&str, &[&str])] = &[
    (
        TOP_LEVEL,
        &[
            "daemon", "delivery", "failures", "focus", "lights", "paths", "phone", "plugins",
            "producer", "quiet", "recap", "remind", "routes", "stale",
        ],
    ),
    (ROUTES, &["default", "urgent"]),
    (
        "recap",
        &[
            "digest",
            "min_events",
            "replay_card",
            "repos",
            "review_notes",
            "summarizer",
            "summarizer_deadline_secs",
        ],
    ),
    ("focus", &["silence"]),
    ("quiet", &["calendar"]),
    (
        "quiet.calendar",
        &["command", "deadline_secs", "enabled", "poll_secs"],
    ),
    (
        "delivery",
        &[
            "bypass_silence_classes",
            "max_attempts",
            "max_age_secs",
            "remote_deadline",
            "retry_base_secs",
        ],
    ),
    ("daemon", &["enabled", "service"]),
    ("paths", &["channels_dir", "state_dir"]),
    ("phone", &["marker_file"]),
    ("remind", &["delay"]),
    // THE NESTED ROW IS A PREFIX, as `lights.<level>` is: `[producer.<name>]`
    // carries the producer's own name, so the roster holds the part that is
    // the schema's and the refusal names the whole path.
    (PRODUCER_KEYS, &["remind"]),
    ("stale", &["escalate_after", "route"]),
    ("failures", &["port", "serve"]),
    (
        "lights",
        &[
            "blocked",
            "dim",
            "done",
            "failed",
            "github",
            "lamp",
            "loop",
            "refresh_secs",
            "room",
            "unread",
            "zone",
        ],
    ),
    ("lights.done", &["brightness", "duration_ms"]),
    ("lights.failed", &["brightness", "duration_ms"]),
    (
        "lights.blocked",
        &["duration_ms", "give_up_after_secs", "high", "low"],
    ),
    ("lights.dim", &["duration_ms", "high", "low"]),
    (
        "lights.github",
        &["brightness", "duration_ms", "fail", "pass"],
    ),
    (
        "lights.unread",
        &["after_secs", "duration_ms", "high", "low"],
    ),
    (
        "lights.loop",
        &[
            "duration_ms",
            "flare",
            "flare_ms",
            "high",
            "lease_timeout_secs",
            "low",
            "threshold_secs",
        ],
    ),
    (TARGET_KEYS, &["dim_behaviours", "dim_window", "shows"]),
    ("plugins.discord", &["channels", "enabled", "token", "type"]),
    // AN OPEN TABLE: the row states the one key the SCHEMA requires, and the
    // rest of its vocabulary is the operator's own project names, which no
    // roster can enumerate. See `OPEN_TABLES`.
    (DISCORD_CHANNELS, &["default"]),
    ("plugins.hermes", &["enabled", "keys", "url"]),
    // AN OPEN TABLE, and the one that decides which routes exist at all: its
    // keys are the ROUTE NAMES the operator's own gateway serves, which no
    // roster compiled into pns can enumerate. See `OPEN_TABLES`.
    (HERMES_KEYS, &[]),
    (
        "plugins.github",
        &[
            "enabled",
            "poll_secs",
            "token",
            "webhook_port",
            "webhook_secret",
        ],
    ),
    (
        "plugins.hue",
        &[
            "bridge",
            "certificate",
            "enabled",
            "key",
            "quiet_hours",
            "rooms",
        ],
    ),
    (
        "plugins.macos-banner",
        &[
            "click_command",
            "click_type",
            "enabled",
            "terminal_bundle_id",
        ],
    ),
    (
        "plugins.presence",
        &[
            "desk_room",
            "desk_stale_after_secs",
            "enabled",
            "exclude",
            "poll_secs",
            "rooms",
            "stale_after_secs",
            "type",
        ],
    ),
    (
        "plugins.mobile",
        &[
            "enabled",
            "image_cards",
            "mobile_watch_card",
            "submit_deadline_secs",
            "token",
            "type",
            "url",
        ],
    ),
    // AN OPEN TABLE: its keys are CARD TYPES, which is the state word an
    // event's producer sent, and pns compiles in no roster of those. The row
    // states the one card type the shipped file shows as an example. See
    // `OPEN_TABLES`.
    (MOBILE_IMAGE_CARDS, &["missed"]),
    (
        "plugins.router",
        &[
            "api_key",
            "device_hostname",
            "device_ipv4",
            "device_mac",
            "enabled",
            "router_url",
            "stale_alert_channel",
            "type",
        ],
    ),
];

/// The roster row for the file's own top level. THE EMPTY NAME, because that
/// level has no heading: an operator writes `[recap]`, never a bracket around
/// the file itself, so there is no name a lookup could use.
pub const TOP_LEVEL: &str = "";

/// The roster row EVERY target declaration shares, whichever of the three
/// levels wrote it.
///
/// ONE ROW FOR THREE LEVELS, because the vocabulary is the same at all of them:
/// a lamp, a room and a zone answer the same questions and differ only in how
/// specific they are. Three rows would be one list to keep in agreement with
/// two others, which is the drift this roster exists to prevent.
pub(super) const TARGET_KEYS: &str = "lights.<level>";

/// The roster row every `[producer.<name>]` table shares, whichever producer
/// wrote it.
pub(super) const PRODUCER_KEYS: &str = "producer.<name>";

/// The channel map, whose keys are PROJECT NAMES.
pub(super) const DISCORD_CHANNELS: &str = "plugins.discord.channels";

/// The per-route signing keys, whose keys are ROUTE NAMES.
pub(super) const HERMES_KEYS: &str = "plugins.hermes.keys";

/// The per-card-type image switches, whose keys are CARD TYPES.
pub(super) const MOBILE_IMAGE_CARDS: &str = "plugins.mobile.image_cards";

/// What the two routes pns selects for itself are called.
pub(super) const ROUTES: &str = "routes";

/// Tables whose vocabulary is the OPERATOR'S rather than this schema's.
///
/// A KEY HERE CANNOT BE REFUSED BY NAME, and that is the trade: a project
/// roster nobody can enumerate is a project roster nobody can spell-check, so
/// `dotfiels = ...` is a channel that never resolves rather than a refusal at
/// load. What the roster still states is the one key the schema itself
/// requires, `default`, and an armed map missing THAT is refused by
/// `refusals::refuse_a_map_without_a_catch_all`.
///
/// THE ROUTE KEYS ARE HERE FOR THE SAME REASON AND ONE MORE (operator ruling,
/// 2026-09-15): a route name belongs to the gateway the operator runs, and a
/// mistyped one is a route with a key and no posts rather than a refusal.
/// What still holds is the safety property the old roster was checked for: a
/// route pns is asked to post to and has no key for is refused at the
/// signature, so one compromised key reaches one channel.
///
/// THE CARD TYPES ARE HERE FOR THE SAME REASON: a card type is the state word
/// a producer sent, and producers are separate tools, so a mistyped one is a
/// card type that never carries an image rather than a refusal at load.
pub(super) const OPEN_TABLES: &[&str] = &[DISCORD_CHANNELS, HERMES_KEYS, MOBILE_IMAGE_CARDS];

/// Whether a table takes keys this schema never declared.
pub(super) fn is_open(table: &str) -> bool {
    OPEN_TABLES.contains(&table)
}

/// What one table serves, or `None` for a table this schema has no vocabulary
/// for (a plugin nothing registered; see `TABLE_KEYS`).
pub(super) fn keys_of(table: &str) -> Option<&'static [&'static str]> {
    TABLE_KEYS
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, keys)| *keys)
}

/// Whether a table admits a key, refusing it BY NAME and with the whole
/// vocabulary spelled out when it does not.
///
/// THE TWO NAMES ARE DIFFERENT ARGUMENTS because a nested table's roster row
/// is a prefix and its refusal has to name the path the operator wrote: an
/// operator told `lights.<level>` has no `dim_windows` would go looking for a
/// table they never typed.
pub(super) fn admits(roster_table: &str, shown_table: &str, key: &str) -> Result<(), ConfigError> {
    if is_open(roster_table) {
        return Ok(());
    }
    match keys_of(roster_table) {
        Some(serves) if !serves.contains(&key) => Err(unknown_key(roster_table, shown_table, key)),
        _ => Ok(()),
    }
}

/// `admits` for a table whose refusal names the table ITSELF, which is every
/// row but the two nested ones. The two-name form earns itself where the names
/// differ and reads as noise where they cannot, so the call site says which
/// case it is rather than repeating an argument.
pub(super) fn admits_flat(table: &str, key: &str) -> Result<(), ConfigError> {
    admits(table, table, key)
}

/// The refusal itself, naming the table, the key, and the whole vocabulary.
///
/// THE LISTING IS THE POINT. A refusal that only says a key is unknown leaves
/// an operator guessing at the spelling, and guessing is what produced the
/// mistyped key; the alternatives are two words away in the same sentence.
pub(super) fn unknown_key(roster_table: &str, shown_table: &str, key: &str) -> ConfigError {
    ConfigError::Invalid(format!(
        "unknown `{shown_table}` key `{key}`; the table serves {}",
        keys_of(roster_table).unwrap_or_default().join(", ")
    ))
}

/// One duration key off a table: `<count><s|m|h>`, inside the range that key
/// allows, refused BY NAME like every other key here.
///
/// THE DOMAIN'S ONE PARSER DOES THE READING, so a duration means the same
/// thing in config as it does on the command line; only the `pns: ` prefix it
/// writes for a terminal is dropped, because a config refusal already carries
/// its own framing.
///
/// ZERO IS CARVED OUT AND IS NOT AN ERROR, for the callers whose key is the
/// switch as well as the timing: `"0s"` is the same statement as leaving the
/// key out, while every other value under the floor is a schedule the
/// operator meant and pns will not run.
pub(super) fn duration_key(
    table: &str,
    key: &str,
    setting: &toml::Value,
    range: RangeInclusive<Duration>,
) -> Result<u64, ConfigError> {
    let Some(text) = setting.as_str() else {
        return Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` has type `{}`, not a duration like \"5m\"",
            setting.type_str()
        )));
    };
    let field = format!("`{table}` key `{key}`");
    if pns_domain::duration::parse_duration(&field, text, Duration::ZERO..=Duration::ZERO).is_ok() {
        return Ok(0);
    }
    pns_domain::duration::parse_duration(&field, text, range)
        .map(|duration| duration.as_secs())
        .map_err(|said| {
            ConfigError::Invalid(said.strip_prefix("pns: ").unwrap_or(&said).to_string())
        })
}
