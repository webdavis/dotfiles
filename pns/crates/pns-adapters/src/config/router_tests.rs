use super::*;
use pns_domain::home::DeviceIdentity;
fn table(text: &str) -> toml::Table {
    text.parse().unwrap()
}
fn identity(text: &str) -> DeviceIdentity {
    device_identity(&table(text)).expect("a valid device identity")
}

#[test]
fn the_router_names_its_backend_with_type_and_no_longer_with_brand() {
    // ONE WORD ACROSS THE FILE. `type` is what selects a backend under
    // every table that has one to select, so the router's old `brand` is
    // not a second spelling of it: it names no backend, and a table
    // carrying it is refused exactly as an empty one is.
    let typed = table(
        "type = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\n",
    );
    assert!(router_settings(&typed).is_ok(), "`type` names the backend");
    let branded =
        table("brand = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n");
    assert!(
        router_settings(&branded).is_err(),
        "`brand` no longer names one"
    );
}

#[test]
fn an_enabled_unifi_router_table_yields_its_url_and_device() {
    // The whole value path in one: the config's `[plugins.router]` table,
    // through the selection gate, into the two settings the probe runs on.
    let config = crate::config::parse_config(
        "[plugins.router]\nenabled = true\ntype = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
    )
    .unwrap();
    let router = enabled_router_table(&config).expect("the enabled table");
    assert_eq!(
        router_settings(router),
        Ok(RouterSettings {
            router_url: "https://192.168.1.1".to_string(),
            device: identity("device_hostname = \"mister\"\n"),
        })
    );
}

#[test]
fn a_well_formed_mac_in_any_case_or_separator_validates_to_one_spelling() {
    // ONE spelling is what makes the two sides comparable at all: the
    // operator may copy the MAC off a sticker in uppercase with dashes,
    // and the UDR answers in lowercase with colons.
    for typed in [
        "2e:11:ab:6d:b0:4f",
        "2E:11:AB:6D:B0:4F",
        "2e-11-ab-6d-b0-4f",
        "2E-11-AB-6D-B0-4F",
    ] {
        assert_eq!(
            device_identity(&table(&format!("device_mac = \"{typed}\"\n"))),
            Ok(DeviceIdentity::new(None, None, Some("2e:11:ab:6d:b0:4f".to_string())).unwrap()),
            "case: {typed:?}"
        );
    }
}

// --- the secret ----------------------------------------------------------

#[test]
fn the_api_key_reads_from_the_router_plugin_table_beside_the_settings() {
    // Through the config, so this pins WHERE the key is read from and not
    // only how: a table lifted from anywhere else would pass a bare-table
    // assertion just as well.
    let config = crate::config::parse_config(
        "[plugins.router]\nenabled = true\ntype = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
    )
    .unwrap();
    let router = enabled_router_table(&config).expect("the enabled table");
    assert_eq!(router_api_key(router), Some("k-123".to_string()));
}

// --- where the stale alert is routed -------------------------------------

#[test]
fn a_usable_stale_alert_channel_is_read_back_as_the_route_verbatim() {
    // The operator names a hermes ROUTE, and the name they typed is what
    // the alert carries: nothing here rewrites, lowercases or defaults it.
    assert_eq!(
        stale_alert_channel(&table("stale_alert_channel = \"priority\"\n")),
        ("priority".to_string(), None)
    );
}

#[test]
fn no_stale_alert_channel_at_all_asks_for_the_default_route_in_silence() {
    // ABSENT IS NOT AN ERROR: the key is optional, and an empty route is
    // how every caller of `hermes_url_for` spells the default route
    // (`/webhooks/pns`). Complaining here would put a config error in
    // front of every operator who never asked to route the alert anywhere.
    assert_eq!(
        stale_alert_channel(&table("type = \"unifi\"\n")),
        (String::new(), None)
    );
}

#[test]
fn a_stale_alert_channel_that_is_not_a_usable_route_complains_and_falls_back() {
    // LOUD-WARD, the same direction `hermes_url_for` falls: a misrouted
    // alert on the default route beats one silently dropped, and the
    // complaint names the CONFIG KEY, because the config is where the
    // operator has to go. The three ways the value can fail are one
    // message, because they are one fix.
    for (setting, quoted) in [
        ("stale_alert_channel = \"\"\n", "\"\""),
        ("stale_alert_channel = \"a/b\"\n", "\"a/b\""),
        ("stale_alert_channel = \"../alert\"\n", "\"../alert\""),
        ("stale_alert_channel = 5\n", "<integer>"),
        ("stale_alert_channel = true\n", "<boolean>"),
    ] {
        let (route, complaint) = stale_alert_channel(&table(setting));
        assert_eq!(route, String::new(), "case: {setting:?}");
        assert_eq!(
            complaint.as_deref(),
            Some(
                format!(
                    "pns: config error (stale_alert_channel = {quoted} in [plugins.router] \
                     is not a usable route name); the stale alert posts to the default route"
                )
                .as_str()
            ),
            "case: {setting:?}"
        );
    }
}
