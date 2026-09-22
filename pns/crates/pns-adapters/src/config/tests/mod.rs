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
        // THE SAME SHAPE ONE LEVEL SHALLOWER: each row is a prefix and the
        // operator's own class or producer name is what a sample writes under
        // it.
        super::DELIVERY_CLASS_KEYS => "delivery_class.security".to_string(),
        super::PRODUCER_KEYS => "producer.claude".to_string(),
        super::PROFILE_KEYS => "profiles.night".to_string(),
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
            "{}[{}]\n{key} = {value}\n{}",
            prelude(table),
            header_for(other),
            companion(table, key)
        ),
    }
}

/// The one key that cannot stand alone, written beside its sample.
///
/// `dim_behaviours` NAMES WHAT RUNS DIMMED INSIDE A WINDOW, and a declaration
/// that states it with no window anywhere is refused by name. This walk asks
/// whether the arm READS the key, so its sample writes the window the key
/// depends on rather than the walk reading a refusal as a key nothing serves.
fn companion(table: &str, key: &str) -> &'static str {
    match (table, key) {
        (super::TARGET_KEYS, "dim_behaviours") => "dim_window = \"22:00-07:00\"\n",
        // THE DURABLE LOG IS FILED UNDER ITS TRANSPORT, so every other key of
        // that table needs the `type` naming one or the file is refused before
        // the key under test is read at all.
        // A RULE NAMES A PROFILE THE FILE DEFINES, so the sample writes the
        // one it names beside it rather than being read as a rule pointing
        // at nothing.
        ("profiles", "rules") => "default = { quiet = false }\n",
        ("plugins.log", "type") => "",
        ("plugins.log", _) => "type = \"hermes\"\n",
        // THE THREE GOOGLE CREDENTIALS ARE READ TOGETHER OR NOT AT ALL, so a
        // sample of one writes the type that reads it and the other two.
        ("quiet.calendar", "client_id") => {
            "type = \"google\"\nclient_secret = \"secret\"\nrefresh_token = \"refresh\"\n"
        }
        ("quiet.calendar", "client_secret") => {
            "type = \"google\"\nclient_id = \"id\"\nrefresh_token = \"refresh\"\n"
        }
        ("quiet.calendar", "refresh_token") => {
            "type = \"google\"\nclient_id = \"id\"\nclient_secret = \"secret\"\n"
        }
        _ => "",
    }
}

/// The table a nested row cannot stand without, written above its heading.
///
/// `[plugins.log.channels]` DECLARES THE DURABLE LOG by writing it, so the
/// `type` that says which transport it is has to come first.
fn prelude(table: &str) -> &'static str {
    match table {
        "plugins.log.channels" | "plugins.log.keys" => "[plugins.log]\ntype = \"hermes\"\n",
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
        super::DELIVERY_CLASS_KEYS => "`delivery_class.security`".to_string(),
        super::PRODUCER_KEYS => "`producer.claude`".to_string(),
        other => shown_as(other),
    }
}

// --- [plugins.presence] --------------------------------------------------

