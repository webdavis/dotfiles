use super::*;
#[test]
fn every_armed_feature_reaches_the_parsed_config_carrying_its_own_answers() {
    let text = compose_config(&every_feature_armed());
    // THE MIRROR OF THE DECLINED CASE: an armed table stands uncommented,
    // or the walk collected an answer and then commented it away.
    for armed in DECLINABLE_TABLES {
        assert!(
            text.contains(&format!("\n{armed}")),
            "`{armed}` was armed and written commented out:\n{text}"
        );
    }
    let config = parsed(&text);
    assert_eq!(
        config.plugins.keys().collect::<Vec<_>>(),
        vec!["hermes", "hue", "macos-banner", "mobile", "router"]
    );
    assert!(config.plugins.values().all(|plugin| plugin.enabled));
    assert_eq!(
        config.plugins["mobile"].settings["token"].as_str(),
        Some("moshi-secret")
    );
    assert_eq!(
        config.plugins["hermes"].settings["key"].as_str(),
        Some("hermes-secret")
    );
    let hue = &config.plugins["hue"].settings;
    assert_eq!(hue["bridge"].as_str(), Some("192.168.1.9"));
    assert_eq!(hue["key"].as_str(), Some("hue-secret"));
    assert_eq!(
        hue["rooms"]
            .as_array()
            .map(|rooms| rooms.iter().filter_map(|room| room.as_str()).collect()),
        Some(vec!["Studio", "Kitchen"])
    );
    let router = &config.plugins["router"].settings;
    assert_eq!(router["type"].as_str(), Some("unifi"));
    assert_eq!(router["router_url"].as_str(), Some("https://192.168.1.1"));
    assert_eq!(router["api_key"].as_str(), Some("router-secret"));
    assert_eq!(router["device_hostname"].as_str(), Some("phone"));
    assert_eq!(config.focus_silence, vec!["Sleep".to_string()]);
    assert_eq!(config.nag_after_secs, 300);
}

#[test]
fn a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one() {
    // AN EMPTY VALUE PARSES AS ABSENT and delivers nothing while reading
    // as configured, which is the silent failure the walk must never
    // write. Every required field of a feature is tried on its own, so a
    // feature armed by one of two credentials cannot slip through.
    for (blank, declined) in [
        (
            (|answers: &mut Answers| answers.hermes_key.clear()) as fn(&mut Answers),
            "hermes",
        ),
        (|answers: &mut Answers| answers.hue_bridge.clear(), "hue"),
        (|answers: &mut Answers| answers.hue_key.clear(), "hue"),
        (|answers: &mut Answers| answers.hue_rooms.clear(), "hue"),
        (|answers: &mut Answers| answers.router_url.clear(), "router"),
        (
            |answers: &mut Answers| answers.router_api_key.clear(),
            "router",
        ),
        (
            |answers: &mut Answers| answers.router_device_hostname.clear(),
            "router",
        ),
    ] {
        let mut answers = every_feature_armed();
        blank(&mut answers);
        let config = parsed(&compose_config(&answers));
        // ONE FEATURE GOES AND THE REST STAY: a blank answer must not cost
        // the walk anything the operator did fill in.
        let armed: Vec<&str> = config.plugins.keys().map(String::as_str).collect();
        assert!(
            !armed.contains(&declined),
            "a blank credential armed `{declined}` anyway"
        );
        assert_eq!(
            armed.len(),
            4,
            "blanking `{declined}` cost more than `{declined}`: {armed:?}"
        );
        for plugin in config.plugins.values() {
            for (key, value) in &plugin.settings {
                assert_ne!(value.as_str(), Some(""), "`{key}` was written empty");
            }
        }
    }
}

#[test]
fn a_backend_the_home_probe_cannot_answer_declines_the_probe_rather_than_arming_it() {
    // THE SILENT NOTHING THIS WALK EXISTS TO PREVENT. Every key of the
    // router table is free text to the parser, so a backend name nothing
    // implements composes a file that loads, is reported as written, and
    // then refuses at the first probe with a type no compiled-in backend
    // answers. WHAT ACCEPTS IT IS ASKED rather than restated: the day a
    // second backend lands, `router_settings` is what has to agree.
    let armed = parsed(&compose_config(&every_feature_armed()));
    crate::config::router_settings(&armed.plugins["router"].settings)
        .expect("an armed walk writes a table the home probe can answer");

    let unanswerable = Answers {
        router_type: "asus".to_string(),
        ..every_feature_armed()
    };
    let config = parsed(&compose_config(&unanswerable));
    assert!(
        !config.plugins.contains_key("router"),
        "a backend nothing answers was written as an armed probe"
    );
}
