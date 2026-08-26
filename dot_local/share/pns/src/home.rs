//! The home probe: is the operator's phone on the home network?
//!
//! THE ROUTER IS THE WITNESS. The UDR (UniFi Dream Router) keeps the list of
//! clients currently on the wifi, and the phone appearing in that list is
//! what "home" means here. The reading is a sensor only: nothing in the
//! delivery plan consumes it yet, because no row of the confirmed matrix
//! changes on home-ness until catch-up-on-return and the quiet window (part
//! 2's B and C) arrive to spend it. Building the integration ahead of the
//! consumer was considered and declined on 2026-08-25.
//!
//! THE PHONE IS MATCHED BY NAME, NEVER BY MAC. iOS ships private wifi
//! addresses: the MAC the router sees is minted per network and can rotate,
//! while the client name survives (verified against the live capture of
//! 2026-08-20, where the phone's MAC is locally administered). The name is an
//! exact match, because a substring match would let "mister-2" answer for
//! "mister".
//!
//! Fail direction: every failure to read is `Unknown`, never `NotHome`. The
//! future consumers suppress or replay on transitions, so inventing "the
//! phone left" out of an unreachable router would fire a false transition;
//! Unknown is the reading that changes nothing.

/// What the router said about the phone. `NotHome` requires a PARSED client
/// list that does not carry the name; anything less is `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomePresence {
    Home,
    NotHome,
    Unknown,
}

/// The `[home]` table's settings, validated. `None` is "not set up", which is
/// a state and not an error, matching the config module's own philosophy.
#[derive(Debug, PartialEq)]
pub struct HomeSettings {
    /// Where the router answers, e.g. `https://192.168.1.1`.
    pub router_url: String,
    /// The client name the phone appears under in the router's list.
    pub phone: String,
}

/// The seam one probe reads the router through. The production impl carries
/// the deadline and the self-signed-TLS stance; a fake answers from a string.
pub trait Router {
    /// The clients listing as the router returned it, or `None` when it could
    /// not be fetched.
    fn clients_json(&self) -> Option<String>;
}

/// The client names in a UniFi `/clients` listing, or `None` when the text is
/// not one. `None` and an empty list are DIFFERENT readings: an empty list is
/// a parsed answer ("nobody is on the wifi"), while `None` is no answer.
pub fn parse_client_names(clients_json: &str) -> Option<Vec<String>> {
    Some(
        serde_json::from_str::<serde_json::Value>(clients_json)
            .ok()?
            .get("data")?
            .as_array()?
            .iter()
            // A client the router has not identified carries no name; the
            // listing is still an answer about everyone it did name.
            .filter_map(|client| client.get("name")?.as_str().map(str::to_string))
            .collect(),
    )
}

/// The verdict for one phone against one parsed listing.
pub fn phone_presence(names: Option<Vec<String>>, phone: &str) -> HomePresence {
    // An empty configured name is a configuration hole, not a device:
    // answering NotHome for it would report a departure nobody's phone made.
    if phone.is_empty() {
        return HomePresence::Unknown;
    }
    match names {
        None => HomePresence::Unknown,
        Some(names) if names.iter().any(|name| name == phone) => HomePresence::Home,
        Some(_) => HomePresence::NotHome,
    }
}

