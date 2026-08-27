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
    let listing = serde_json::from_str::<serde_json::Value>(clients_json).ok()?;
    let clients = listing.get("data")?.as_array()?;
    // An INCOMPLETE PAGE is no answer: totalCount counts every client the
    // router knows, and a phone beyond this page would read as departed,
    // which is a false transition. Completeness is judged on the entries
    // rather than the names, because the filter below legitimately skips
    // clients the router has not identified.
    if let Some(total) = listing
        .get("totalCount")
        .and_then(serde_json::Value::as_u64)
        && total > clients.len() as u64
    {
        return None;
    }
    Some(
        clients
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
    /// The agent every call rides, INJECTED so a test can hand in one wearing
    /// a scripted transport (`Agent::with_parts`): the production pipeline
    /// runs for real and only the wire is fake, which is the closest Rust
    /// analog to stubbing Swift's URL Loading System.
    agent: ureq::Agent,
    /// e.g. `https://192.168.1.1`, from the `[home]` table.
    base: String,
    /// The API key, from the auth file.
    key: String,
}

/// One bounded fetch: a probe on a diagnostic path is worth seconds, never a
/// hang.
const ROUTER_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

/// The router's answers are small (a 200-client listing measures kilobytes),
/// so the read is capped far below ureq's own 10MB default: a faulty router
/// streaming garbage costs at most this much memory before reading Unknown.
const ROUTER_BODY_CAP: u64 = 1_000_000;

impl UreqRouter {
    /// The production wiring: TLS with verification disabled exactly as the
    /// hue bridge's is (the router serves a self-signed certificate for its
    /// own LAN address, and no CA vouches for it), no redirects, and the
    /// deadline on every call.
    pub fn new(base: String, key: String) -> Self {
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
        Self::with_agent(agent, base, key)
    }

    /// The same router over any agent: the seam the scripted-transport tests
    /// inject through.
    pub fn with_agent(agent: ureq::Agent, base: String, key: String) -> Self {
        Self { agent, base, key }
    }
}

