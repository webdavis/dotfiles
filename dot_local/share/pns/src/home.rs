//! The home probe: is the operator's device on the home network?
//!
//! THE ROUTER IS THE WITNESS. The UDR (UniFi Dream Router) keeps the list of
//! clients currently on the wifi, and the device appearing in that list is
//! what "home" means here. The reading is a sensor only: nothing in the
//! delivery plan consumes it yet, because no row of the confirmed matrix
//! changes on home-ness until catch-up-on-return and the quiet window (part
//! 2's B and C) arrive to spend it. Building the integration ahead of the
//! consumer was considered and declined on 2026-08-25.
//!
//! THREE KEYS NAME THE DEVICE, and at least one is required. Any one of them
//! matching any client reads Home; the key the verdict NAMES is the strongest
//! that matched, scanning MAC, then hostname, then address. That order is the
//! disagreement rule: a MAC is the device itself, a client name is a label the
//! operator can move, and an address is only today's lease. A phone still
//! wants the NAME key, because iOS ships private wifi addresses and the MAC
//! the router sees is minted per network and can rotate (verified against the
//! live capture of 2026-08-20, where the phone's MAC is locally
//! administered); the MAC key is for devices whose MAC stays put. The name
//! match is exact and case-sensitive, because anything looser would let
//! "mister-2" answer for "mister".
//!
//! A KEY CAN GO STALE, and this slice does not detect it: a rotated or
//! reassigned MAC can name the wrong client while the verdict is still Home
//! off the hostname, and `device_ipv4` drifts under DHCP by design. Detecting
//! the disagreement is the staleness slice's job, not this one's.
//!
//! Fail direction: every failure to read is `Unknown`, never `NotHome`. The
//! future consumers suppress or replay on transitions, so inventing "the
//! device left" out of an unreachable router would fire a false transition;
//! Unknown is the reading that changes nothing.

/// One way a device can be recognized in the router's client list, DECLARED
/// STRONGEST FIRST: a MAC is the device itself, a client name is what the
/// operator called it, and an address is whatever DHCP handed out today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKey {
    Mac,
    Hostname,
    Ipv4,
}

impl DeviceKey {
    /// The key's spelling in `[plugins.router]`, which is also how every line
    /// names it: the operator reads back the word they typed.
    pub fn config_key(self) -> &'static str {
        match self {
            DeviceKey::Mac => "device_mac",
            DeviceKey::Hostname => "device_hostname",
            DeviceKey::Ipv4 => "device_ipv4",
        }
    }
}

/// The configured device, as identifiers that have already been VALIDATED:
/// an address is an `Ipv4Addr` and never text, a MAC is the one normalized
/// spelling, and at least one of the three is set. `device_identity` is the
/// only way to build one, so an unparsed value cannot reach a comparison.
#[derive(Debug, PartialEq)]
pub struct DeviceIdentity {
    hostname: Option<String>,
    ipv4: Option<std::net::Ipv4Addr>,
    mac: Option<String>,
}

/// What the router said about the device. `NotHome` requires a PARSED client
/// list that no configured key matched; anything less is `Unknown`.
///
/// HOME CARRIES ITS OWN EVIDENCE. The key that answered and the value it
/// answered with live inside the `Home` variant rather than beside the
/// verdict, so there is no matched-key field for a `NotHome` to fill in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomePresence {
    Home {
        matched_by: DeviceKey,
        value: String,
    },
    NotHome,
    Unknown,
}

/// The router sensor's settings, validated.
#[derive(Debug, PartialEq)]
pub struct RouterSettings {
    /// Where the router answers, e.g. `https://192.168.1.1`.
    pub router_url: String,
    /// The device to look for in the router's client list.
    pub device: DeviceIdentity,
}

/// The seam one probe reads the router through. The production impl carries
/// the deadline and the self-signed-TLS stance; a fake answers from a string.
pub trait Router {
    /// The clients listing as the router returned it, or `None` when it could
    /// not be fetched.
    fn clients_json(&self) -> Option<String>;
}

/// One client the router listed, in the router's own spelling. Every field is
/// optional because every one of them is: the UDR omits `name` for a client
/// it has not identified, and a client can be listed before it has an
/// address. Nothing here is validated, because the router is not the operator:
/// an entry this probe cannot read is a client that matches nothing, never a
/// listing that failed.
#[derive(Debug, PartialEq)]
pub struct Client {
    name: Option<String>,
    ipv4: Option<String>,
    mac: Option<String>,
}

/// The clients in a UniFi `/clients` listing, or `None` when the text is not
/// one. `None` and an empty list are DIFFERENT readings: an empty list is a
/// parsed answer ("nobody is on the wifi"), while `None` is no answer.
pub fn parse_clients(clients_json: &str) -> Option<Vec<Client>> {
    let listing = serde_json::from_str::<serde_json::Value>(clients_json).ok()?;
    let clients = listing.get("data")?.as_array()?;
    // An INCOMPLETE PAGE is no answer: totalCount counts every client the
    // router knows, and a device beyond this page would read as departed,
    // which is a false transition. Completeness is judged on the ENTRIES,
    // which is what it has always been judged on.
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
            .map(|client| {
                let field =
                    |key: &str| -> Option<String> { client.get(key)?.as_str().map(str::to_string) };
                Client {
                    name: field("name"),
                    ipv4: field("ipAddress"),
                    mac: field("macAddress"),
                }
            })
            .collect(),
    )
}

