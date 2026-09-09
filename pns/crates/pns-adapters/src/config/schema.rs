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
            "daemon", "delivery", "focus", "lights", "nag", "plugins", "recap",
        ],
    ),
    (
        "recap",
        &[
            "digest",
            "digest_as_thread",
            "min_events",
            "replay_card",
            "repos",
            "review_notes",
            "summarizer",
            "summarizer_deadline_secs",
        ],
    ),
    ("focus", &["silence"]),
    (
        "delivery",
        &[
            "bypass_silence_classes",
            "max_attempts",
            "max_age_secs",
            "retry_base_secs",
        ],
    ),
    ("daemon", &["enabled"]),
    ("nag", &["after_secs"]),
    (
        "lights",
        &[
            "blocked",
            "dim",
            "done",
            "failed",
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
    ("plugins.hermes", &["enabled", "key"]),
    (
        "plugins.hue",
        &["bridge", "enabled", "key", "quiet_hours", "rooms"],
    ),
    (
        "plugins.macos-banner",
        &["click_command", "click_type", "enabled"],
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
            "mobile_watch_card",
            "submit_deadline_secs",
            "token",
            "type",
        ],
    ),
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
