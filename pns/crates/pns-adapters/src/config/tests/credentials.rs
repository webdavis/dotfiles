use super::*;

/// A pin the plugin table under test needs before it will load.
const PIN: &str =
    "certificate = \"sha256:0000000000000000000000000000000000000000000000000000000000000001\"\n";

/// Each single credential, as the table that holds it, the key that names it
/// now, and the key that named it before (operator ruling, 2026-09-17): a
/// credential is named for the kind of secret its own tool issues, spelled
/// out, taking the wording from the vault entry that already states it.
const CREDENTIALS: [(&str, &str, &str); 5] = [
    ("plugins.phone", "device_token", "token"),
    ("plugins.log", "bot_token", "token"),
    ("plugins.github", "personal_access_token", "token"),
    ("plugins.lights", "api_key", "key"),
    ("plugins.home_presence", "api_key", "api_key"),
];

/// The config one credential's table needs around it before the key under
/// test is reached at all.
fn armed(table: &str) -> String {
    match table {
        "plugins.phone" => "type = \"moshi\"\n".to_string(),
        "plugins.log" => "type = \"discord\"\n".to_string(),
        "plugins.lights" => format!("type = \"hue\"\nbridge_host = \"192.168.1.10\"\n{PIN}"),
        "plugins.home_presence" => "type = \"unifi\"\n".to_string(),
        _ => String::new(),
    }
}

/// The value one plugin's credential reads back as, through the resolved
/// config and the reader that plugin's own destination uses.
fn credential(config: &Config, table: &str) -> Option<String> {
    // THE DURABLE LOG IS FILED UNDER ITS TRANSPORT, never under the heading
    // that declared it, so the lookup asks for the name the roster registered.
    let registered = match table {
        "plugins.log" => "discord",
        other => other.trim_start_matches("plugins."),
    };
    let settings = &config.plugins[registered].settings;
    match table {
        "plugins.phone" => super::super::moshi_secret(settings),
        "plugins.log" => super::super::discord_settings(settings)
            .token()
            .map(str::to_string),
        "plugins.github" => super::super::parse_github(config)
            .expect("the github table parses")
            .map(|source| source.token),
        "plugins.lights" => crate::hue::hue_settings(settings)
            .expect("the hue table parses")
            .map(|hue| hue.api_key),
        "plugins.home_presence" => super::super::router_api_key(settings),
        other => panic!("no credential reader for `{other}`"),
    }
}

#[test]
fn every_credential_key_reaches_the_setting_its_own_plugin_reads() {
    // ONE CASE PER PLUGIN, through the resolved config rather than a bare
    // table, because the roster is what a key has to get past first: a
    // rename landed in the reader and not in the roster refuses the file.
    for (table, key, _) in CREDENTIALS {
        let text = format!(
            "[{table}]\nenabled = true\n{}{key} = \"a-secret\"\n{}",
            armed(table),
            companion_channels(table)
        );
        let config = parse_config(&text).unwrap_or_else(|error| panic!("`{table}`: {error:?}"));
        assert_eq!(
            credential(&config, table).as_deref(),
            Some("a-secret"),
            "`{table}` key `{key}` did not reach its own reader"
        );
    }
}

#[test]
fn every_retired_credential_spelling_is_refused_naming_the_one_to_write() {
    // THE MUTANT THIS PINS: an old key left in the roster, which loads a
    // file whose credential nothing reads and takes that destination away
    // with no line saying why.
    for (table, key, was) in CREDENTIALS {
        if was == key {
            continue;
        }
        let said = refusal(&format!(
            "[{table}]\nenabled = true\n{}{was} = \"a-secret\"\n",
            armed(table)
        ));
        assert!(said.contains(&format!("`{was}`")), "{said}");
        assert!(said.contains(key), "the key to write is listed: {said}");
    }
}

#[test]
fn the_stale_alert_names_a_route_and_the_channel_spelling_is_refused() {
    let armed = "[plugins.home_presence]\nenabled = true\ntype = \"unifi\"\n";
    let config = parse_config(&format!("{armed}alert_route = \"priority\"\n")).unwrap();
    assert_eq!(
        super::super::stale_alert_route(&config.plugins["home_presence"].settings),
        ("priority".to_string(), None)
    );
    let said = refusal(&format!("{armed}stale_alert_channel = \"priority\"\n"));
    assert!(said.contains("`stale_alert_channel`"), "{said}");
    assert!(said.contains("alert_route"), "{said}");
}

#[test]
fn the_bridge_and_the_router_each_name_their_host_and_the_old_keys_are_refused() {
    // TWO SETTINGS OF ONE KIND, and they now say so: the bridge names a
    // HOST, the router names a URL, and neither stutters its own table's
    // name back at the reader.
    let hue = parse_config(&format!(
        "[plugins.lights]\nenabled = true\ntype = \"hue\"\n{PIN}\
         bridge_host = \"192.168.1.10\"\napi_key = \"a-secret\"\n"
    ))
    .expect("the lights table parses");
    assert_eq!(
        crate::hue::hue_settings(&hue.plugins["lights"].settings)
            .expect("the hue table parses")
            .expect("an armed table")
            .bridge_host,
        "192.168.1.10"
    );
    let router = parse_config(
        "[plugins.home_presence]\nenabled = true\ntype = \"unifi\"\n\
         url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\n",
    )
    .expect("the home presence table parses");
    assert_eq!(
        super::super::router_settings(&router.plugins["home_presence"].settings)
            .expect("the router settings")
            .url,
        "https://192.168.1.1"
    );
    for (table, was, now) in [
        ("plugins.lights", "bridge", "bridge_host"),
        ("plugins.home_presence", "router_url", "url"),
    ] {
        let said = refusal(&format!(
            "[{table}]\nenabled = true\n{}{was} = \"x\"\n",
            armed(table)
        ));
        assert!(said.contains(&format!("`{was}`")), "{said}");
        assert!(said.contains(now), "the key to write is listed: {said}");
    }
}

/// The catch-all an armed discord log cannot load without, written beside
/// the credential under test.
fn companion_channels(table: &str) -> &'static str {
    match table {
        "plugins.log" => "[plugins.log.channels]\ndefault = \"9001\"\npriority = \"9002\"\n",
        _ => "",
    }
}
