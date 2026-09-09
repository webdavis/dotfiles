use super::*;

// --- [lights] -----------------------------------------------------------

/// The Invalid refusal's own sentence, or a panic naming what came instead.
fn refusal(text: &str) -> String {
    match parse_config(text) {
        Err(ConfigError::Invalid(message)) => message,
        other => panic!("expected a named refusal, got {other:?}"),
    }
}

/// The `[lights]` table one config parses to.
fn lights(text: &str) -> Lights {
    *parse_config(text)
        .expect("this config parses")
        .lights
        .expect("and carries a lights table")
}

// --- every table's own vocabulary ---------------------------------------

/// The header a config writes to reach one roster table. Two of the rows
/// name a NESTED table whose own name is the operator's, so the roster
/// holds the prefix and this picks a name to write under it.
fn header_for(table: &str) -> String {
    match table {
        // THE ONE ROSTER ROW WITH NO HEADING OF ITS OWN: three levels share
        // it, so a sample writes whichever of them, and the refusal names
        // the path the operator wrote rather than this row.
        super::TARGET_KEYS => "lights.room.\"3F - Studio\"".to_string(),
        other => other.to_string(),
    }
}

/// A config that writes one key under one roster row. THE TOP-LEVEL ROW HAS
/// NO HEADING: its keys are the six table names, written bare at the start
/// of the file, which is the shape of every misspelled or moved table.
fn config_writing(table: &str, key: &str, value: &str) -> String {
    match table {
        super::TOP_LEVEL => format!("{key} = {value}\n"),
        other => format!(
            "[{}]\n{key} = {value}\n{}",
            header_for(other),
            companion(table, key)
        ),
    }
}

/// The one key that cannot stand alone, written beside its sample.
///
/// `dim_behaviours` NAMES WHAT RUNS DIMMED INSIDE A WINDOW, and a
/// declaration that states it without one is refused by name. This walk
/// asks whether the arm READS the key, so its sample writes the window the
/// key depends on rather than the walk reading a refusal as a key nothing
/// serves.
fn companion(table: &str, key: &str) -> &'static str {
    match (table, key) {
        (super::TARGET_KEYS, "dim_behaviours") => "dim_window = \"22:00-07:00\"\n",
        _ => "",
    }
}

/// How a refusal from one roster row names the level it refused: every
/// bracketed table by its own header, and the top level as what it is.
fn shown_as(table: &str) -> String {
    match table {
        super::TOP_LEVEL => "top-level".to_string(),
        other => format!("`{}`", header_for(other)),
    }
}

/// The header text a refusal from one roster row carries, which for the
/// three levels sharing a row is the PATH THE OPERATOR WROTE rather than the
/// row's own name: an operator told `lights.<level>` has no `dim_hours`
/// would go looking for a table they never typed.
fn refusal_names(table: &str) -> String {
    match table {
        super::TARGET_KEYS => "`lights.room.3F - Studio`".to_string(),
        other => shown_as(other),
    }
}

// --- [plugins.presence] --------------------------------------------------

/// A presence table with `hue` beside it, which is what the registry
/// insists on before the sensor is selected at all.
fn presence_config(body: &str) -> Config {
    parse_config(&format!(
        "[plugins.hue]\nenabled = true\n[plugins.presence]\nenabled = true\n{body}"
    ))
    .unwrap()
}

// --- the roster, the template and the doctor's wording ------------------