/// The verdict for one configured device against one parsed listing.
pub fn home_presence(clients: Option<Vec<Client>>, device: &DeviceIdentity) -> HomePresence {
    let Some(clients) = clients else {
        return HomePresence::Unknown;
    };
    // STRONGEST KEY FIRST, and the FIRST one that matches anything wins.
    // ANY match is Home, so the order is only ever read on disagreement,
    // which is exactly the operator's rule: when the keys agree the winner's
    // identity does not matter, and when they point at different clients the
    // strongest key is the one that says which client the device is. A key
    // that matches nothing is skipped and never a failure.
    if let Some(mac) = &device.mac
        && clients.iter().any(|client| {
            client.mac.as_deref().and_then(normalized_mac).as_deref() == Some(mac.as_str())
        })
    {
        return HomePresence::Home {
            matched_by: DeviceKey::Mac,
            value: mac.clone(),
        };
    }
    if let Some(hostname) = &device.hostname
        && clients
            .iter()
            .any(|client| client.name.as_deref() == Some(hostname.as_str()))
    {
        return HomePresence::Home {
            matched_by: DeviceKey::Hostname,
            value: hostname.clone(),
        };
    }
    if let Some(ipv4) = device.ipv4
        && clients.iter().any(|client| {
            client
                .ipv4
                .as_deref()
                .and_then(|text| text.parse::<std::net::Ipv4Addr>().ok())
                == Some(ipv4)
        })
    {
        return HomePresence::Home {
            matched_by: DeviceKey::Ipv4,
            value: ipv4.to_string(),
        };
    }
    // A COMPLETE listing that no configured key matched is the only NotHome
    // there is: every unreadable, unparseable or incomplete answer left
    // through the Unknown above.
    HomePresence::NotHome
}

/// The enabled router sensor's settings table, or the cause it could not be
/// had. The probe's config home is `[plugins.router]`, the sensor registered
/// in the roster, so the whole file is read by one schema and the operator has
/// one spelling to get right.
pub fn enabled_router_table(config: &crate::config::Config) -> Result<&toml::Table, SetupFailure> {
    let entry = config
        .plugins
        .get("router")
        .ok_or(SetupFailure::NoRouterPlugin)?;
    // A probe the operator SWITCHED OFF is not one they never wrote: one is
    // fixed by flipping the flag in front of them, the other by writing a
    // table, and a single "not configured" line sends half of them to the
    // wrong edit.
    if !entry.enabled {
        return Err(SetupFailure::RouterDisabled);
    }
    Ok(&entry.settings)
}

/// The one brand a compiled-in backend answers. It is VALIDATED and then
/// discarded: `trait Router` is the seam a second backend enters through, and
/// the enum that dispatches between two of them is worth writing the day
/// there are two.
const UNIFI_BRAND: &str = "unifi";

/// The settings out of the router sensor's table, or the cause they could not
/// be had. The BRAND is settled first, because every setting under it belongs
/// to whichever router it names.
pub fn router_settings(router: &toml::Table) -> Result<RouterSettings, SetupFailure> {
    // Present but EMPTY is the key left blank, the same hole as absent: the
    // filter is what keeps `brand = ""` from being quoted back as a brand no
    // backend answers, which names a value the operator never typed.
    let brand = router
        .get("brand")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(SetupFailure::NoBrand)?;
    if brand != UNIFI_BRAND {
        return Err(SetupFailure::UnknownBrand(brand.to_string()));
    }
    // Present but empty is a hole, not a value, and present but the wrong
    // type is refused rather than coerced: both are one line for the operator
    // to fix, and neither is a router this probe could reach.
    let router_url = router
        .get("router_url")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(SetupFailure::InvalidRouterTable)?;
    Ok(RouterSettings {
        router_url,
        device: device_identity(router)?,
    })
}

/// The configured device out of the router's table, or the cause it could
/// not be had. ABSENT IS NOT CONFIGURED, and all three absent is refused: a
/// probe with nothing to look for would read NotHome forever.
pub fn device_identity(router: &toml::Table) -> Result<DeviceIdentity, SetupFailure> {
    // A client name is an arbitrary label, so there is no shape to check
    // beyond it being a string; an address is PARSED, so no unvalidated text
    // can reach a comparison.
    let hostname = read_device_key(router, DeviceKey::Hostname, |text| Some(text.to_string()))?;
    let ipv4 = read_device_key(router, DeviceKey::Ipv4, |text| text.parse().ok())?;
    let mac = read_device_key(router, DeviceKey::Mac, normalized_mac)?;
    if hostname.is_none() && ipv4.is_none() && mac.is_none() {
        return Err(SetupFailure::NoDeviceIdentifier);
    }
    Ok(DeviceIdentity {
        hostname,
        ipv4,
        mac,
    })
}

/// One MAC in the single spelling everything compares in: lowercase, colons,
/// or `None` for six-group text that is not one. THE SAME FUNCTION VALIDATES
/// AND COMPARES, on both sides, so the config's notion of equal and the
/// router's cannot drift apart.
fn normalized_mac(text: &str) -> Option<String> {
    // ONE uniform separator. No separator at all (a bare 12-hex run) and a
    // mix of the two are typos rather than spellings, and accepting the run
    // would mean guessing at a grouping nothing on the wire uses.
    let separator = match (text.contains(':'), text.contains('-')) {
        (true, false) => ':',
        (false, true) => '-',
        _ => return None,
    };
    let groups: Vec<&str> = text.split(separator).collect();
    (groups.len() == 6
        && groups
            .iter()
            .all(|group| group.len() == 2 && group.bytes().all(|byte| byte.is_ascii_hexdigit())))
    .then(|| groups.join(":").to_ascii_lowercase())
}