/// The `[home]` settings out of the config's table, or `None` when the table
/// is absent or does not carry both values. A present-but-wrong-typed value
/// is reported on stderr by the caller's layer; this stays pure.
pub fn home_settings(home: Option<&toml::Table>) -> Option<HomeSettings> {
    let home = home?;
    let value = |key: &str| {
        home.get(key)
            .and_then(toml::Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    Some(HomeSettings {
        router_url: value("router_url")?,
        phone: value("phone")?,
    })
}

/// The UniFi API key out of the relay auth file, or `None` for every way the
/// file can fail to provide one. Mirrors `moshi_secret`: the key's path from
/// file to request header must never touch argv, the environment, or an
/// error string.
pub fn unifi_secret(auth_json: &str) -> Option<String> {
    let key = serde_json::from_str::<serde_json::Value>(auth_json)
        .ok()?
        .get("unifi_api_key")?
        .as_str()?
        .to_string();
    (!key.is_empty()).then_some(key)
}

/// One reading: fetch through the seam, parse, judge.
pub fn read_home_presence<R: Router>(router: &R, phone: &str) -> HomePresence {
    phone_presence(
        router
            .clients_json()
            .as_deref()
            .and_then(parse_client_names),
        phone,
    )
}

/// The production router client, over the same HTTP stack as the other
/// native legs.
///
/// THE SECRET'S PATH IS THE POINT, exactly as in the moshi channel: the key
/// travels from the auth file into the request HEADER and nowhere else,
/// never argv, never a child's environment, never an error string. TLS
/// verification is disabled the way the hue bridge's is, and for the same
/// reason: the router serves a self-signed certificate for its own LAN
/// address, and no CA vouches for it.
pub struct UreqRouter {
    /// e.g. `https://192.168.1.1`, from the `[home]` table.
    pub base: String,
    /// The API key, from the auth file.
    pub key: String,
}

/// One bounded fetch: a probe on a diagnostic path is worth seconds, never a
/// hang.
const ROUTER_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

/// The integration API's clients listing for the default site.
///
/// The site is resolved by name (`default`) through the sites listing first,
/// because site ids are per-install; both calls ride one agent and one
/// deadline each.
impl Router for UreqRouter {
    fn clients_json(&self) -> Option<String> {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(ROUTER_DEADLINE))
            .max_redirects(0)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .disable_verification(true)
                    .build(),
            )
            .build()
            .new_agent();
        let get = |path: &str| {
            agent
                .get(format!("{}{path}", self.base))
                .header("X-API-KEY", &self.key)
                .call()
                .ok()?
                .body_mut()
                .read_to_string()
                .ok()
        };
        let sites = get("/proxy/network/integration/v1/sites")?;
        let site = first_site_id(&sites)?;
        get(&format!(
            "/proxy/network/integration/v1/sites/{site}/clients?limit=200"
        ))
    }
}

