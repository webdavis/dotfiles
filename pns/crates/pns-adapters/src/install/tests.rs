use super::*;
use crate::parse_config;
use std::collections::BTreeMap;

/// The home every case resolves against, so a `~/` path has somewhere to go.
const HOME: &str = "/home/tester";

fn settings(config: &str, environment: &[(&str, &str)]) -> InstallSettings {
    let parsed = parse_config(config).expect("the case config parses");
    let environment: BTreeMap<String, String> = environment
        .iter()
        .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
        .collect();
    resolve(Some(&parsed), HOME, &|variable| {
        environment.get(variable).cloned()
    })
}

/// Each setting: what names it in the file, what names it in the environment,
/// and the value each case writes.
const SETTINGS: &[(&str, &str, &str, &str)] = &[
    (
        "[paths]\nstate_dir = \"/from-config/state\"\n",
        "PNS_STATE_DIR",
        "/from-config/state",
        "/from-environment/state",
    ),
    (
        "[paths]\nchannels_dir = \"/from-config/channels\"\n",
        "PNS_CHANNELS_DIR",
        "/from-config/channels",
        "/from-environment/channels",
    ),
    (
        "[plugins.hermes]\nurl = \"http://config.invalid/pns-events\"\n",
        "PNS_HERMES_URL",
        "http://config.invalid/pns-events",
        "http://environment.invalid/pns-events",
    ),
    (
        "[plugins.mobile]\nurl = \"http://config.invalid/push\"\n",
        "PNS_MOSHI_URL",
        "http://config.invalid/push",
        "http://environment.invalid/push",
    ),
    (
        "[plugins.macos-banner]\nterminal_bundle_id = \"com.config.Terminal\"\n",
        "PNS_TERMINAL_BUNDLE_ID",
        "com.config.Terminal",
        "com.environment.Terminal",
    ),
];

/// One reading of the five settings that still have a variable, by the index
/// they occupy in `SETTINGS`.
fn read(settings: &InstallSettings, which: usize) -> Option<&str> {
    match which {
        0 => settings.state_dir.as_deref(),
        1 => settings.channels_dir.as_deref(),
        2 => settings.hermes_url.as_deref(),
        3 => settings.moshi_url.as_deref(),
        4 => settings.terminal_bundle_id.as_deref(),
        _ => unreachable!("SETTINGS has five rows"),
    }
}

#[test]
fn every_setting_the_file_names_is_read_from_the_file() {
    for (which, (config, variable, configured, exported)) in SETTINGS.iter().enumerate() {
        // THE VARIABLE IS EXPORTED IN EVERY CASE, which is what makes this
        // the precedence test rather than a reading test: the file wins
        // while both name the setting.
        let resolved = settings(config, &[(variable, exported)]);
        assert_eq!(read(&resolved, which), Some(*configured), "{variable}");
    }
}

#[test]
fn every_setting_the_file_leaves_out_is_read_from_its_variable() {
    for (which, (_, variable, _, exported)) in SETTINGS.iter().enumerate() {
        let resolved = settings("", &[(variable, exported)]);
        assert_eq!(read(&resolved, which), Some(*exported), "{variable}");
    }
}

#[test]
fn a_setting_neither_the_file_nor_a_variable_names_is_left_to_its_default() {
    let resolved = settings("", &[]);
    for which in 0..SETTINGS.len() {
        assert_eq!(read(&resolved, which), None, "setting {which}");
    }
}

#[test]
fn an_exported_but_blank_variable_shadows_nothing() {
    for (which, (_, variable, _, _)) in SETTINGS.iter().enumerate() {
        assert_eq!(read(&settings("", &[(variable, "")]), which), None);
    }
}

#[test]
fn a_tilde_path_in_the_file_is_read_against_this_home() {
    let resolved = settings("[paths]\nstate_dir = \"~/state/pns\"\n", &[]);
    assert_eq!(
        resolved.state_dir.as_deref(),
        Some("/home/tester/state/pns")
    );
}

#[test]
fn the_remote_deadline_is_the_config_key_and_the_retired_variable_changes_nothing() {
    // `PNS_REMOTE_TIMEOUT` IS GONE. It named the same bound the file now
    // carries, so a machine still exporting it must see exactly the default.
    let exported = settings("", &[("PNS_REMOTE_TIMEOUT", "97")]);
    assert_eq!(
        exported.remote_deadline,
        Some(Duration::from_secs(crate::DEFAULT_REMOTE_DEADLINE_SECS))
    );
    let configured = settings(
        "[delivery]\nremote_deadline = 30\n",
        &[("PNS_REMOTE_TIMEOUT", "97")],
    );
    assert_eq!(configured.remote_deadline, Some(Duration::from_secs(30)));
}

#[test]
fn a_config_nobody_could_load_leaves_every_setting_to_its_variable() {
    let resolved = resolve(None, HOME, &|variable| {
        (variable == "PNS_STATE_DIR").then(|| "/from-environment/state".to_string())
    });
    assert_eq!(
        resolved.state_dir.as_deref(),
        Some("/from-environment/state")
    );
    assert_eq!(resolved.channels_dir, None);
}