/// One optional device key, read through its own shape. ABSENT STAYS ABSENT;
/// present but empty, the wrong type, or unreadable as that shape is refused
/// by name. A blank key read as absent is the silent typo the output cannot
/// show: the probe would report having nothing to look for while the
/// operator is looking at the key they filled in.
fn read_device_key<T>(
    router: &toml::Table,
    key: DeviceKey,
    read: impl Fn(&str) -> Option<T>,
) -> Result<Option<T>, SetupFailure> {
    let Some(value) = router.get(key.config_key()) else {
        return Ok(None);
    };
    let refuse = || SetupFailure::InvalidDeviceKey {
        key,
        found: spell(value),
    };
    let text = value
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(refuse)?;
    read(text).map(Some).ok_or_else(refuse)
}

/// One config value as the operator would see it in the file: a string in
/// quotes, anything else by its TOML type, because a non-string has no
/// spelling worth echoing back and its TYPE is what has to change.
fn spell(value: &toml::Value) -> String {
    match value.as_str() {
        Some(text) => format!("{text:?}"),
        None => format!("<{}>", value.type_str()),
    }
}

/// The router's API key out of its own table (`api_key`), or `None` for every
/// way the config can fail to provide one.
///
/// SEPARATE from the settings, and it stays that way: the key never enters a
/// type that derives Debug, so it cannot ride a formatted dump into a log
/// line. Mirrors `moshi_secret`: the path from config to request header must
/// never touch argv, the environment, or an error string.
pub fn router_api_key(router: &toml::Table) -> Option<String> {
    let key = router.get("api_key")?.as_str()?;
    (!key.is_empty()).then(|| key.to_string())
}

/// One reading: fetch through the seam, parse, judge.
pub fn read_home_presence<R: Router>(router: &R, device: &DeviceIdentity) -> HomePresence {
    home_presence(
        router.clients_json().as_deref().and_then(parse_clients),
        device,
    )
}

/// The production router client, over the same HTTP stack as the other
/// native legs.
///
/// THE SECRET'S PATH IS THE POINT, exactly as in the moshi channel: the key
/// travels from the config into the request HEADER and nowhere else,
/// never argv, never a child's environment, never an error string. TLS
/// verification is disabled the way the hue bridge's is, and for the same
/// reason: the router serves a self-signed certificate for its own LAN
/// address, and no CA vouches for it.
pub struct UniFiRouter {
    /// The agent every call rides, INJECTED so a test can hand in one wearing
    /// a scripted transport (`Agent::with_parts`): the production pipeline
    /// runs for real and only the wire is fake, which is the closest Rust
    /// analog to stubbing Swift's URL Loading System.
    agent: ureq::Agent,
    /// e.g. `https://192.168.1.1`, from the `[plugins.router]` table.
    base: String,
    /// The API key, from the `[plugins.router]` table.
    key: String,
}

/// One bounded fetch: a probe on a diagnostic path is worth seconds, never a
/// hang.
const ROUTER_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

/// The router's answers are small (a 200-client listing measures kilobytes),
/// so the read is capped far below ureq's own 10MB default: a faulty router
/// streaming garbage costs at most this much memory before reading Unknown.
const ROUTER_BODY_CAP: u64 = 1_000_000;