/// The integration API's clients listing for the default site.
///
/// The site is resolved by name (`default`) through the sites listing first,
/// because site ids are per-install; both calls ride one agent and one
/// deadline each.
impl Router for UreqRouter {
    fn clients_json(&self) -> Option<String> {
        let get = |path: &str| {
            self.agent
                .get(format!("{}{path}", self.base))
                .header("X-API-KEY", &self.key)
                .call()
                .ok()?
                .body_mut()
                .with_config()
                .limit(ROUTER_BODY_CAP)
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

/// The one line `pns home` says for a verdict. PURE, so the words and the
/// verdict cannot drift apart untested: a swap of the two sentences survived
/// every suite before this function existed.
pub fn report(presence: HomePresence, phone: &str) -> String {
    match presence {
        HomePresence::Home => format!("home: phone \"{phone}\" is on the home network"),
        HomePresence::NotHome => format!("home: phone \"{phone}\" is NOT on the home network"),
        HomePresence::Unknown => {
            "home: unknown (router unreachable or its answer unreadable)".to_string()
        }
    }
}

/// Every way the probe can be not set up, as data: the diagnostic's whole
/// vocabulary in one place, each state with its own line, because one
/// "not configured" covering both a missing table and a mistyped value sent
/// the operator to write a table they already had.
#[derive(Debug, PartialEq)]
pub enum SetupFailure {
    NoConfigFile,
    ConfigError(String),
    NoHomeTable,
    InvalidHomeTable,
    NoAuthKey(String),
    AuthFileIrregular(String),
}

/// The one line for a setup failure. PURE for the same reason as `report`.
pub fn setup_report(failure: &SetupFailure) -> String {
    match failure {
        SetupFailure::NoConfigFile => "home: not configured (no config file)".to_string(),
        SetupFailure::ConfigError(detail) => format!("home: config error ({detail})"),
        SetupFailure::NoHomeTable => {
            "home: not configured (no [home] table with router_url and phone)".to_string()
        }
        SetupFailure::InvalidHomeTable => {
            "home: the [home] table is present but router_url or phone is missing, empty, \
             or not a string"
                .to_string()
        }
        SetupFailure::NoAuthKey(path) => {
            format!("home: no unifi_api_key in {path} (the probe is not set up)")
        }
        SetupFailure::AuthFileIrregular(path) => {
            format!("home: {path} is not a regular file, so it was not read")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HomePresence, HomeSettings, Router, SetupFailure, first_site_id, home_settings,
        parse_client_names, phone_presence, read_home_presence, report, setup_report, unifi_secret,
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

    // --- pagination: an incomplete page must not report a departure ----------

    #[test]
    fn a_page_the_phone_could_be_beyond_is_no_answer_rather_than_not_home() {
        // totalCount says 201 clients exist and the page carries one: the
        // phone may be among the 200 not shown, so absent-from-this-page must
        // not become NotHome, which is a false departure.
        assert_eq!(
            parse_client_names(
                r#"{"offset":0,"limit":200,"count":1,"totalCount":201,"data":[{"name":"dresden"}]}"#
            ),
            None
        );
    }

    #[test]
    fn a_complete_page_with_unnamed_clients_still_answers() {
        // Completeness is judged on the ENTRIES, not the names: an unnamed
        // client is on the page even though the name filter skips it.
        assert_eq!(
            parse_client_names(r#"{"totalCount":2,"data":[{"id":"x"},{"name":"mister"}]}"#),
            Some(vec!["mister".to_string()])
        );
    }

    #[test]
    fn a_listing_without_total_count_is_taken_whole_as_before() {
        assert_eq!(
            parse_client_names(r#"{"data":[{"name":"mister"}]}"#),
            Some(vec!["mister".to_string()])
        );
    }

    // --- the site id, the one router answer that becomes part of a URL -------

    #[test]
    fn the_first_sites_id_is_extracted_from_the_live_shape() {
        assert_eq!(
            first_site_id(
                r#"{"offset":0,"limit":25,"count":1,"totalCount":1,"data":[{"id":"88f7af54-98f8-306a-a1c7-c9349722b1f6","internalReference":"default","name":"Default"}]}"#
            ),
            Some("88f7af54-98f8-306a-a1c7-c9349722b1f6".to_string())
        );
    }

    #[test]
    fn a_sites_answer_without_a_usable_id_is_no_answer() {
        for sites in [
            "",
            "not json",
            r#"{"data":[]}"#,
            r#"{"data":[{}]}"#,
            r#"{"data":[{"id":5}]}"#,
        ] {
            assert_eq!(first_site_id(sites), None, "case: {sites:?}");
        }
    }

    #[test]
    fn an_id_that_could_escape_the_url_path_is_refused_outright() {
        // The id is joined into /sites/{id}/clients, so this is the trust
        // boundary: a corrupt or hostile router answer must never become a
        // path segment.
        for hostile in ["../evil", "a/b", "a?x=1", "a#frag", "id with space", ""] {
            assert_eq!(
                first_site_id(&format!(r#"{{"data":[{{"id":"{hostile}"}}]}}"#)),
                None,
                "case: {hostile:?}"
            );
        }
    }

    // --- the reported lines, pinned so the words match the verdict -----------

    #[test]
    fn each_presence_verdict_reports_its_own_line() {
        assert_eq!(
            report(HomePresence::Home, "mister"),
            "home: phone \"mister\" is on the home network"
        );
        assert_eq!(
            report(HomePresence::NotHome, "mister"),
            "home: phone \"mister\" is NOT on the home network"
        );
        assert_eq!(
            report(HomePresence::Unknown, "mister"),
            "home: unknown (router unreachable or its answer unreadable)"
        );
    }

    #[test]
    fn a_missing_home_table_and_an_invalid_one_are_told_apart() {
        // `phone = 5` used to be reported as "no [home] table", sending the
        // operator to write a table that already exists instead of fixing
        // one value.
        let missing = setup_report(&SetupFailure::NoHomeTable);
        let invalid = setup_report(&SetupFailure::InvalidHomeTable);
        assert_ne!(missing, invalid);
        assert!(missing.contains("no [home] table"), "got: {missing}");
        assert!(
            invalid.contains("[home] table is present"),
            "got: {invalid}"
        );
    }

    #[test]
    fn every_setup_failure_line_names_what_to_look_at() {
        for (failure, needle) in [
            (SetupFailure::NoConfigFile, "no config file"),
            (
                SetupFailure::ConfigError("bad at line 3".to_string()),
                "bad at line 3",
            ),
            (SetupFailure::NoHomeTable, "router_url"),
            (SetupFailure::InvalidHomeTable, "router_url"),
            (
                SetupFailure::NoAuthKey("/x/auth.json".to_string()),
                "/x/auth.json",
            ),
            (
                SetupFailure::AuthFileIrregular("/x/auth.json".to_string()),
                "regular file",
            ),
        ] {
            let line = setup_report(&failure);
            assert!(line.starts_with("home: "), "case {failure:?}: {line}");
            assert!(line.contains(needle), "case {failure:?}: {line}");
        }
    }

    // --- the production adapter, over a scripted transport -------------------
    //
    // The URLProtocolStub move, in Rust: ureq's `Agent::with_parts` accepts a
    // bespoke `Connector`, so the REAL agent pipeline runs (URL building, the
    // header, redirect policy, the body cap) and only the wire is scripted.
    // The seam lives in `ureq::unversioned`, which is exempt from semver;
    // Cargo.lock pins the version, so it can only shift the day ureq is
    // deliberately bumped, with these tests here to catch it.

    use std::sync::{Arc, Mutex};
    use ureq::unversioned::resolver::DefaultResolver;
    use ureq::unversioned::transport::{
        Buffers, ConnectionDetails, Connector, LazyBuffers, NextTimeout, Transport,
    };

    /// Hands out one scripted response per connection and keeps a shared
    /// capture of every byte the adapter transmits.
    #[derive(Debug, Default)]
    struct ScriptedConnector {
        wire: Arc<Mutex<Vec<u8>>>,
        responses: Arc<Mutex<std::collections::VecDeque<Vec<u8>>>>,
    }

    impl Connector for ScriptedConnector {
        type Out = ScriptedTransport;

        fn connect(
            &self,
            _details: &ConnectionDetails,
            _chained: Option<()>,
        ) -> Result<Option<Self::Out>, ureq::Error> {
            let response = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_default();
            Ok(Some(ScriptedTransport {
                buffers: LazyBuffers::new(65536, 65536),
                wire: Arc::clone(&self.wire),
                response,
                fed: 0,
            }))
        }
    }

    /// One connection: records what is transmitted, feeds the scripted
    /// response in chunks, and refuses reuse so every request reconnects and
    /// pops the next script entry.
    #[derive(Debug)]
    struct ScriptedTransport {
        buffers: LazyBuffers,
        wire: Arc<Mutex<Vec<u8>>>,
        response: Vec<u8>,
        fed: usize,
    }

    impl Transport for ScriptedTransport {
        fn buffers(&mut self) -> &mut dyn Buffers {
            &mut self.buffers
        }

        fn transmit_output(
            &mut self,
            amount: usize,
            _timeout: NextTimeout,
        ) -> Result<(), ureq::Error> {
            let sent = self.buffers.output()[..amount].to_vec();
            self.wire.lock().unwrap().extend_from_slice(&sent);
            Ok(())
        }

        fn await_input(&mut self, _timeout: NextTimeout) -> Result<bool, ureq::Error> {
            let pending = &self.response[self.fed..];
            if pending.is_empty() {
                return Ok(false);
            }
            let sink = self.buffers.input_append_buf();
            let amount = pending.len().min(sink.len());
            sink[..amount].copy_from_slice(&pending[..amount]);
            self.buffers.input_appended(amount);
            self.fed += amount;
            Ok(amount > 0)
        }

        fn is_open(&mut self) -> bool {
            // A finished response closes the connection, so the agent cannot
            // pool it: the next request reconnects and pops the next script.
            self.fed < self.response.len()
        }
    }

    /// The production config semantics (no redirects) over the scripted wire.
    fn scripted_router(
        responses: &[Vec<u8>],
        key: &str,
    ) -> (super::UreqRouter, Arc<Mutex<Vec<u8>>>) {
        let connector = ScriptedConnector::default();
        let wire = Arc::clone(&connector.wire);
        *connector.responses.lock().unwrap() = responses.iter().cloned().collect();
        let config = ureq::Agent::config_builder().max_redirects(0).build();
        let agent = ureq::Agent::with_parts(config, connector, DefaultResolver::default());
        (
            super::UreqRouter::with_agent(agent, "http://localhost:9".to_string(), key.to_string()),
            wire,
        )
    }

    fn http_ok(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    const SITES_CAPTURE: &str = r#"{"offset":0,"limit":25,"count":1,"totalCount":1,"data":[{"id":"88f7af54-98f8-306a-a1c7-c9349722b1f6","internalReference":"default","name":"Default"}]}"#;

    #[test]
    fn the_adapter_sends_the_key_and_walks_sites_then_clients() {
        let (router, wire) =
            scripted_router(&[http_ok(SITES_CAPTURE), http_ok(CLIENTS_CAPTURE)], "k-123");
        assert_eq!(read_home_presence(&router, "mister"), HomePresence::Home);
        let wire = String::from_utf8_lossy(&wire.lock().unwrap()).to_lowercase();
        let sites_at = wire
            .find("get /proxy/network/integration/v1/sites http/1.1")
            .expect("the sites request was made");
        let clients_at = wire
            .find(
                "get /proxy/network/integration/v1/sites/88f7af54-98f8-306a-a1c7-c9349722b1f6/clients?limit=200 http/1.1",
            )
            .expect("the clients request was made with the extracted site id");
        assert!(sites_at < clients_at, "sites is asked before clients");
        assert!(
            wire.contains("x-api-key: k-123"),
            "the key rides the header: {wire}"
        );
    }

    #[test]
    fn a_redirecting_router_is_never_followed_and_reads_unknown() {
        // max_redirects(0) is production config: a router answering 301 must
        // not send the adapter (and its key header) to the Location target.
        let (router, wire) = scripted_router(
            &[
                b"HTTP/1.1 301 Moved Permanently\r\nlocation: http://elsewhere.test/\r\ncontent-length: 0\r\n\r\n".to_vec(),
            ],
            "k-123",
        );
        assert_eq!(read_home_presence(&router, "mister"), HomePresence::Unknown);
        let wire = String::from_utf8_lossy(&wire.lock().unwrap()).to_lowercase();
        assert_eq!(wire.matches("get ").count(), 1, "no second request: {wire}");
        assert!(
            !wire.contains("elsewhere"),
            "the redirect target is never contacted"
        );
    }

    #[test]
    fn a_body_past_the_cap_reads_unknown_rather_than_being_swallowed() {
        // The oversized answer is VALID and would parse into Home if it were
        // read: that is what makes this pin the 1MB cap itself rather than
        // ureq's own 10MB default, which an unparseable body cannot tell
        // apart (a garbage body reads Unknown under any cap).
        let oversized_but_valid_sites = format!(
            r#"{{"padding":"{}","data":[{{"id":"88f7af54-98f8-306a-a1c7-c9349722b1f6"}}]}}"#,
            "x".repeat(1_100_000)
        );
        let (router, _wire) = scripted_router(
            &[
                http_ok(&oversized_but_valid_sites),
                http_ok(CLIENTS_CAPTURE),
            ],
            "k-123",
        );
        assert_eq!(read_home_presence(&router, "mister"), HomePresence::Unknown);
    }
}