/// The first site's id out of the sites listing, which on a UDR is the one
/// `default` site. Validated as id-shaped before it is joined into a path:
/// this is the one place a router answer becomes part of a URL.
pub fn first_site_id(sites_json: &str) -> Option<String> {
    let id = serde_json::from_str::<serde_json::Value>(sites_json)
        .ok()?
        .get("data")?
        .as_array()?
        .first()?
        .get("id")?
        .as_str()?
        .to_string();
    (!id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-'))
    .then_some(id)
}

#[cfg(test)]
mod tests {
    use super::{
        HomePresence, HomeSettings, Router, home_settings, parse_client_names, phone_presence,
        read_home_presence, unifi_secret,
    };

    /// The live capture of 2026-08-20 from the UDR's
    /// `/proxy/network/integration/v1/sites/{id}/clients`, verbatim: four
    /// clients, the phone ("mister") among them with an iOS private MAC.
    const CLIENTS_CAPTURE: &str = r#"{"offset":0,"limit":200,"count":4,"totalCount":4,"data":[{"type":"WIRED","id":"b6363aa8-67c6-326b-93da-90f8e9632d95","name":"hue-bridge-pro","connectedAt":"2026-08-14T03:01:20Z","ipAddress":"192.168.4.37","macAddress":"c4:29:96:bb:6d:cc","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"f7585d92-906f-3c6c-84c3-89e2ce6b2eda","name":"dresden","connectedAt":"2026-08-19T05:44:57Z","ipAddress":"192.168.1.26","macAddress":"3c:06:30:0f:8a:bf","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"71e95d68-f736-3554-b547-75fa1cdc7bf4","name":"mister","connectedAt":"2026-08-20T04:14:55Z","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"84038944-790a-3965-9f7e-cfee3468308c","name":"mouse","connectedAt":"2026-08-20T05:18:12Z","ipAddress":"192.168.1.248","macAddress":"60:82:46:3c:fb:01","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}}]}"#;

    // --- parsing the router's listing ---------------------------------------

    #[test]
    fn every_client_name_in_the_live_capture_is_read() {
        assert_eq!(
            parse_client_names(CLIENTS_CAPTURE),
            Some(vec![
                "hue-bridge-pro".to_string(),
                "dresden".to_string(),
                "mister".to_string(),
                "mouse".to_string(),
            ])
        );
    }

    #[test]
    fn a_listing_that_is_not_json_is_no_answer_rather_than_an_empty_wifi() {
        // Unparseable and empty must stay distinct: empty means "nobody is
        // on the wifi" and would read as the phone having LEFT.
        assert_eq!(parse_client_names("<html>router login</html>"), None);
        assert_eq!(parse_client_names(""), None);
    }

    #[test]
    fn a_json_answer_without_the_data_list_is_no_answer() {
        // The auth-failed shape: valid JSON, no clients in it.
        assert_eq!(parse_client_names(r#"{"error":"unauthorized"}"#), None);
        assert_eq!(parse_client_names(r#"{"data":"not-a-list"}"#), None);
    }

    #[test]
    fn a_parsed_empty_list_is_an_answer_and_not_a_failure() {
        assert_eq!(
            parse_client_names(r#"{"offset":0,"limit":200,"count":0,"totalCount":0,"data":[]}"#),
            Some(Vec::new())
        );
    }

    #[test]
    fn a_client_without_a_name_is_skipped_rather_than_sinking_the_listing() {
        // UniFi omits `name` for clients it has not identified; the listing
        // is still an answer about everyone it did name.
        assert_eq!(
            parse_client_names(r#"{"data":[{"id":"x"},{"name":"mister"}]}"#),
            Some(vec!["mister".to_string()])
        );
    }

    // --- the verdict ---------------------------------------------------------

    #[test]
    fn the_phone_in_the_list_is_home_and_absent_is_not_home() {
        let names = || Some(vec!["dresden".to_string(), "mister".to_string()]);
        assert_eq!(phone_presence(names(), "mister"), HomePresence::Home);
        assert_eq!(
            phone_presence(Some(vec!["dresden".to_string()]), "mister"),
            HomePresence::NotHome
        );
    }

    #[test]
    fn no_answer_from_the_router_is_unknown_never_not_home() {
        // The future consumers act on TRANSITIONS. An unreachable router
        // inventing "the phone left" would fire a false one.
        assert_eq!(phone_presence(None, "mister"), HomePresence::Unknown);
    }

    #[test]
    fn the_name_match_is_exact_so_a_sibling_device_cannot_answer_for_the_phone() {
        let names = Some(vec!["mister-2".to_string(), "MISTER".to_string()]);
        assert_eq!(phone_presence(names, "mister"), HomePresence::NotHome);
    }

    #[test]
    fn an_empty_configured_phone_name_is_unknown_rather_than_matching_nothing() {
        // "" is a configuration hole, not a device: answering NotHome for it
        // would report a departure nobody's phone made.
        assert_eq!(
            phone_presence(Some(vec!["dresden".to_string()]), ""),
            HomePresence::Unknown
        );
    }

    // --- the seam, end to end ------------------------------------------------

    struct FakeRouter(Option<&'static str>);
    impl Router for FakeRouter {
        fn clients_json(&self) -> Option<String> {
            self.0.map(str::to_string)
        }
    }

    #[test]
    fn one_reading_runs_fetch_parse_judge_in_order() {
        assert_eq!(
            read_home_presence(&FakeRouter(Some(CLIENTS_CAPTURE)), "mister"),
            HomePresence::Home
        );
        assert_eq!(
            read_home_presence(&FakeRouter(None), "mister"),
            HomePresence::Unknown
        );
    }

    // --- the settings --------------------------------------------------------

    fn table(text: &str) -> toml::Table {
        text.parse().unwrap()
    }

    #[test]
    fn a_complete_home_table_yields_its_settings() {
        assert_eq!(
            home_settings(Some(&table(
                "router_url = \"https://192.168.1.1\"\nphone = \"mister\"\n"
            ))),
            Some(HomeSettings {
                router_url: "https://192.168.1.1".to_string(),
                phone: "mister".to_string(),
            })
        );
    }

    #[test]
    fn an_absent_or_incomplete_table_is_not_set_up() {
        assert_eq!(home_settings(None), None);
        assert_eq!(
            home_settings(Some(&table("router_url = \"https://192.168.1.1\"\n"))),
            None
        );
        assert_eq!(home_settings(Some(&table("phone = \"mister\"\n"))), None);
        // Present but empty is a hole, not a value.
        assert_eq!(
            home_settings(Some(&table("router_url = \"\"\nphone = \"mister\"\n"))),
            None
        );
        // Present but the wrong type is refused, not coerced.
        assert_eq!(
            home_settings(Some(&table("router_url = 5\nphone = \"mister\"\n"))),
            None
        );
    }

    // --- the secret ----------------------------------------------------------

    #[test]
    fn the_unifi_key_reads_from_the_auth_file_beside_the_other_secrets() {
        assert_eq!(
            unifi_secret(r#"{"moshi_secret":"m","unifi_api_key":"k-123"}"#),
            Some("k-123".to_string())
        );
    }

    #[test]
    fn every_way_the_auth_file_fails_to_provide_a_key_is_quietly_not_set_up() {
        for auth in [
            "",
            "not json",
            "{}",
            r#"{"unifi_api_key":""}"#,
            r#"{"unifi_api_key":5}"#,
        ] {
            assert_eq!(unifi_secret(auth), None, "case: {auth:?}");
        }
    }
}