/// One valid value for every key the roster declares, which is what makes
/// the walk below a real parse rather than a name check.
///
/// ITS KEY SET IS ASSERTED EQUAL TO THE ROSTER'S, so a key added to the
/// roster with no sample here is a red test rather than a key nobody ever
/// proved the parser reads. Written out rather than generated, `enabled`
/// five times included: a generator over the roster would derive this list
/// from the very thing it is here to check.
///
/// THE TOP-LEVEL SAMPLES ARE INLINE TABLES, which is the same statement in
/// TOML as the heading each of them would otherwise be written as, and it
/// is what lets one walk cover a level with no heading of its own.
const SAMPLE_VALUES: &[(&str, &str, &str)] = &[
    (super::TOP_LEVEL, "daemon", "{ enabled = true }"),
    (
        super::TOP_LEVEL,
        "delivery",
        "{ bypass_silence_classes = [\"custom\"], max_attempts = 3 }",
    ),
    ("delivery", "bypass_silence_classes", "[\"custom\"]"),
    ("delivery", "max_attempts", "3"),
    ("delivery", "max_age_secs", "7"),
    ("delivery", "retry_base_secs", "7"),
    (super::TOP_LEVEL, "focus", "{ silence = [\"Sleep\"] }"),
    (super::TOP_LEVEL, "lights", "{ refresh_secs = 12 }"),
    (super::TOP_LEVEL, "nag", "{ after_secs = 300 }"),
    (
        super::TOP_LEVEL,
        "plugins",
        "{ hermes = { enabled = true } }",
    ),
    (super::TOP_LEVEL, "recap", "{ digest = true }"),
    ("recap", "digest", "true"),
    ("recap", "digest_as_thread", "true"),
    ("recap", "min_events", "8"),
    ("recap", "replay_card", "true"),
    ("recap", "repos", "[\"webdavis/dotfiles\"]"),
    ("recap", "review_notes", "\"~/.claude/checklist-*.md\""),
    (
        "recap",
        "summarizer",
        "[\"ollama\", \"run\", \"qwen3.5:4b\"]",
    ),
    ("recap", "summarizer_deadline_secs", "240"),
    ("focus", "silence", "[\"Sleep\"]"),
    ("daemon", "enabled", "true"),
    ("nag", "after_secs", "300"),
    ("lights", "blocked", "{ duration_ms = 2000 }"),
    ("lights", "dim", "{ duration_ms = 3000 }"),
    ("lights", "done", "{ duration_ms = 4000 }"),
    ("lights", "failed", "{ duration_ms = 4000 }"),
    ("lights", "lamp", "{ HCL1 = { shows = [\"done\"] } }"),
    ("lights", "loop", "{ threshold_secs = 300 }"),
    ("lights", "refresh_secs", "12"),
    ("lights", "room", "{ Study = { shows = [\"done\"] } }"),
    ("lights", "unread", "{ after_secs = 300 }"),
    ("lights", "zone", "{ Upstairs = { shows = [\"done\"] } }"),
    ("lights.blocked", "duration_ms", "2000"),
    ("lights.blocked", "give_up_after_secs", "57600"),
    ("lights.blocked", "high", "100"),
    ("lights.blocked", "low", "30"),
    ("lights.dim", "duration_ms", "3000"),
    ("lights.dim", "high", "7"),
    ("lights.dim", "low", "1"),
    ("lights.done", "brightness", "100"),
    ("lights.done", "duration_ms", "4000"),
    ("lights.failed", "brightness", "100"),
    ("lights.failed", "duration_ms", "4000"),
    ("lights.loop", "duration_ms", "4000"),
    ("lights.loop", "flare", "100"),
    ("lights.loop", "flare_ms", "200"),
    ("lights.loop", "high", "80"),
    ("lights.loop", "lease_timeout_secs", "3900"),
    ("lights.loop", "low", "10"),
    ("lights.loop", "threshold_secs", "300"),
    ("lights.unread", "after_secs", "300"),
    ("lights.unread", "duration_ms", "4000"),
    ("lights.unread", "high", "60"),
    ("lights.unread", "low", "10"),
    (super::TARGET_KEYS, "dim_behaviours", "[\"blocked\"]"),
    (super::TARGET_KEYS, "dim_window", "\"22:00-07:00\""),
    (super::TARGET_KEYS, "shows", "[\"done\"]"),
    ("plugins.hermes", "enabled", "true"),
    ("plugins.hermes", "key", "\"secret\""),
    ("plugins.hue", "bridge", "\"192.168.1.10\""),
    ("plugins.hue", "enabled", "true"),
    ("plugins.hue", "key", "\"secret\""),
    ("plugins.hue", "quiet_hours", "\"22:00-07:00\""),
    ("plugins.hue", "rooms", "[\"3F - Studio\"]"),
    (
        "plugins.macos-banner",
        "click_command",
        "\"/usr/bin/open {id}\"",
    ),
    ("plugins.macos-banner", "click_type", "\"herdr\""),
    ("plugins.macos-banner", "enabled", "true"),
    ("plugins.presence", "enabled", "true"),
    ("plugins.presence", "desk_room", "\"3F - Studio\""),
    ("plugins.presence", "desk_stale_after_secs", "120"),
    ("plugins.presence", "exclude", "[\"3F - MBedroom\"]"),
    ("plugins.presence", "poll_secs", "5"),
    ("plugins.presence", "rooms", "[\"3F - Studio\"]"),
    ("plugins.presence", "stale_after_secs", "15"),
    ("plugins.presence", "type", "\"hue\""),
    ("plugins.mobile", "enabled", "true"),
    ("plugins.mobile", "mobile_watch_card", "false"),
    ("plugins.mobile", "submit_deadline_secs", "5"),
    ("plugins.mobile", "token", "\"secret\""),
    ("plugins.mobile", "type", "\"moshi\""),
    ("plugins.router", "api_key", "\"secret\""),
    ("plugins.router", "device_hostname", "\"mister\""),
    ("plugins.router", "device_ipv4", "\"192.168.1.9\""),
    ("plugins.router", "device_mac", "\"2e:11:ab:6d:b0:4f\""),
    ("plugins.router", "enabled", "true"),
    ("plugins.router", "router_url", "\"https://192.168.1.1\""),
    ("plugins.router", "stale_alert_channel", "\"priority\""),
    ("plugins.router", "type", "\"unifi\""),
];