impl UniFiRouter {
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
impl Router for UniFiRouter {
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
pub fn report(presence: &HomePresence) -> String {
    match presence {
        HomePresence::Home { matched_by, value } => format!(
            "home: on the home network (matched by {} \"{value}\")",
            matched_by.config_key()
        ),
        HomePresence::NotHome => {
            "home: NOT on the home network (no configured identifier matched a client)".to_string()
        }
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
    NoRouterPlugin,
    RouterDisabled,
    NoBrand,
    UnknownBrand(String),
    InvalidRouterTable,
    NoDeviceIdentifier,
    InvalidDeviceKey { key: DeviceKey, found: String },
    NoApiKey,
}

/// The one line for a setup failure. PURE for the same reason as `report`.
pub fn setup_report(failure: &SetupFailure) -> String {
    match failure {
        SetupFailure::NoConfigFile => "home: not configured (no config file)".to_string(),
        SetupFailure::ConfigError(detail) => format!("home: config error ({detail})"),
        SetupFailure::NoRouterPlugin => {
            "home: not configured (no [plugins.router] table)".to_string()
        }
        SetupFailure::RouterDisabled => {
            "home: [plugins.router] is present but enabled = false".to_string()
        }
        SetupFailure::NoBrand => {
            format!("home: no brand in [plugins.router] (the only brand is \"{UNIFI_BRAND}\")")
        }
        SetupFailure::UnknownBrand(brand) => format!(
            "home: [plugins.router] has brand \"{brand}\", which no compiled-in backend \
             answers (the only brand is \"{UNIFI_BRAND}\")"
        ),
        SetupFailure::InvalidRouterTable => {
            "home: the [plugins.router] table is present but router_url is missing, empty, \
             or not a string"
                .to_string()
        }
        SetupFailure::NoDeviceIdentifier => format!(
            "home: no device to look for in [plugins.router] (set at least one of {}, {}, {})",
            DeviceKey::Mac.config_key(),
            DeviceKey::Hostname.config_key(),
            DeviceKey::Ipv4.config_key()
        ),
        SetupFailure::InvalidDeviceKey { key, found } => {
            let shape = match key {
                DeviceKey::Mac => {
                    "a MAC address (six hex pairs under one separator, e.g. \"2e:11:ab:6d:b0:4f\")"
                }
                DeviceKey::Hostname => "a client name (a non-empty string)",
                DeviceKey::Ipv4 => "an IPv4 address (a dotted quad, e.g. \"192.168.1.169\")",
            };
            format!(
                "home: {} = {found} in [plugins.router] is not {shape}",
                key.config_key()
            )
        }
        SetupFailure::NoApiKey => {
            "home: no api_key in the [plugins.router] table (the probe is not set up)".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Client, DeviceIdentity, DeviceKey, HomePresence, Router, RouterSettings, SetupFailure,
        device_identity, enabled_router_table, first_site_id, home_presence, parse_clients,
        read_home_presence, report, router_api_key, router_settings, setup_report,
    };

    /// The live capture of 2026-08-20 from the UDR's
    /// `/proxy/network/integration/v1/sites/{id}/clients`, verbatim: four
    /// clients, the phone ("mister") among them with an iOS private MAC.
    const CLIENTS_CAPTURE: &str = r#"{"offset":0,"limit":200,"count":4,"totalCount":4,"data":[{"type":"WIRED","id":"b6363aa8-67c6-326b-93da-90f8e9632d95","name":"hue-bridge-pro","connectedAt":"2026-08-14T03:01:20Z","ipAddress":"192.168.4.37","macAddress":"c4:29:96:bb:6d:cc","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"f7585d92-906f-3c6c-84c3-89e2ce6b2eda","name":"dresden","connectedAt":"2026-08-19T05:44:57Z","ipAddress":"192.168.1.26","macAddress":"3c:06:30:0f:8a:bf","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"71e95d68-f736-3554-b547-75fa1cdc7bf4","name":"mister","connectedAt":"2026-08-20T04:14:55Z","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"84038944-790a-3965-9f7e-cfee3468308c","name":"mouse","connectedAt":"2026-08-20T05:18:12Z","ipAddress":"192.168.1.248","macAddress":"60:82:46:3c:fb:01","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}}]}"#;

    // --- parsing the router's listing ---------------------------------------

    #[test]
    fn every_client_in_the_live_capture_is_read_with_all_three_of_its_fields() {
        let clients = parse_clients(CLIENTS_CAPTURE).expect("the live capture is a listing");
        assert_eq!(clients.len(), 4);
        assert_eq!(
            clients[2],
            Client {
                name: Some("mister".to_string()),
                ipv4: Some("192.168.1.169".to_string()),
                mac: Some("2e:11:ab:6d:b0:4f".to_string()),
            }
        );
        // An UNNAMED client STAYS in the listing now. The router has not
        // identified it, but it still carries the MAC and the address a
        // configured device can match on, and dropping it would answer
        // NotHome for a device sitting right there in the list.
        assert_eq!(
            parse_clients(r#"{"data":[{"macAddress":"2E:11:AB:6D:B0:4F"},{"name":"mister"}]}"#),
            Some(vec![
                Client {
                    name: None,
                    ipv4: None,
                    mac: Some("2E:11:AB:6D:B0:4F".to_string()),
                },
                Client {
                    name: Some("mister".to_string()),
                    ipv4: None,
                    mac: None,
                },
            ])
        );
    }

    #[test]
    fn a_listing_that_is_not_json_is_no_answer_rather_than_an_empty_wifi() {
        // Unparseable and empty must stay distinct: empty means "nobody is
        // on the wifi" and would read as the phone having LEFT.
        assert_eq!(parse_clients("<html>router login</html>"), None);
        assert_eq!(parse_clients(""), None);
    }

    #[test]
    fn a_json_answer_without_the_data_list_is_no_answer() {
        // The auth-failed shape: valid JSON, no clients in it.
        assert_eq!(parse_clients(r#"{"error":"unauthorized"}"#), None);
        assert_eq!(parse_clients(r#"{"data":"not-a-list"}"#), None);
    }

    #[test]
    fn a_parsed_empty_list_is_an_answer_and_not_a_failure() {
        assert_eq!(
            parse_clients(r#"{"offset":0,"limit":200,"count":0,"totalCount":0,"data":[]}"#),
            Some(Vec::new())
        );
    }

    // --- the verdict ---------------------------------------------------------

    /// A listing of clients the router named and nothing else, which is what
    /// every hostname case is about.
    fn only_names(names: &[&str]) -> Option<Vec<Client>> {
        Some(
            names
                .iter()
                .map(|name| Client {
                    name: Some((*name).to_string()),
                    ipv4: None,
                    mac: None,
                })
                .collect(),
        )
    }

    /// The configured device, through the FRONT DOOR: the identity a test
    /// judges against is the one the operator's table would have produced.
    fn identity(text: &str) -> DeviceIdentity {
        device_identity(&table(text)).expect("a valid device identity")
    }

    #[test]
    fn a_hostname_match_is_exact_so_a_sibling_device_cannot_answer_for_the_phone() {
        let device = identity("device_hostname = \"mister\"\n");
        assert_eq!(
            home_presence(only_names(&["dresden", "mister"]), &device),
            HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            }
        );
        // A substring match would let "mister-2" answer, and a case-blind
        // one would let "MISTER": both are other devices on this wifi.
        assert_eq!(
            home_presence(only_names(&["mister-2", "MISTER"]), &device),
            HomePresence::NotHome
        );
    }

    #[test]
    fn a_mac_only_identity_reads_home_through_the_one_normalized_spelling() {
        // The operator copied it off a sticker (uppercase, dashes) and the
        // UDR answers lowercase with colons. BOTH SIDES go through the same
        // normalizer, so those are one value and not two.
        let device = identity("device_mac = \"2E-11-AB-6D-B0-4F\"\n");
        let matched = HomePresence::Home {
            matched_by: DeviceKey::Mac,
            value: "2e:11:ab:6d:b0:4f".to_string(),
        };
        assert_eq!(
            home_presence(parse_clients(CLIENTS_CAPTURE), &device),
            matched
        );
        // And an UNNAMED client answers on its MAC, which is what the old
        // name filter would have thrown away.
        assert_eq!(
            home_presence(
                parse_clients(r#"{"data":[{"macAddress":"2E:11:AB:6D:B0:4F"}]}"#),
                &device
            ),
            matched
        );
        // A MAC the probe cannot read is a client that matches nothing.
        assert_eq!(
            home_presence(
                parse_clients(r#"{"data":[{"macAddress":"nonsense"},{"name":"dresden"}]}"#),
                &device
            ),
            HomePresence::NotHome
        );
    }

    #[test]
    fn an_ipv4_only_identity_reads_home_against_the_client_carrying_that_address() {
        // ADDRESSES are compared, never the texts they were written as: the
        // client's `ipAddress` is parsed the same way the config's value was.
        let device = identity("device_ipv4 = \"192.168.1.169\"\n");
        assert_eq!(
            home_presence(parse_clients(CLIENTS_CAPTURE), &device),
            HomePresence::Home {
                matched_by: DeviceKey::Ipv4,
                value: "192.168.1.169".to_string(),
            }
        );
        // A client whose address is missing or is not an IPv4 is a client
        // that matches nothing. The ROUTER is not the operator: its entries
        // are read for what they hold, never refused for what they lack.
        assert_eq!(
            home_presence(
                parse_clients(r#"{"data":[{"ipAddress":"not-an-address"},{"name":"dresden"}]}"#),
                &device
            ),
            HomePresence::NotHome
        );
    }

    /// All three keys pointed at the phone of the live capture, which is the
    /// only shape where the keys can disagree with each other.
    fn full_identity() -> DeviceIdentity {
        identity(
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_hostname = \"mister\"\n\
             device_ipv4 = \"192.168.1.169\"\n",
        )
    }

    #[test]
    fn any_one_configured_key_matching_reads_home_while_the_others_match_nothing() {
        // A key that matches NOTHING is skipped, never a failure: the device
        // is home on the strength of the one key that answered, whichever it
        // is. Each listing below carries exactly one of the three.
        for (listing, matched_by, value) in [
            (
                r#"{"data":[{"name":"dresden","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.7"}]}"#,
                DeviceKey::Mac,
                "2e:11:ab:6d:b0:4f",
            ),
            (
                r#"{"data":[{"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"}]}"#,
                DeviceKey::Hostname,
                "mister",
            ),
            (
                r#"{"data":[{"name":"dresden","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.169"}]}"#,
                DeviceKey::Ipv4,
                "192.168.1.169",
            ),
        ] {
            assert_eq!(
                home_presence(parse_clients(listing), &full_identity()),
                HomePresence::Home {
                    matched_by,
                    value: value.to_string(),
                },
                "case: {listing:?}"
            );
        }
    }

    #[test]
    fn on_keys_matching_different_clients_the_verdict_names_the_strongest() {
        // The three keys DISAGREE here: each points at a different client.
        // The strongest one that matched anything is the one that names the
        // device, because a MAC is the device itself, a name is a label the
        // operator can reuse, and an address is only today's lease.
        let disagreeing = r#"{"data":[
            {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
            {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"},
            {"name":"dresden","macAddress":"3c:06:30:0f:8a:bf","ipAddress":"192.168.1.169"}]}"#;
        assert_eq!(
            home_presence(parse_clients(disagreeing), &full_identity()),
            HomePresence::Home {
                matched_by: DeviceKey::Mac,
                value: "2e:11:ab:6d:b0:4f".to_string(),
            }
        );
        // Drop the MAC and the name is next in line; drop that too and the
        // address answers on its own.
        assert_eq!(
            home_presence(
                parse_clients(disagreeing),
                &identity("device_hostname = \"mister\"\ndevice_ipv4 = \"192.168.1.169\"\n")
            ),
            HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            }
        );
        assert_eq!(
            home_presence(
                parse_clients(disagreeing),
                &identity("device_ipv4 = \"192.168.1.169\"\n")
            ),
            HomePresence::Home {
                matched_by: DeviceKey::Ipv4,
                value: "192.168.1.169".to_string(),
            }
        );
    }

    #[test]
    fn a_complete_listing_no_key_matched_is_not_home_and_anything_less_is_unknown() {
        let device = full_identity();
        // NotHome needs a COMPLETE listing that none of the three keys found:
        // the wifi answered, and the device was not on it.
        assert_eq!(
            home_presence(
                parse_clients(
                    r#"{"totalCount":1,"data":[{"name":"mouse","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.248"}]}"#
                ),
                &device
            ),
            HomePresence::NotHome
        );
        assert_eq!(
            home_presence(parse_clients(r#"{"totalCount":0,"data":[]}"#), &device),
            HomePresence::NotHome
        );
        // Unreachable, unparseable and INCOMPLETE all stay Unknown. The
        // consumers act on transitions, so inventing a departure out of a
        // page the device could be beyond would fire a false one.
        for no_answer in [
            "<html>router login</html>",
            r#"{"error":"unauthorized"}"#,
            r#"{"offset":0,"limit":200,"count":1,"totalCount":201,"data":[{"name":"mouse"}]}"#,
        ] {
            assert_eq!(
                home_presence(parse_clients(no_answer), &device),
                HomePresence::Unknown,
                "case: {no_answer:?}"
            );
        }
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
        let device = identity("device_hostname = \"mister\"\n");
        assert_eq!(
            read_home_presence(&FakeRouter(Some(CLIENTS_CAPTURE)), &device),
            HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            }
        );
        assert_eq!(
            read_home_presence(&FakeRouter(None), &device),
            HomePresence::Unknown
        );
    }

    // --- the settings --------------------------------------------------------

    fn table(text: &str) -> toml::Table {
        text.parse().unwrap()
    }

    #[test]
    fn no_router_plugin_table_at_all_is_not_configured_naming_the_table() {
        // hermes rides along so this is a MISS on the router's own name and
        // not a config the parser dropped whole.
        let config = crate::config::parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
        assert_eq!(
            enabled_router_table(&config),
            Err(SetupFailure::NoRouterPlugin)
        );
        let line = setup_report(&SetupFailure::NoRouterPlugin);
        assert!(line.contains("not configured"), "got: {line}");
        assert!(line.contains("[plugins.router]"), "got: {line}");
    }

    #[test]
    fn a_router_table_switched_off_is_told_apart_from_no_table_at_all() {
        // Selection is the operator's, and a probe they turned off must not
        // read as one they never wrote: the first is fixed by flipping a flag
        // they are looking at, the second by writing a table.
        let config = crate::config::parse_config(
            "[plugins.router]\nenabled = false\nbrand = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
        )
        .unwrap();
        assert_eq!(
            enabled_router_table(&config),
            Err(SetupFailure::RouterDisabled)
        );
        let disabled = setup_report(&SetupFailure::RouterDisabled);
        assert_ne!(disabled, setup_report(&SetupFailure::NoRouterPlugin));
        assert!(disabled.contains("[plugins.router]"), "got: {disabled}");
        assert!(disabled.contains("enabled = false"), "got: {disabled}");
    }

    #[test]
    fn a_router_table_with_no_brand_names_the_key_and_the_one_brand_that_answers() {
        // WHICH ROUTER answers is the first question the table has to settle,
        // because every setting under it belongs to that one. A non-string
        // brand and an EMPTY one are the same hole: there is no name to match
        // a backend by, and the empty one used to be quoted back as a brand
        // nothing implements, which points at a value the operator never
        // typed instead of at the key they left blank.
        for text in [
            "router_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
            "brand = 5\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
            "brand = \"\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
        ] {
            assert_eq!(
                router_settings(&table(text)),
                Err(SetupFailure::NoBrand),
                "case: {text:?}"
            );
        }
        let line = setup_report(&SetupFailure::NoBrand);
        assert!(line.contains("brand"), "got: {line}");
        assert!(line.contains("[plugins.router]"), "got: {line}");
        assert!(line.contains("\"unifi\""), "got: {line}");
    }

    #[test]
    fn a_brand_no_compiled_in_backend_answers_is_refused_quoting_it() {
        // Silently probing a UniFi endpoint on a router that is not one would
        // read Unknown forever with nothing to look at; the refusal quotes
        // what was asked for and says what this binary can answer.
        let asus =
            table("brand = \"asus\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n");
        assert_eq!(
            router_settings(&asus),
            Err(SetupFailure::UnknownBrand("asus".to_string()))
        );
        let line = setup_report(&SetupFailure::UnknownBrand("asus".to_string()));
        assert!(line.contains("\"asus\""), "got: {line}");
        assert!(line.contains("\"unifi\""), "got: {line}");
        assert!(line.contains("[plugins.router]"), "got: {line}");
    }

    #[test]
    fn an_enabled_unifi_router_table_yields_its_url_and_device() {
        // The whole value path in one: the config's `[plugins.router]` table,
        // through the selection gate, into the two settings the probe runs on.
        let config = crate::config::parse_config(
            "[plugins.router]\nenabled = true\nbrand = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
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
    fn a_missing_empty_or_mistyped_url_reports_the_invalid_table_line() {
        // A present-but-wrong VALUE is fixed by editing one line; a missing
        // TABLE is fixed by writing one. `router_url = 5` reported as "no
        // table" used to send the operator to write a table they already had.
        let brand = "brand = \"unifi\"\n";
        for text in [
            "device_hostname = \"mister\"\n",
            "router_url = \"\"\ndevice_hostname = \"mister\"\n",
            "router_url = 5\ndevice_hostname = \"mister\"\n",
        ] {
            assert_eq!(
                router_settings(&table(&format!("{brand}{text}"))),
                Err(SetupFailure::InvalidRouterTable),
                "case: {text:?}"
            );
        }
        let invalid = setup_report(&SetupFailure::InvalidRouterTable);
        assert_ne!(invalid, setup_report(&SetupFailure::NoRouterPlugin));
        assert!(invalid.contains("[plugins.router]"), "got: {invalid}");
        assert!(invalid.contains("router_url"), "got: {invalid}");
        // The line stops naming the device keys: each of the three has its
        // own refusal now, and one covering all four sends the operator to
        // read four keys to find the one that is wrong.
        assert!(!invalid.contains("device_"), "got: {invalid}");
    }

    // --- the device identity -------------------------------------------------

    #[test]
    fn a_router_table_naming_no_device_at_all_is_refused_naming_every_key() {
        // Absent is not configured, and all three absent is a probe with
        // nothing to look for. The line has to spell all three keys: "no
        // device identifier" on its own sends the operator to the docs to
        // find out what one is called.
        assert_eq!(
            device_identity(&table(
                "brand = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\n"
            )),
            Err(SetupFailure::NoDeviceIdentifier)
        );
        let line = setup_report(&SetupFailure::NoDeviceIdentifier);
        assert!(line.contains("device_mac"), "got: {line}");
        assert!(line.contains("device_hostname"), "got: {line}");
        assert!(line.contains("device_ipv4"), "got: {line}");
    }

    #[test]
    fn a_table_carrying_only_a_hostname_yields_it_and_the_retired_key_is_not_read() {
        assert_eq!(
            device_identity(&table("device_hostname = \"mister\"\n")),
            Ok(DeviceIdentity {
                hostname: Some("mister".to_string()),
                ipv4: None,
                mac: None,
            })
        );
        // `phone` is the RETIRED spelling. Reading it as an identifier would
        // hide the rename the operator still has to make, and this repo does
        // not carry compatibility for its own unshipped code.
        assert_eq!(
            device_identity(&table("phone = \"mister\"\n")),
            Err(SetupFailure::NoDeviceIdentifier)
        );
        // Present but EMPTY, and present but the wrong TYPE, are the same
        // hole: the key is there and there is no device in it. Read as
        // absent, both would report "no device to look for" while the
        // operator is looking straight at a value they typed.
        // The wrong type is quoted back BY TYPE, because an integer has no
        // spelling worth echoing and the type is what has to change.
        for (text, found) in [
            ("device_hostname = \"\"\n", "\"\""),
            ("device_hostname = 5\n", "<integer>"),
            ("device_hostname = [\"mister\"]\n", "<array>"),
        ] {
            assert_eq!(
                device_identity(&table(text)),
                Err(SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Hostname,
                    found: found.to_string(),
                }),
                "case: {text:?}"
            );
        }
        let line = setup_report(&SetupFailure::InvalidDeviceKey {
            key: DeviceKey::Hostname,
            found: "<integer>".to_string(),
        });
        assert!(line.contains("device_hostname"), "got: {line}");
        assert!(line.contains("<integer>"), "got: {line}");
    }

    #[test]
    fn a_malformed_device_ipv4_is_refused_naming_the_key_and_quoting_the_value() {
        // The stdlib parser IS the validator, and these are the shapes it
        // refuses, measured on this toolchain (rustc 1.92.0-nightly): a
        // leading zero, a short quad, a long one, an octet past 255, and
        // surrounding whitespace. A hand-rolled octet parser would have to
        // relearn every one of them.
        for bad in [
            "010.1.1.1",
            "1.2.3",
            "1.2.3.4.5",
            "256.1.1.1",
            " 1.2.3.4",
            "1.2.3.4 ",
            "",
        ] {
            assert_eq!(
                device_identity(&table(&format!("device_ipv4 = \"{bad}\"\n"))),
                Err(SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Ipv4,
                    found: format!("{bad:?}"),
                }),
                "case: {bad:?}"
            );
        }
        // The line quotes what was typed, so the typo is visible without
        // opening the file.
        let line = setup_report(&SetupFailure::InvalidDeviceKey {
            key: DeviceKey::Ipv4,
            found: "\"1.2.3\"".to_string(),
        });
        assert!(line.contains("device_ipv4"), "got: {line}");
        assert!(line.contains("\"1.2.3\""), "got: {line}");
        // A well-formed one is kept as an ADDRESS, never as the text it was
        // typed as: nothing downstream can compare it as a string.
        assert_eq!(
            device_identity(&table("device_ipv4 = \"192.168.1.169\"\n")),
            Ok(DeviceIdentity {
                hostname: None,
                ipv4: Some(std::net::Ipv4Addr::new(192, 168, 1, 169)),
                mac: None,
            })
        );
    }

    #[test]
    fn a_malformed_device_mac_is_refused_naming_the_key_and_quoting_the_value() {
        // Too few groups, too many, a bare 12-hex run, a non-hex digit, a
        // group that is not exactly two digits, a MIXED separator, and a
        // trailing one. A single uniform separator is what tells a MAC from
        // a typo, and accepting the bare run would mean guessing at
        // groupings the router never uses.
        for bad in [
            "2e:11:ab:6d:b0",
            "2e:11:ab:6d:b0:4f:aa",
            "2e11ab6db04f",
            "zz:11:ab:6d:b0:4f",
            "2e:1:ab:6d:b0:4f",
            "2e-11:ab-6d:b0-4f",
            "2e:11:ab:6d:b0:4f:",
        ] {
            assert_eq!(
                device_identity(&table(&format!("device_mac = \"{bad}\"\n"))),
                Err(SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Mac,
                    found: format!("{bad:?}"),
                }),
                "case: {bad:?}"
            );
        }
        let line = setup_report(&SetupFailure::InvalidDeviceKey {
            key: DeviceKey::Mac,
            found: "\"2e11ab6db04f\"".to_string(),
        });
        assert!(line.contains("device_mac"), "got: {line}");
        assert!(line.contains("\"2e11ab6db04f\""), "got: {line}");
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
                Ok(DeviceIdentity {
                    hostname: None,
                    ipv4: None,
                    mac: Some("2e:11:ab:6d:b0:4f".to_string()),
                }),
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
            "[plugins.router]\nenabled = true\nbrand = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\napi_key = \"k-123\"\n",
        )
        .unwrap();
        let router = enabled_router_table(&config).expect("the enabled table");
        assert_eq!(router_api_key(router), Some("k-123".to_string()));
    }

    #[test]
    fn every_way_the_router_table_fails_to_provide_a_key_is_quietly_not_set_up() {
        for router in ["", "api_key = \"\"\n", "api_key = 5\n"] {
            assert_eq!(router_api_key(&table(router)), None, "case: {router:?}");
        }
        // And the line sends the operator to the table the key now lives in.
        let line = setup_report(&SetupFailure::NoApiKey);
        assert!(line.contains("[plugins.router]"), "got: {line}");
        assert!(line.contains("api_key"), "got: {line}");
    }

    // --- pagination: an incomplete page must not report a departure ----------

    #[test]
    fn a_page_the_phone_could_be_beyond_is_no_answer_rather_than_not_home() {
        // totalCount says 201 clients exist and the page carries one: the
        // phone may be among the 200 not shown, so absent-from-this-page must
        // not become NotHome, which is a false departure.
        assert_eq!(
            parse_clients(
                r#"{"offset":0,"limit":200,"count":1,"totalCount":201,"data":[{"name":"dresden"}]}"#
            ),
            None
        );
    }

    #[test]
    fn a_complete_page_with_unnamed_clients_still_answers() {
        // Completeness is judged on the ENTRIES: a page carrying as many as
        // the router counted is whole, and the unnamed entry is one of them.
        assert_eq!(
            parse_clients(r#"{"totalCount":2,"data":[{"id":"x"},{"name":"mister"}]}"#)
                .map(|clients| clients.len()),
            Some(2)
        );
    }

    #[test]
    fn a_listing_without_total_count_is_taken_whole_as_before() {
        assert_eq!(
            parse_clients(r#"{"data":[{"name":"mister"}]}"#),
            only_names(&["mister"])
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
        // The Home line NAMES the identifier that answered and the value it
        // answered with, which is the only observable difference precedence
        // makes: the operator can see WHICH key spoke without a per-key
        // breakdown that would expose the probe's internals.
        assert_eq!(
            report(&HomePresence::Home {
                matched_by: DeviceKey::Mac,
                value: "2e:11:ab:6d:b0:4f".to_string(),
            }),
            "home: on the home network (matched by device_mac \"2e:11:ab:6d:b0:4f\")"
        );
        assert_eq!(
            report(&HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            }),
            "home: on the home network (matched by device_hostname \"mister\")"
        );
        assert_eq!(
            report(&HomePresence::NotHome),
            "home: NOT on the home network (no configured identifier matched a client)"
        );
        assert_eq!(
            report(&HomePresence::Unknown),
            "home: unknown (router unreachable or its answer unreadable)"
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
            (SetupFailure::NoRouterPlugin, "[plugins.router]"),
            (SetupFailure::RouterDisabled, "[plugins.router]"),
            (SetupFailure::NoBrand, "brand"),
            (SetupFailure::UnknownBrand("asus".to_string()), "asus"),
            (SetupFailure::InvalidRouterTable, "router_url"),
            (SetupFailure::NoDeviceIdentifier, "device_hostname"),
            (
                SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Ipv4,
                    found: "\"1.2.3\"".to_string(),
                },
                "device_ipv4",
            ),
            (SetupFailure::NoApiKey, "api_key"),
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
    ) -> (super::UniFiRouter, Arc<Mutex<Vec<u8>>>) {
        let connector = ScriptedConnector::default();
        let wire = Arc::clone(&connector.wire);
        *connector.responses.lock().unwrap() = responses.iter().cloned().collect();
        let config = ureq::Agent::config_builder().max_redirects(0).build();
        let agent = ureq::Agent::with_parts(config, connector, DefaultResolver::default());
        (
            super::UniFiRouter::with_agent(
                agent,
                "http://localhost:9".to_string(),
                key.to_string(),
            ),
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
        assert_eq!(
            read_home_presence(&router, &identity("device_hostname = \"mister\"\n")),
            HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            }
        );
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
        assert_eq!(
            read_home_presence(&router, &identity("device_hostname = \"mister\"\n")),
            HomePresence::Unknown
        );
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
        assert_eq!(
            read_home_presence(&router, &identity("device_hostname = \"mister\"\n")),
            HomePresence::Unknown
        );
    }
}