/// A presence table with `hue` beside it, which is what the registry
/// insists on before the sensor is selected at all.
fn presence_config(body: &str) -> Config {
    parse_config(&format!(
        "[plugins.lights]\nenabled = true\n[plugins.presence]\nenabled = true\n{body}"
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
    (super::TOP_LEVEL, "paths", "{ state_dir = '~/state' }"),
    ("paths", "state_dir", "'~/state'"),
    ("paths", "channels_dir", "'/opt/pns/channels'"),
    (super::TOP_LEVEL, "gateway", "{ enabled = true }"),
    (super::TOP_LEVEL, "delivery", "{ max_retries = 3 }"),
    (
        super::TOP_LEVEL,
        "delivery_class",
        "{ security = { bypass_mute = true } }",
    ),
    (super::DELIVERY_CLASS_KEYS, "route", "'pages'"),
    (super::DELIVERY_CLASS_KEYS, "bypass_mute", "true"),
    ("delivery", "event_max_age", "'7m'"),
    ("delivery", "max_retries", "3"),
    ("delivery", "remote_deadline", "5"),
    ("delivery", "retry_step", "'7s'"),
    (super::TOP_LEVEL, "failures", "{ page_enabled = true }"),
    ("failures", "page_enabled", "true"),
    ("failures", "page_port", "8646"),
    (super::TOP_LEVEL, "focus", "{ modes = [\"Sleep\"] }"),
    (super::TOP_LEVEL, "lights", "{ arm_interval = \"12s\" }"),
    (
        super::TOP_LEVEL,
        "quiet",
        "{ calendar = { enabled = false } }",
    ),
    ("quiet", "calendar", "{ enabled = false }"),
    ("quiet.calendar", "enabled", "false"),
    ("quiet.calendar", "type", "\"command\""),
    ("quiet.calendar", "command", "[\"busy-window\"]"),
    ("quiet.calendar", "calendars", "[\"primary\"]"),
    ("quiet.calendar", "client_id", "\"id\""),
    ("quiet.calendar", "client_secret", "\"secret\""),
    ("quiet.calendar", "refresh_token", "\"refresh\""),
    ("quiet.calendar", "poll_interval", "\"2m\""),
    ("quiet.calendar", "deadline", "\"20s\""),
    (
        super::TOP_LEVEL,
        "producer",
        "{ claude = { remind = true } }",
    ),
    (super::PRODUCER_KEYS, "remind", "true"),
    (super::TOP_LEVEL, "remind", "{ delay = \"5m\" }"),
    (super::TOP_LEVEL, "stale", "{ escalate_after = \"1h\" }"),
    (super::TOP_LEVEL, "storage", "{ busy_deadline = \"5s\" }"),
    (
        super::TOP_LEVEL,
        "plugins",
        "{ log = { enabled = true, type = \"hermes\" } }",
    ),
    (
        super::TOP_LEVEL,
        "profiles",
        "{ default = { quiet = false } }",
    ),
    ("profiles", "location_poll", "\"30s\""),
    ("profiles", "locations", "{ home = \"00:11:22:aa:bb:cc\" }"),
    ("profiles", "rules", "[{ profile = \"default\" }]"),
    (super::PROFILE_KEYS, "quiet", "true"),
    (super::PROFILE_KEYS, "banner", "\"all\""),
    (super::PROFILE_KEYS, "discord", "\"priority\""),
    (super::PROFILE_KEYS, "phone", "\"all\""),
    (super::PROFILE_KEYS, "lights", "\"none\""),
    (super::TOP_LEVEL, "recap", "{ post_window_recap = true }"),
    (super::TOP_LEVEL, "routes", "{ urgent = \"sirens\" }"),
    ("routes", "default", "\"logbook\""),
    ("routes", "urgent", "\"sirens\""),
    ("recap", "minimum_events", "8"),
    ("recap", "post_window_recap", "true"),
    ("recap", "replay_card", "true"),
    ("recap", "nightshift", "[\"22:00\", \"06:00\"]"),
    ("recap", "morning", "[\"06:00\", \"12:00\"]"),
    ("recap", "afternoon", "[\"12:00\", \"17:00\"]"),
    ("recap", "evening", "[\"17:00\", \"22:00\"]"),
    ("recap", "week_starts_on", "\"monday\""),
    ("recap", "rows_per_section", "8"),
    ("recap", "sources", "{ tasks = [\"dam\", \"ls\"] }"),
    (
        "recap.sources",
        "pull_requests",
        "[\"gh\", \"pr\", \"list\"]",
    ),
    ("recap.sources", "commits", "[\"git\", \"log\"]"),
    ("recap.sources", "tasks", "[\"dam\", \"ls\"]"),
    ("recap.sources", "applies", "[\"chezmoi\", \"status\"]"),
    ("recap", "retain", "\"720h\""),
    ("recap", "review_notes_glob", "\"~/.claude/checklist-*.md\""),
    ("recap", "summarizer", "{ type = \"claude\" }"),
    ("recap", "pregenerate", "[\"morning\"]"),
    ("recap.summarizer", "type", "\"claude\""),
    ("recap.summarizer", "command", "[\"my-model\"]"),
    ("recap.summarizer", "model", "\"haiku\""),
    ("recap.summarizer", "deadline", "\"4m\""),
    ("recap.summarizer", "transcripts", "true"),
    ("recap.summarizer", "transcript_bytes_per_session", "8192"),
    ("recap.summarizer", "transcript_bytes_total", "65536"),
    ("recap.summarizer", "prompt", "\"say what moved\""),
    ("recap.summarizer", "prompt_file", "\"/tmp/instruction\""),
    ("focus", "enabled", "true"),
    ("focus", "modes", "[\"Sleep\"]"),
    ("gateway", "enabled", "true"),
    ("gateway", "service", "'com.example.pns-daemon'"),
    ("remind", "delay", "\"5m\""),
    ("stale", "enabled", "true"),
    ("stale", "escalate_after", "\"1h\""),
    ("stale", "route", "\"priority\""),
    ("storage", "busy_deadline", "\"5s\""),
    ("lights", "blocked", "{ duration = \"2s\" }"),
    ("lights", "checks", "{ duration = \"4s\" }"),
    ("lights", "dim", "{ duration = \"3s\" }"),
    ("lights", "done", "{ duration = \"4s\" }"),
    ("lights", "failed", "{ duration = \"4s\" }"),
    ("lights", "lamp", "{ HCL1 = { behaviours = [\"done\"] } }"),
    ("lights", "arm_interval", "\"12s\""),
    ("lights", "loop", "{ arm_after = \"5m\" }"),
    ("lights", "room", "{ Study = { behaviours = [\"done\"] } }"),
    ("lights", "dim_window", "\"22:00-07:00\""),
    ("lights", "unseen", "{ arm_after = \"5m\" }"),
    (
        "lights",
        "zone",
        "{ Upstairs = { behaviours = [\"done\"] } }",
    ),
    ("lights.blocked", "duration", "\"2s\""),
    ("lights.blocked", "lease_expiry", "\"16h\""),
    ("lights.blocked", "high_percent", "100"),
    ("lights.blocked", "low_percent", "30"),
    ("lights.dim", "duration", "\"3s\""),
    ("lights.dim", "high_percent", "7"),
    ("lights.dim", "low_percent", "1"),
    ("lights.checks", "brightness_percent", "100"),
    ("lights.checks", "duration", "\"4s\""),
    ("lights.checks", "fail_color", "[0.5562, 0.4084]"),
    ("lights.checks", "pass_color", "[0.2725, 0.1283]"),
    ("lights.done", "brightness_percent", "100"),
    ("lights.done", "duration", "\"4s\""),
    ("lights.failed", "brightness_percent", "100"),
    ("lights.failed", "duration", "\"4s\""),
    ("lights.loop", "arm_after", "\"5m\""),
    ("lights.loop", "duration", "\"4s\""),
    ("lights.loop", "flare_percent", "100"),
    ("lights.loop", "flare_duration", "\"200ms\""),
    ("lights.loop", "high_percent", "80"),
    ("lights.loop", "lease_expiry", "\"65m\""),
    ("lights.loop", "low_percent", "10"),
    ("lights.unseen", "arm_after", "\"5m\""),
    ("lights.unseen", "duration", "\"4s\""),
    ("lights.unseen", "high_percent", "60"),
    ("lights.unseen", "low_percent", "10"),
    (super::TARGET_KEYS, "behaviours", "[\"done\"]"),
    (super::TARGET_KEYS, "dim_behaviours", "[\"blocked\"]"),
    (super::TARGET_KEYS, "dim_window", "\"22:00-07:00\""),
    ("plugins.log", "channels", "{ default = \"9001\" }"),
    ("plugins.log", "enabled", "true"),
    ("plugins.log", "keys", "{ pns-events = \"secret\" }"),
    ("plugins.log", "bot_token", "\"secret\""),
    ("plugins.log", "type", "\"hermes\""),
    (
        "plugins.log",
        "url",
        "\"http://127.0.0.1:8644/webhooks/pns-events\"",
    ),
    ("plugins.log.channels", "default", "\"9001\""),
    ("plugins.phone.image_cards", "missed", "true"),
    ("plugins.lights", "bridge_host", "\"192.168.1.10\""),
    (
        "plugins.lights",
        "certificate",
        "\"sha256:0000000000000000000000000000000000000000000000000000000000000001\"",
    ),
    ("plugins.lights", "enabled", "true"),
    ("plugins.lights", "api_key", "\"secret\""),
    ("plugins.lights", "type", "\"hue\""),
    ("plugins.banner", "click_command", "\"/usr/bin/open {id}\""),
    ("plugins.banner", "click_type", "\"herdr\""),
    ("plugins.banner", "enabled", "true"),
    (
        "plugins.banner",
        "terminal_bundle_id",
        "\"com.mitchellh.ghostty\"",
    ),
    ("plugins.banner", "type", "\"macos\""),
    ("plugins.presence", "enabled", "true"),
    ("plugins.presence", "desk_room", "\"3F - Studio\""),
    ("plugins.presence", "desk_input_max_age", "\"2m\""),
    ("plugins.presence", "excluded_rooms", "[\"3F - MBedroom\"]"),
    ("plugins.github", "enabled", "true"),
    (
        "plugins.github",
        "personal_access_token",
        "\"ghp-not-a-real-token\"",
    ),
    ("plugins.github", "poll_interval", "\"60s\""),
    ("plugins.github", "webhook_secret", "\"a-webhook-secret\""),
    ("plugins.github", "webhook_port", "8648"),
    ("plugins.presence", "poll_interval", "\"5s\""),
    ("plugins.presence", "reading_max_age", "\"15s\""),
    ("plugins.presence", "rooms", "[\"3F - Studio\"]"),
    ("plugins.presence", "type", "\"hue\""),
    ("plugins.phone", "enabled", "true"),
    ("plugins.phone", "image_cards", "{ missed = true }"),
    ("plugins.phone", "card_while_watching", "false"),
    ("plugins.phone", "ack_deadline", "\"5s\""),
    ("plugins.phone", "marker_file", "'~/attention'"),
    ("plugins.phone", "device_token", "\"secret\""),
    (
        "plugins.phone",
        "url",
        "\"https://api.getmoshi.app/api/webhook\"",
    ),
    ("plugins.phone", "type", "\"moshi\""),
    ("plugins.home_presence", "api_key", "\"secret\""),
    ("plugins.home_presence", "device_hostname", "\"mister\""),
    ("plugins.home_presence", "device_ipv4", "\"192.168.1.9\""),
    (
        "plugins.home_presence",
        "device_mac",
        "\"2e:11:ab:6d:b0:4f\"",
    ),
    ("plugins.home_presence", "enabled", "true"),
    ("plugins.home_presence", "url", "\"https://192.168.1.1\""),
    ("plugins.home_presence", "alert_route", "\"priority\""),
    ("plugins.home_presence", "type", "\"unifi\""),
];

mod credentials;
mod delivery;
mod failure_wording;
mod focus;
mod gateway;
mod lights_bounds;
mod lights_checks;
mod lights_defaults;
mod lights_motion;
mod lights_targets;
mod loading;
mod phone;
mod presence_intervals;
mod presence_rooms;
mod presence_shape;
mod reading;
mod recap_sources;
mod recap_summarizer;
mod recap_switches;
mod recap_threshold;
mod remind;
mod roster;
mod schema;
mod stale;
mod storage;
mod vocabulary;

#[test]
fn delivery_retry_settings_are_accepted_and_bad_limits_are_refused() {
    let parsed = parse_config("[delivery]\nmax_retries = 3\nevent_max_age = \"7m\"\n")
        .expect("delivery retry limits must load");
    assert_eq!(
        (
            parsed.retry_limits.max_retries,
            parsed.retry_limits.event_max_age_secs
        ),
        (3, 420)
    );
    assert_eq!(
        parse_config("").unwrap().retry_limits,
        pns_domain::retry::RetryLimits {
            max_retries: 20,
            event_max_age_secs: 604800
        }
    );
    // ZERO IS CARVED OUT of both, as it is for every other duration key: no
    // retry at all, and an event that expires the moment it has any age.
    assert_eq!(
        parse_config("[delivery]\nmax_retries = 0\nevent_max_age = \"0s\"")
            .unwrap()
            .retry_limits,
        pns_domain::retry::RetryLimits {
            max_retries: 0,
            event_max_age_secs: 0
        }
    );
    for value in ["-1", "1.5", "true", "\"20\""] {
        assert!(parse_config(&format!("[delivery]\nmax_retries = {value}\n")).is_err());
    }
    for value in ["-1", "1.5", "true", "20", "\"20\"", "\"1s\"", "\"31d\""] {
        assert!(
            parse_config(&format!("[delivery]\nevent_max_age = {value}\n")).is_err(),
            "{value}"
        );
    }
}

#[test]
fn the_delivery_backoff_takes_its_one_step_and_defaults_to_a_minute() {
    let configured = parse_config("[delivery]\nretry_step = \"7s\"\n").unwrap();
    assert_eq!(configured.retry_backoff.step_secs, 7);
    assert_eq!(parse_config("").unwrap().retry_backoff.step_secs, 60);
    for invalid in ["-1", "1.5", "true", "7", "\"secret\"", "[]", "\"2h\""] {
        assert!(
            parse_config(&format!("[delivery]\nretry_step = {invalid}\n")).is_err(),
            "{invalid}"
        );
    }
}

/// The spellings these three keys replaced. Each is refused by name, and the
/// listing that comes back carries the word to write instead.
#[test]
fn the_delivery_keys_these_replaced_are_refused_by_name_with_the_new_spelling_listed() {
    for (retired, replacement) in [
        ("max_attempts = 3", "max_retries"),
        ("max_age_secs = 7", "event_max_age"),
        ("retry_base_secs = 7", "retry_step"),
    ] {
        let error = refusal(&format!("[delivery]\n{retired}\n"));
        assert!(
            error.contains(retired.split(' ').next().unwrap()),
            "{error}"
        );
        assert!(error.contains(replacement), "{error}");
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