mod daemon;
mod delivery;
mod failure_wording;
mod focus;
mod lights_bounds;
mod lights_defaults;
mod lights_motion;
mod lights_targets;
mod loading;
mod mobile;
mod nag;
mod presence_intervals;
mod presence_rooms;
mod presence_shape;
mod reading;
mod recap_sources;
mod recap_summarizer;
mod recap_switches;
mod recap_threshold;
mod roster;
mod schema;
mod vocabulary;

#[test]
fn delivery_retry_settings_are_accepted_and_bad_limits_are_refused() {
    assert!(
        parse_config("[delivery]\nmax_attempts = 3\nmax_age_secs = 7\n").is_ok(),
        "delivery retry limits must load"
    );
    let parsed = parse_config("[delivery]\nmax_attempts = 3\nmax_age_secs = 7\n").unwrap();
    assert_eq!(
        (
            parsed.retry_limits.max_attempts,
            parsed.retry_limits.max_age_secs
        ),
        (3, 7)
    );
    assert_eq!(
        parse_config("").unwrap().retry_limits,
        pns_domain::retry::RetryLimits {
            max_attempts: 20,
            max_age_secs: 604800
        }
    );
    assert_eq!(
        parse_config("[delivery]\nmax_attempts = 0\nmax_age_secs = 0")
            .unwrap()
            .retry_limits
            .max_attempts,
        0
    );
    for value in ["-1", "1.5", "true", "\"20\""] {
        assert!(parse_config(&format!("[delivery]\nmax_attempts = {value}\n")).is_err());
    }
}

#[test]
fn the_delivery_backoff_takes_its_one_base_and_defaults_to_a_minute() {
    let configured = parse_config("[delivery]\nretry_base_secs = 7\n").unwrap();
    assert_eq!(configured.retry_backoff.base_secs, 7);
    assert_eq!(parse_config("").unwrap().retry_backoff.base_secs, 60);
    for invalid in ["-1", "1.5", "true", "\"secret\"", "[]"] {
        assert!(parse_config(&format!("[delivery]\nretry_base_secs = {invalid}\n")).is_err());
    }
}

/// The retired jitter key is REFUSED, not ignored. A key that parses and
/// changes nothing reads as configured behavior, so an operator who still has
/// `retry_random_secs` in their file learns it is gone rather than believing a
/// spread is still applied.
#[test]
fn the_retired_jitter_key_is_refused_by_name() {
    let refusal = parse_config("[delivery]\nretry_random_secs = 0\n").unwrap_err();
    assert!(
        format!("{refusal:?}").contains("retry_random_secs"),
        "the refusal must name the key: {refusal:?}"
    );
}
